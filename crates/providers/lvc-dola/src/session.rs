//! Phiên API app Android cho MỘT tài khoản — port `AppSession` (`dola_app.py:263-383`).
//!
//! Dựng từ cookie web (`sessionid`, `uid_tt`, `odin_tt`, `sid_guard`...) + thiết bị (id đã đăng ký
//! hoặc bộ id gốc) + proxy. Mọi request POST ký qua [`RequestSigner`] (RC4 `dolaSign` hoặc ký thật
//! X-Gorgon/Argus — xem `sign`). KHÔNG bao giờ log cookie/chữ ký/device id.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use lvc_core::ProxySpec;
use serde_json::Value;
use sha1::{Digest as _, Sha1};

use crate::error::ApiError;
use crate::payload::{BOT_ID, ChatIdentity};

/// Host API app (`dola_app.py:29`).
pub const HOST: &str = "api16-normal-i18n-myb.dola.com";
/// `aid` app Dola Android.
pub const AID: u32 = 489_823;
/// Timeout đọc SSE `chat/completion` (`config.API_SSE_TIMEOUT`).
pub const SSE_TIMEOUT: Duration = Duration::from_secs(1800);
/// Timeout poll chat (`config.API_POLL_TIMEOUT`).
pub const POLL_TIMEOUT: Duration = Duration::from_secs(90);

/// Ký một request. `query` = phần sau `?` của URL (đã có `_rticket`), `headers` = header sẽ gửi,
/// `now_ms` = `X-SS-REQ-TICKET`. Trả các header chữ ký cần thêm (X-Gorgon/X-Khronos/X-Ladon[/X-Argus]).
pub trait RequestSigner: Send + Sync {
    fn sign(
        &self,
        query: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
        cookie: &str,
        now_ms: u64,
    ) -> Vec<(String, String)>;
}

/// Thiết bị Android dùng cho phiên (id do server cấp khi đăng ký, hoặc bộ id gốc `_REF_DEVICE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceIds {
    pub device_id: String,
    pub install_id: String,
    pub cdid: String,
    pub device_type: String,
    pub device_brand: String,
    pub os_version: String,
    pub resolution: String,
    pub dpi: String,
    /// Thiết bị đã đăng ký với Dola (đi kèm ký thật) hay bộ id gốc dùng chung.
    pub registered: bool,
}

impl DeviceIds {
    /// Bộ id app thật dùng chung (`dola_app.py:32-34`, `:60`) — dự phòng khi chưa đăng ký được.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            device_id: "7681452747673093652".into(),
            install_id: "7681561912517068597".into(),
            cdid: "ee3c4d85-c8f0-4c57-8027-1ec83293a5d5".into(),
            device_type: "SM-A525M".into(),
            device_brand: "samsung".into(),
            os_version: "14".into(),
            resolution: "1080*2186".into(),
            dpi: "420".into(),
            registered: false,
        }
    }
}

/// Cookie `ttreq` của app: `1$<sha1(device_id_install_id)>` (`device_profile.py:52-54`).
#[must_use]
pub fn ttreq_cookie(device_id: &str, install_id: &str) -> String {
    let d = Sha1::digest(format!("{device_id}_{install_id}").as_bytes());
    format!("1${}", hex::encode(d))
}

/// Tuỳ chọn dựng phiên.
#[derive(Clone)]
pub struct SessionOpts {
    /// `https://api16-normal-i18n-myb.dola.com` — test trỏ về wiremock.
    pub base_url: String,
    pub language: String,
    pub signer: Arc<dyn RequestSigner>,
}

impl std::fmt::Debug for SessionOpts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionOpts")
            .field("base_url", &self.base_url)
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// Phiên API của một tài khoản.
pub struct AppSession {
    pub account: String,
    pub uid: String,
    pub language: String,
    pub device: DeviceIds,
    pub params_chat: String,
    pub params_im: String,
    base_url: String,
    headers: Vec<(String, String)>,
    cookie: String,
    signer: Arc<dyn RequestSigner>,
    client: reqwest::Client,
    proxy: Option<ProxySpec>,
}

impl std::fmt::Debug for AppSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppSession")
            .field("account", &self.account)
            .field("registered_device", &self.device.registered)
            .field("proxy", &self.proxy.as_ref().map(ProxySpec::server))
            .finish_non_exhaustive()
    }
}

/// Mã hoá phần trăm cho giá trị query (chỉ giữ ký tự không cần mã hoá).
fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'*') {
            out.push(char::from(b));
        } else {
            use std::fmt::Write as _;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Client reqwest KHÔNG đọc proxy môi trường (`trust_env = False`, `dola_app.py:323`).
///
/// # Errors
/// `ApiError::Network` khi proxy/TLS không dựng được (thông điệp không chứa mật khẩu proxy).
pub fn build_client(proxy: Option<&ProxySpec>) -> Result<reqwest::Client, ApiError> {
    let mut b = reqwest::Client::builder().no_proxy().gzip(true);
    if let Some(spec) = proxy {
        let server = spec.server();
        let mut p = reqwest::Proxy::all(&server)
            .map_err(|e| ApiError::Network(format!("proxy {server}: {e}")))?;
        if spec.has_auth() {
            p = p.basic_auth(
                spec.username.as_deref().unwrap_or(""),
                spec.password.as_deref().unwrap_or(""),
            );
        }
        b = b.proxy(p);
    }
    b.build()
        .map_err(|e| ApiError::Network(format!("reqwest client: {e}")))
}

/// Lỗi reqwest → `ProxyDown` (có proxy + lỗi ở tầng kết nối) hoặc `Network`
/// (`_is_proxy_dead`, `dola_app.py:193-202`, `:366-376`).
pub(crate) fn net_error(e: reqwest::Error, proxy: Option<&ProxySpec>) -> ApiError {
    let (connect, timeout) = (e.is_connect(), e.is_timeout());
    let e = e.without_url();
    let chain = {
        let mut s = e.to_string();
        let mut src = std::error::Error::source(&e);
        while let Some(x) = src {
            s.push_str(": ");
            s.push_str(&x.to_string());
            src = x.source();
        }
        s
    };
    if let Some(p) = proxy {
        let low = chain.to_lowercase();
        let dead = connect
            || timeout
            || [
                "10054",
                "10060",
                "10061",
                "forcibly closed",
                "tunnel",
                "proxy",
                "connection reset",
                "connection refused",
                "wrong_version_number",
                "tls",
                "ssl",
            ]
            .iter()
            .any(|k| low.contains(k));
        if dead {
            return ApiError::ProxyDown(format!("{}: {chain}", p.server()));
        }
    }
    ApiError::Network(chain)
}

fn now_ms() -> u64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

impl AppSession {
    /// Dựng phiên từ cookie (`tên → giá trị`).
    ///
    /// # Errors
    /// `LoggedOut` khi thiếu `sessionid`; `Network` khi không dựng được client/proxy.
    #[allow(
        clippy::too_many_lines,
        reason = "dung query string + cookie + header cua app Android - chep nguyen tu AppSession Python, tach nho lam kho doi chieu"
    )]
    pub fn new(
        account: &str,
        cookies: &HashMap<String, String>,
        device: DeviceIds,
        proxy: Option<ProxySpec>,
        opts: &SessionOpts,
    ) -> Result<Self, ApiError> {
        let get = |k: &str| cookies.get(k).map(String::as_str).unwrap_or_default();
        let sessionid = [get("sessionid"), get("sessionid_ss")]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default()
            .to_string();
        if sessionid.is_empty() {
            return Err(ApiError::LoggedOut("cookie khong co sessionid".into()));
        }
        let uid = [get("uid_tt"), get("uid_tt_ss")]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default()
            .to_string();
        let carrier = {
            let c = get("store-country-code");
            if c.is_empty() {
                "vn".to_string()
            } else {
                c.to_lowercase()
            }
        };
        let lang = opts.language.as_str();
        let d = &device;
        let common = format!(
            "device_platform=android&os=android&ssmix=a\
             &cdid={cdid}&channel=googleplay&aid=489823\
             &app_name=nova_ai&version_code=14080001&version_name=14.8.0\
             &manifest_version_code=14080005&update_version_code=14080040\
             &resolution={res}&dpi={dpi}&device_type={dtype}&device_brand={dbrand}\
             &language={lang}&os_api=34&os_version={dos}&ac=wifi\
             &uid={uid}&app_language={lang}&carrier_region={carrier}&flow_app_variant=cici\
             &sys_region=JP&tz_name=Asia%2FTokyo&system_language_detail={lang}-JP\
             &user_is_login=1&is_new_user=0&region=US&lang={lang}&pkg_type=release_version\
             &iid={iid}&device_id={did}\
             &doubao_update_version_code=14080040&doubao_os_version={dos}\
             &doubao_device_type={dtype}&doubao_device_brand={dbrand}\
             &doubao_device_platform=android&region=US&flow_sdk_version=14080040",
            cdid = d.cdid,
            res = d.resolution,
            dpi = d.dpi,
            dtype = pct(&d.device_type),
            dbrand = pct(&d.device_brand),
            dos = d.os_version,
            iid = d.install_id,
            did = d.device_id,
        );
        let store_idc = {
            let s = get("store-idc");
            if s.is_empty() { "maliva" } else { s }
        };
        let sid_tt = {
            let s = get("sid_tt");
            if s.is_empty() { sessionid.as_str() } else { s }
        };
        let cookie = format!(
            "store-idc={store_idc}; store-country-code={carrier}; store-country-code-src=uid; \
             install_id={iid}; ttreq={ttreq}; odin_tt={odin}; sid_guard={guard}; \
             sessionid={sessionid}; sessionid_ss={sessionid}; sid_tt={sid_tt}",
            iid = d.install_id,
            ttreq = ttreq_cookie(&d.device_id, &d.install_id),
            odin = get("odin_tt"),
            guard = get("sid_guard"),
        );
        let ua = format!(
            "com.larus.wolf/14080005 (Linux; U; Android {dos}; {lang}_JP; {dtype}; \
             Build/UP1A.231005.007; Cronet/TTNetVersion:f5f9daf9 2026-08-06 QuicVersion:cad331fc 2026-06-29)",
            dos = d.os_version,
            dtype = d.device_type,
        );
        let headers: Vec<(String, String)> = [
            ("X-OMNI-REQUEST", "1"),
            ("sdk-version", "2"),
            (
                "passport-sdk-settings",
                "device_transfer_s_0,device_transfer_ab_0",
            ),
            ("passport-sdk-version", "60592"),
            ("Content-Type", "application/json; encoding=utf-8"),
            ("x-tt-store-region", carrier.as_str()),
            ("x-tt-store-region-src", "uid"),
            ("X-SS-DP", "489823"),
            ("User-Agent", ua.as_str()),
            // KHONG ttzip: server tra nen rieng ByteDance (dola_app.py:12, :314).
            ("Accept-Encoding", "gzip, deflate"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let client = build_client(proxy.as_ref())?;
        Ok(Self {
            account: account.to_string(),
            uid,
            language: opts.language.clone(),
            params_chat: format!("flow_im_arch=v2&is_retry=0&{common}"),
            params_im: format!("flow_im_arch=v2&{common}"),
            device,
            base_url: opts.base_url.trim_end_matches('/').to_string(),
            headers,
            cookie,
            signer: Arc::clone(&opts.signer),
            client,
            proxy,
        })
    }

    /// Định danh chat cho `payload::*`.
    #[must_use]
    pub fn identity(&self) -> ChatIdentity {
        ChatIdentity {
            bot_id: BOT_ID.to_string(),
            uid: self.uid.clone(),
            language: self.language.clone(),
        }
    }

    #[must_use]
    pub fn proxy(&self) -> Option<&ProxySpec> {
        self.proxy.as_ref()
    }

    #[must_use]
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// POST có ký (`AppSession.post`, `dola_app.py:359-383`). `extra` ghi đè header (vd gzip body).
    ///
    /// # Errors
    /// `ProxyDown`/`Network` (mạng), `LoggedOut` (401/403), `Http` (khác 200).
    pub async fn post(
        &self,
        path: &str,
        params: &str,
        body: Vec<u8>,
        timeout: Duration,
        extra: &[(&str, &str)],
    ) -> Result<reqwest::Response, ApiError> {
        let ts = now_ms();
        let query = format!("{params}&_rticket={ts}");
        let url = format!("{}/{path}?{query}", self.base_url);
        let mut headers = self.headers.clone();
        headers.push(("Cookie".into(), self.cookie.clone()));
        headers.push(("X-SS-REQ-TICKET".into(), ts.to_string()));
        for (k, v) in extra {
            if let Some(h) = headers.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(k)) {
                h.1 = (*v).to_string();
            } else {
                headers.push(((*k).to_string(), (*v).to_string()));
            }
        }
        let sig = self.signer.sign(&query, &headers, None, &self.cookie, ts);
        headers.extend(sig);
        let mut req = self.client.post(&url).timeout(timeout).body(body);
        for (k, v) in &headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req
            .send()
            .await
            .map_err(|e| net_error(e, self.proxy.as_ref()))?;
        let status = resp.status().as_u16();
        let logid = resp
            .headers()
            .get("x-tt-logid")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if status == 401 || status == 403 {
            return Err(ApiError::LoggedOut(format!(
                "HTTP {status} - phien/cookie khong con hieu luc{}",
                if logid.is_empty() {
                    String::new()
                } else {
                    format!(" · logid {logid}")
                }
            )));
        }
        if status != 200 {
            let text = resp.text().await.unwrap_or_default();
            return Err(ApiError::Http {
                status,
                body: text.chars().take(200).collect(),
            });
        }
        Ok(resp)
    }

    /// POST JSON, đọc JSON trả về.
    ///
    /// # Errors
    /// Như [`AppSession::post`] + `Protocol` khi body không phải JSON.
    pub async fn post_json(
        &self,
        path: &str,
        params: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<Value, ApiError> {
        let bytes = serde_json::to_vec(body).map_err(|e| ApiError::Protocol(e.to_string()))?;
        let resp = self.post(path, params, bytes, timeout, &[]).await?;
        let text = resp
            .text()
            .await
            .map_err(|e| net_error(e, self.proxy.as_ref()))?;
        serde_json::from_str(&text).map_err(|e| {
            ApiError::Protocol(format!(
                "{path}: JSON hong ({e}): {}",
                text.chars().take(120).collect::<String>()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoSign;
    impl RequestSigner for NoSign {
        fn sign(
            &self,
            _q: &str,
            _h: &[(String, String)],
            _b: Option<&[u8]>,
            _c: &str,
            _t: u64,
        ) -> Vec<(String, String)> {
            vec![("X-Gorgon".into(), "g".into())]
        }
    }

    fn opts() -> SessionOpts {
        SessionOpts {
            base_url: "https://example.invalid".into(),
            language: "ja".into(),
            signer: Arc::new(NoSign),
        }
    }

    #[test]
    fn ttreq_khop_sha1_python() {
        // python: "1$" + hashlib.sha1(b"123_456").hexdigest()
        assert_eq!(
            ttreq_cookie("123", "456"),
            "1$d4f8b0f74c4089325e88c74dfd961dc251fd6044"
        );
    }

    #[test]
    fn thieu_sessionid_la_mat_phien() {
        let e = AppSession::new("a", &HashMap::new(), DeviceIds::reference(), None, &opts())
            .unwrap_err();
        assert!(matches!(e, ApiError::LoggedOut(_)));
    }

    #[test]
    fn query_co_dung_thiet_bi_va_uid_cookie_co_ttreq() {
        let ck: HashMap<String, String> = [
            ("sessionid_ss", "S1"),
            ("uid_tt", "U9"),
            ("store-country-code", "JP"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let s = AppSession::new("a", &ck, DeviceIds::reference(), None, &opts()).unwrap();
        assert!(
            s.params_chat
                .starts_with("flow_im_arch=v2&is_retry=0&device_platform=android")
        );
        assert!(s.params_im.contains("&device_id=7681452747673093652"));
        assert!(s.params_im.contains("&uid=U9&"));
        assert!(s.params_im.contains("&carrier_region=jp&"));
        assert!(
            s.cookie
                .contains("sessionid=S1; sessionid_ss=S1; sid_tt=S1")
        );
        assert!(s.cookie.contains("store-idc=maliva"));
        assert!(
            format!("{s:?}").find("S1").is_none(),
            "Debug khong lo cookie"
        );
    }
}
