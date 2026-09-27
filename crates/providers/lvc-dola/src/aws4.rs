//! Ký AWS Signature V4 cho `ImageX` (bước `ApplyImageUpload` của upload ảnh) — port CHÍNH XÁC
//! `_aws4_sign` (`dola_client.py:418-452`). HÀM THUẦN: nhận `amz_date` làm tham số để test tất định.
//!
//! Biến thể `ByteDance`: `canonical_querystring` KHÔNG mã hoá phần trăm giá trị (chép `f"{k}={v}"`),
//! nên bên gọi phải truyền `query_params` đúng giá trị THÔ sẽ gửi. `headers_to_sign` phải là khoá
//! CHỮ THƯỜNG (như bản Python luôn dùng `x-amz-date`, `x-amz-security-token`).

use std::collections::BTreeMap;

use hmac::{Hmac, Mac};
use sha2::{Digest as _, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// HMAC-SHA256(key, msg) → 32 byte.
fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC nhận khoá mọi độ dài");
    m.update(msg);
    m.finalize().into_bytes().to_vec()
}

/// SHA256(data) → hex thường.
fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// Sinh header `Authorization` AWS4-HMAC-SHA256 khớp `_aws4_sign` của bản Python.
///
/// `host` được giữ để 1-1 với chữ ký Python nhưng KHÔNG đi vào chữ ký (bản Python cũng bỏ qua) —
/// nhờ vậy test qua wiremock (host khác) vẫn khớp. `query_params`/`headers_to_sign` là `BTreeMap`
/// nên đã sắp theo khoá; khoá header phải viết thường.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "1-1 voi _aws4_sign cua Python (method/host/path/query/headers/body/ak/sk/region/service/amz_date)"
)]
pub fn aws4_sign(
    method: &str,
    host: &str,
    path: &str,
    query_params: &BTreeMap<String, String>,
    headers_to_sign: &BTreeMap<String, String>,
    body: &[u8],
    access_key: &str,
    secret_key: &str,
    region: &str,
    service: &str,
    amz_date: &str,
) -> String {
    let _ = host; // parity voi Python: nhan host nhung khong ky (server tu dung canonical).
    let date_stamp = &amz_date[..8];

    let canonical_querystring = query_params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    // Khoa da la chu thuong nen thu tu BTreeMap == sorted(k.lower()) cua Python.
    let mut canonical_headers = String::new();
    for (k, v) in headers_to_sign {
        use std::fmt::Write as _;
        let _ = writeln!(canonical_headers, "{}:{v}", k.to_lowercase());
    }
    let signed_headers = headers_to_sign
        .keys()
        .map(|k| k.to_lowercase())
        .collect::<Vec<_>>()
        .join(";");
    let payload_hash = sha256_hex(body);

    let canonical_request = format!(
        "{method}\n{path}\n{canonical_querystring}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    let credential_scope = format!("{date_stamp}/{region}/{service}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );

    let k_date = hmac(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, service.as_bytes());
    let k_signing = hmac(&k_service, b"aws4_request");
    let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes()));

    format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope},\
         SignedHeaders={signed_headers},Signature={signature}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn khop_vector_python_applyimageupload() {
        // Golden sinh tu _aws4_sign (dola_client.py) qua .venv Python — xem bao cao.
        let qp = map(&[
            ("Action", "ApplyImageUpload"),
            ("FileType", "image"),
            ("Prefix", "rc_vlm"),
            ("ServiceId", "svc123"),
            ("UploadNum", "1"),
            ("Version", "2018-08-01"),
            ("appid", "489823"),
            ("device_platform", "android"),
            ("did", "7681452747673093652"),
            ("region", "US"),
            ("uid", "U9"),
            ("version_code", "14080001"),
        ]);
        let h = map(&[
            ("x-amz-date", "20231114T221320Z"),
            ("x-amz-security-token", "STStokenABC"),
        ]);
        let got = aws4_sign(
            "GET",
            "api16-normal-i18n-myb.dola.com",
            "/top/v1",
            &qp,
            &h,
            b"",
            "AKIATEST",
            "secretkey123",
            "sdwdmwlll",
            "imagex",
            "20231114T221320Z",
        );
        assert_eq!(
            got,
            "AWS4-HMAC-SHA256 Credential=AKIATEST/20231114/sdwdmwlll/imagex/aws4_request,\
             SignedHeaders=x-amz-date;x-amz-security-token,\
             Signature=b1096fb26392251fbc05797214b56e38c10bb8b27051924f188da29613b4f8d6"
        );
    }

    #[test]
    fn khop_vector_python_co_body_va_post() {
        let got = aws4_sign(
            "POST",
            "host",
            "/p",
            &map(&[("b", "2"), ("a", "1")]),
            &map(&[("x-amz-date", "20200101T000000Z")]),
            b"hello",
            "AK2",
            "sk2",
            "us-east-1",
            "s3",
            "20200101T000000Z",
        );
        assert_eq!(
            got,
            "AWS4-HMAC-SHA256 Credential=AK2/20200101/us-east-1/s3/aws4_request,\
             SignedHeaders=x-amz-date,\
             Signature=678affba1bc43616727cc1c25bfcb9d8a1c17499c834671a56d7ce6b66ec6c16"
        );
    }
}
