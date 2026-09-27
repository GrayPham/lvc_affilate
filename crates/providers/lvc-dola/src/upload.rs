//! Upload ảnh tham chiếu lên `ImageX` của Dola (3 bước) — port `upload_image`
//! (`dola_client.py:455-639`), nhánh `cloak=false` (KHÔNG che ảnh; ta dùng ảnh Dola tự tạo hoặc
//! ảnh gốc, việc lách chân dung do engine lo).
//!
//! Ba bước:
//! 1. `POST alice/resource/prepare_upload` (ký X-Gorgon qua [`AppSession`]) → `service_id` +
//!    `upload_auth_token` (`AccessKeyId`/`secret_key`/`session_token` cho AWS4).
//! 2. `GET /top/v1?Action=ApplyImageUpload` ký AWS4 ([`crate::aws4`]) → host CDN + `StoreUri` + Auth.
//! 3. `POST {cdn}/upload/v1/{uri}` (Auth của CDN) tải byte lên, rồi
//!    `POST alice/resource/register` (body gzip, ký X-Gorgon) đăng ký tài nguyên → `resource_id`.
//!
//! KHÔNG log `AccessKeyId`/`secret_key`/`session_token`/Auth (bí mật tạm của phiên upload).

use std::collections::BTreeMap;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

use crate::aws4::aws4_sign;
use crate::datetime;
use crate::dims;
use crate::error::ApiError;
use crate::payload::{BOT_ID, UploadedImage};
use crate::session::{AppSession, HOST, POLL_TIMEOUT, net_error};

/// Hằng `ImageX` chép từ `dola_client.py` (`:522-543`).
const REGION_AWS: &str = "sdwdmwlll";
const SERVICE: &str = "imagex";
const APP_VER: &str = "14080001";
const APPID: &str = "489823";

/// Ảnh đã upload xong — thông tin để đính vào lệnh tạo video/ảnh (khối 10052) và tra cứu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedRef {
    /// `StoreUri` do CDN cấp (vd `tos-.../...`), dùng làm `uri` của khối đính.
    pub uri: String,
    pub md5: String,
    pub width: u32,
    pub height: u32,
    /// `resource_id` do `register` trả (có thể rỗng nếu server không trả).
    pub resource_id: String,
}

impl UploadedRef {
    /// Chuyển sang [`UploadedImage`] để dựng khối đính 10052 (`payload::attachment_block`).
    #[must_use]
    pub fn attachment(&self) -> UploadedImage {
        UploadedImage {
            uri: self.uri.clone(),
            md5: self.md5.clone(),
            width: self.width,
            height: self.height,
            resource_id: self.resource_id.clone(),
        }
    }
}

fn now_ms() -> u64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// base64 (chuẩn) dung nạp thiếu padding — bù `=` cho đủ bội số 4 (như `response::b64_text`).
fn b64_lenient(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    let pad = (4 - s.len() % 4) % 4;
    let padded = format!("{s}{}", "=".repeat(pad));
    base64::engine::general_purpose::STANDARD
        .decode(padded)
        .ok()
}

/// Bóc `AccessKeyId` từ `session_token`: bỏ 4 ký tự đầu, base64-decode phần còn lại, đọc JSON
/// (`dola_client.py:512-514`).
fn access_key_from_token(sts_raw: &str) -> Result<String, ApiError> {
    if sts_raw.len() < 4 {
        return Err(ApiError::Protocol("session_token qua ngan".into()));
    }
    let raw = b64_lenient(&sts_raw[4..])
        .ok_or_else(|| ApiError::Protocol("session_token khong phai base64".into()))?;
    let v: Value = serde_json::from_slice(&raw)
        .map_err(|e| ApiError::Protocol(format!("session_token JSON hong: {e}")))?;
    v.get("AccessKeyId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ApiError::Protocol("session_token thieu AccessKeyId".into()))
}

fn str_field<'a>(v: &'a Value, path: &[&str]) -> Result<&'a str, ApiError> {
    let mut cur = v;
    for k in path {
        cur = cur
            .get(*k)
            .ok_or_else(|| ApiError::Protocol(format!("thieu truong '{}'", path.join("."))))?;
    }
    cur.as_str()
        .ok_or_else(|| ApiError::Protocol(format!("'{}' khong phai chuoi", path.join("."))))
}

/// Upload một ảnh (byte) lên `ImageX`, trả [`UploadedRef`].
///
/// # Errors
/// `ApiError` các loại: mạng/proxy, `LoggedOut`, `Http` (bước AWS4/CDN khác 200), `Protocol` (JSON
/// thiếu trường / mã lỗi CDN/register khác kỳ vọng).
pub async fn upload_image(
    session: &AppSession,
    image_bytes: &[u8],
) -> Result<UploadedRef, ApiError> {
    let md5 = crate::sign::md5_hex(image_bytes);
    let crc32 = {
        let mut h = crc32fast::Hasher::new();
        h.update(image_bytes);
        format!("{:08x}", h.finalize())
    };
    let (width, height) = dims::image_size(image_bytes);

    let token = prepare_upload(session).await?; // bước 1: X-Gorgon
    let target = apply_image_upload(session, &token).await?; // bước 2: AWS4
    cdn_put(session, &target, image_bytes, &crc32).await?; // bước 3a: tải byte
    let resource_id = register(session, &target.oid_key, &md5, width, height).await?; // bước 3b

    Ok(UploadedRef {
        uri: target.oid_key,
        md5,
        width,
        height,
        resource_id,
    })
}

/// Kết quả bước 1: token AWS4 + `service_id` (KHÔNG log — chứa bí mật tạm).
struct UploadToken {
    service_id: String,
    sts_raw: String,
    sk: String,
    ak: String,
}

/// Đích upload do bước 2 (`ApplyImageUpload`) trả về.
struct UploadTarget {
    cdn_auth: String,
    oid_key: String,
    cdn_host: String,
}

/// Bước 1: `alice/resource/prepare_upload` (ký X-Gorgon) → token AWS4.
async fn prepare_upload(session: &AppSession) -> Result<UploadToken, ApiError> {
    let prep = session
        .post_json(
            "alice/resource/prepare_upload",
            &session.params_im,
            &json!({"resource_type": 2, "scene_id": "5", "tenant_id": "3"}),
            POLL_TIMEOUT,
        )
        .await?;
    let sts_raw = str_field(&prep, &["data", "upload_auth_token", "session_token"])?.to_string();
    let ak = access_key_from_token(&sts_raw)?;
    Ok(UploadToken {
        service_id: str_field(&prep, &["data", "service_id"])?.to_string(),
        sk: str_field(&prep, &["data", "upload_auth_token", "secret_key"])?.to_string(),
        sts_raw,
        ak,
    })
}

/// Bước 2: `GET /top/v1?Action=ApplyImageUpload` ký AWS4 → host CDN + `StoreUri` + Auth.
async fn apply_image_upload(
    session: &AppSession,
    token: &UploadToken,
) -> Result<UploadTarget, ApiError> {
    let ts = now_ms();
    let amz = datetime::amz_date(ts / 1000);
    let http_d = datetime::http_date(ts / 1000);
    let apply_params: BTreeMap<String, String> = [
        ("Action", "ApplyImageUpload"),
        ("FileType", "image"),
        ("Prefix", "rc_vlm"),
        ("ServiceId", token.service_id.as_str()),
        ("UploadNum", "1"),
        ("Version", "2018-08-01"),
        ("appid", APPID),
        ("device_platform", "android"),
        ("did", session.device.device_id.as_str()),
        ("region", "US"),
        ("uid", session.uid.as_str()),
        ("version_code", APP_VER),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let headers_to_sign: BTreeMap<String, String> = [
        ("x-amz-date", amz.as_str()),
        ("x-amz-security-token", token.sts_raw.as_str()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let auth = aws4_sign(
        "GET",
        HOST,
        "/top/v1",
        &apply_params,
        &headers_to_sign,
        b"",
        &token.ak,
        &token.sk,
        REGION_AWS,
        SERVICE,
        &amz,
    );
    // Query gui LEN phai TRUNG voi chuoi da ky (BTreeMap = sorted, gia tri tho khong ma hoa).
    let query = apply_params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let apply_url = format!("{}/top/v1?{query}", session.base_url());
    let apply_resp = session
        .client()
        .get(&apply_url)
        .timeout(Duration::from_secs(30))
        .header("Authorization", &auth)
        .header("Date", &http_d)
        .header("User-Agent", format!("BDFileUpload({ts})"))
        .header("X-Amz-Date", &amz)
        .header("X-Amz-Expires", "31536000")
        .header("X-Amz-Security-Token", &token.sts_raw)
        .header("accept-encoding", "identity")
        .header("X-SS-DP", APPID)
        .header("X-Neptune", "-8|50:51:59:30:40:47:49:31:39")
        .send()
        .await
        .map_err(|e| net_error(e, session.proxy()))?;
    let apply = read_ok_json(apply_resp, "ApplyImageUpload").await?;
    let addr = apply
        .get("Result")
        .and_then(|r| r.get("UploadAddress"))
        .ok_or_else(|| ApiError::Protocol("ApplyImageUpload thieu UploadAddress".into()))?;
    let store = addr
        .get("StoreInfos")
        .and_then(|s| s.get(0))
        .ok_or_else(|| ApiError::Protocol("ApplyImageUpload thieu StoreInfos".into()))?;
    Ok(UploadTarget {
        cdn_auth: str_field(store, &["Auth"])?.to_string(),
        oid_key: str_field(store, &["StoreUri"])?.to_string(),
        cdn_host: addr
            .get("UploadHosts")
            .and_then(|h| h.get(0))
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::Protocol("ApplyImageUpload thieu UploadHosts".into()))?
            .to_string(),
    })
}

/// Bước 3a: tải byte lên CDN (`POST {cdn}/upload/v1/{uri}`) — scheme theo `base_url` (test = http).
async fn cdn_put(
    session: &AppSession,
    target: &UploadTarget,
    image_bytes: &[u8],
    crc32: &str,
) -> Result<(), ApiError> {
    let scheme = if session.base_url().starts_with("https") {
        "https"
    } else {
        "http"
    };
    let cdn_url = format!(
        "{scheme}://{}/upload/v1/{}",
        target.cdn_host, target.oid_key
    );
    let ts = now_ms();
    let cdn_resp = session
        .client()
        .post(&cdn_url)
        .timeout(Duration::from_secs(60))
        .header("Authorization", &target.cdn_auth)
        .header("Date", datetime::http_date(ts / 1000))
        .header("User-Agent", format!("BDFileUpload({ts})"))
        .header("X-Upload-Content-CRC32", crc32)
        .header("accept-encoding", "identity")
        .header("X-SS-DP", APPID)
        .body(image_bytes.to_vec())
        .send()
        .await
        .map_err(|e| net_error(e, session.proxy()))?;
    let cdn_json = read_ok_json(cdn_resp, "CDN").await?;
    if cdn_json.get("code").and_then(Value::as_i64) != Some(2000) {
        return Err(ApiError::Protocol(format!(
            "CDN upload that bai: code={}",
            cdn_json.get("code").unwrap_or(&Value::Null)
        )));
    }
    Ok(())
}

/// Kiểm HTTP 200 rồi đọc JSON; khác 200 → `Http`, JSON hỏng → `Protocol` (kèm nhãn bước).
async fn read_ok_json(resp: reqwest::Response, what: &str) -> Result<Value, ApiError> {
    let status = resp.status().as_u16();
    if status != 200 {
        let body = resp.text().await.unwrap_or_default();
        return Err(ApiError::Http {
            status,
            body: body.chars().take(200).collect(),
        });
    }
    resp.json()
        .await
        .map_err(|e| ApiError::Protocol(format!("{what} JSON hong: {e}")))
}

/// `POST alice/resource/register` với body JSON nén gzip (`dola_client.py:589-636`).
async fn register(
    session: &AppSession,
    oid_key: &str,
    md5: &str,
    width: u32,
    height: u32,
) -> Result<String, ApiError> {
    let extra = json!({
        "bot_id": BOT_ID,
        "conversation_id": "",
        "device_id": session.device.device_id,
        "local_message_id": uuid::Uuid::new_v4().to_string(),
        "section_id": "",
    });
    let md5_head: String = md5.chars().take(10).collect();
    let body = json!({
        "extra": serde_json::to_string(&extra).unwrap_or_default(),
        "resource_list": [{
            "meta": {"image_meta": {"data": {
                "md5": md5,
                "image_ori": {"width": width, "height": height},
                "local_res_path": format!("/storage/emulated/0/Pictures/DolaAI/dola_canvas_{md5_head}.png"),
                "identifier": uuid::Uuid::new_v4().to_string(),
                "metadata": {
                    "is_marked": "0",
                    "source": "aigc",
                    "media_type": "ai_generated",
                    "generator": "Dola AI Model Studio",
                },
            }}},
            "resource_type": 2,
            "uri": oid_key,
        }],
        "scene_id": "5",
        "tenant_id": "3",
    });
    let raw = serde_json::to_vec(&body).map_err(|e| ApiError::Protocol(e.to_string()))?;
    let gz = gzip(&raw)?;
    let resp = session
        .post(
            "alice/resource/register",
            &session.params_im,
            gz,
            POLL_TIMEOUT,
            &[
                ("Content-Type", "application/json; charset=UTF-8"),
                ("x-bd-content-encoding", "gzip"),
            ],
        )
        .await?;
    let text = resp
        .text()
        .await
        .map_err(|e| net_error(e, session.proxy()))?;
    let reg: Value = serde_json::from_str(&text)
        .map_err(|e| ApiError::Protocol(format!("register JSON hong: {e}")))?;
    if reg.get("code").and_then(Value::as_i64) != Some(0) {
        return Err(ApiError::Protocol(format!(
            "register that bai: code={}",
            reg.get("code").unwrap_or(&Value::Null)
        )));
    }
    Ok(reg
        .get("data")
        .and_then(|d| d.get(oid_key))
        .and_then(|r| r.get("resource_id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

/// Nén gzip (mặc định) — `gzip.compress` của Python.
fn gzip(data: &[u8]) -> Result<Vec<u8>, ApiError> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write as _;
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).map_err(|e| ApiError::Io(e.to_string()))?;
    e.finish().map_err(|e| ApiError::Io(e.to_string()))
}
