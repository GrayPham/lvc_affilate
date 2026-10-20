//! THUẦN: canonical JSON (khớp Python `json.dumps(sort_keys=True)`) + verify chữ ký ECDSA P-256 +
//! 4 lớp guard phản hồi license. Không I/O, không log bí mật.

use base64::Engine as _;
use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey as _;
use serde_json::Value;

/// Canonical JSON KHỚP Python `json.dumps(data, sort_keys=True)` (mặc định `ensure_ascii=True`,
/// `separators=(", ", ": ")`): sort key đệ quy, escape non-ASCII thành `\uXXXX` (chữ thường, surrogate
/// pair cho > U+FFFF), có dấu cách sau `,` và `:`. ĐÂY là chuỗi được ký — phải khớp byte-for-byte.
#[must_use]
pub fn canonical_json(v: &Value) -> String {
    let mut out = String::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_string(s, out),
        Value::Array(a) => {
            out.push('[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Object(m) => {
            // sort_keys: thứ tự code-point. Rust `String` Ord = byte UTF-8 = thứ tự code-point.
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_string(k, out);
                out.push_str(": ");
                write_value(&m[*k], out);
            }
            out.push('}');
        }
    }
}

/// Escape chuỗi khớp `py_encode_basestring_ascii`: điều khiển/đặc biệt có tên, `<0x20` → `\u00xx`,
/// in được ASCII (0x20..=0x7e) giữ nguyên, còn lại → `\uXXXX` (surrogate pair nếu > U+FFFF).
fn write_string(s: &str, out: &mut String) {
    use std::fmt::Write as _;
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if ('\u{20}'..='\u{7e}').contains(&c) => out.push(c),
            c => {
                let cp = c as u32;
                if cp <= 0xFFFF {
                    let _ = write!(out, "\\u{cp:04x}");
                } else {
                    let v = cp - 0x1_0000;
                    let hi = 0xD800 + (v >> 10);
                    let lo = 0xDC00 + (v & 0x3FF);
                    let _ = write!(out, "\\u{hi:04x}\\u{lo:04x}");
                }
            }
        }
    }
    out.push('"');
}

/// Verify chữ ký ECDSA P-256 (SHA-256) của server trên `data`. `signature_b64` = base64 CHUẨN của
/// chữ ký **DER**. Mọi lỗi (base64 hỏng / PEM sai / chữ ký sai / rỗng) → `false` (không mách nước).
#[must_use]
pub fn verify_signature(pubkey_pem: &str, data: &Value, signature_b64: &str) -> bool {
    if signature_b64.is_empty() {
        return false;
    }
    let Ok(sig_der) = base64::engine::general_purpose::STANDARD.decode(signature_b64) else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_public_key_pem(pubkey_pem) else {
        return false;
    };
    let Ok(sig) = Signature::from_der(&sig_der) else {
        return false;
    };
    vk.verify(canonical_json(data).as_bytes(), &sig).is_ok()
}

/// Thông tin license đã tin được (nằm TRONG phần đã ký). Trường thiếu → mặc định (khớp Python
/// `str(None)=""`); trường lạ bị bỏ qua.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LicenseInfo {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub expires_at: String,
    #[serde(default)]
    pub tool_id: Option<String>,
    #[serde(default)]
    pub max_devices: Option<i64>,
    #[serde(default)]
    pub device_id: String,
}

/// Phản hồi không qua được kiểm chứng. Thông điệp cho LOG, KHÔNG cho UI (xem `client`).
#[derive(Debug, Clone, thiserror::Error)]
pub enum GuardError {
    #[error("phan hoi khong phai JSON object")]
    NotObject,
    #[error("phan hoi thieu truong 'data'")]
    MissingData,
    #[error("phan hoi khong co chu ky")]
    NoSignature,
    #[error("chu ky khong hop le")]
    BadSignature,
    #[error("server khong xac nhan license hop le")]
    NotValid,
    #[error("nonce khong khop - nghi phat lai")]
    NonceMismatch,
    #[error("phan hoi thieu 'license_info'")]
    MissingLicenseInfo,
    #[error("giay phep khong thuoc may nay")]
    DeviceMismatch,
    #[error("giay phep khong khop khoa da nhap")]
    KeyMismatch,
    #[error("phan hoi thieu moc thoi gian")]
    MissingTimestamp,
    #[error("moc thoi gian lech qua xa - nghi phan hoi cu")]
    TooOld,
}

/// Đọc mốc ISO về epoch giây UTC (hỗ trợ `Z`, có/không offset, naive = UTC). Khớp `_parse_iso` Veo3.
#[must_use]
pub(crate) fn parse_iso_to_unix(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let s2 = if let Some(stripped) = s.strip_suffix('Z') {
        format!("{stripped}+00:00")
    } else {
        s.to_string()
    };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s2) {
        return Some(dt.timestamp());
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(&s2, fmt) {
            return Some(ndt.and_utc().timestamp());
        }
    }
    None
}

/// 4 lớp guard phản hồi license (ném lỗi, KHÔNG trả bool — khó bị vá thành `return true`). Thứ tự:
/// object → `data` → **chữ ký BẮT BUỘC** → `status`/`license_valid` → nonce → `license_info` →
/// `device_id` → key → thời gian (skew). `now_unix`/`max_skew_secs` tiêm vào để test tất định.
///
/// # Errors
/// [`GuardError`] tương ứng lớp đầu tiên không qua.
pub fn check_license_response(
    result: &Value,
    pubkey_pem: &str,
    expected_nonce: &str,
    expected_device_id: &str,
    expected_key: &str,
    now_unix: i64,
    max_skew_secs: i64,
) -> Result<LicenseInfo, GuardError> {
    if !result.is_object() {
        return Err(GuardError::NotObject);
    }
    let data = result
        .get("data")
        .filter(|d| d.is_object())
        .ok_or(GuardError::MissingData)?;

    // Lớp 1: chữ ký BẮT BUỘC.
    let signature = result
        .get("signature")
        .and_then(Value::as_str)
        .unwrap_or("");
    if signature.is_empty() {
        return Err(GuardError::NoSignature);
    }
    if !verify_signature(pubkey_pem, data, signature) {
        return Err(GuardError::BadSignature);
    }

    // Từ đây mọi trường đều nằm trong phần đã ký.
    let status_ok = data.get("status").and_then(Value::as_str) == Some("success");
    let valid = data
        .get("license_valid")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !status_ok || !valid {
        return Err(GuardError::NotValid);
    }

    // Lớp 2: nonce.
    if !expected_nonce.is_empty() {
        let echoed = data.get("nonce").and_then(Value::as_str).unwrap_or("");
        if echoed != expected_nonce {
            return Err(GuardError::NonceMismatch);
        }
    }

    let info_val = data
        .get("license_info")
        .filter(|v| v.is_object())
        .ok_or(GuardError::MissingLicenseInfo)?;
    let info: LicenseInfo =
        serde_json::from_value(info_val.clone()).map_err(|_| GuardError::MissingLicenseInfo)?;

    // Lớp 3: gắn máy. Lớp 3b: khớp key.
    if !expected_device_id.is_empty() && info.device_id != expected_device_id {
        return Err(GuardError::DeviceMismatch);
    }
    if !expected_key.is_empty() && info.key != expected_key {
        return Err(GuardError::KeyMismatch);
    }

    // Lớp 4: thời gian (skew rộng, KHÔNG chống lùi đồng hồ).
    let ts = data.get("timestamp").and_then(Value::as_str).unwrap_or("");
    let stamped = parse_iso_to_unix(ts).ok_or(GuardError::MissingTimestamp)?;
    if (now_unix - stamped).abs() > max_skew_secs {
        return Err(GuardError::TooOld);
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::{Signature, SigningKey, signature::Signer};
    use p256::pkcs8::EncodePublicKey;
    use serde_json::json;

    /// Ký `data` theo đúng cách server: canonical JSON → ECDSA(SHA256) DER → base64 chuẩn. Trả
    /// `(public_key_pem, signature_b64)`.
    fn sign(data: &serde_json::Value) -> (String, String) {
        let sk = SigningKey::random(&mut rand::rngs::OsRng);
        let pem = sk
            .verifying_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();
        let sig: Signature = sk.sign(canonical_json(data).as_bytes());
        let b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_der().as_bytes());
        (pem, b64)
    }

    #[test]
    fn canonical_sort_va_space() {
        assert_eq!(canonical_json(&json!({"b":1,"a":2})), r#"{"a": 2, "b": 1}"#);
    }

    #[test]
    fn canonical_escape_non_ascii() {
        // Python json.dumps mặc định ensure_ascii=True -> "ăn" => "\u0103n".
        assert_eq!(canonical_json(&json!({"n":"ăn"})), "{\"n\": \"\\u0103n\"}");
    }

    #[test]
    fn canonical_nested_sort() {
        assert_eq!(
            canonical_json(&json!({"z":[{"y":1,"x":2}]})),
            r#"{"z": [{"x": 2, "y": 1}]}"#
        );
    }

    #[test]
    fn verify_chu_ky_dung_true() {
        let data = json!({"status":"success","nonce":"abc"});
        let (pem, sig) = sign(&data);
        assert!(verify_signature(&pem, &data, &sig));
    }

    #[test]
    fn verify_chu_ky_sai_false() {
        let data = json!({"status":"success"});
        let (pem, sig) = sign(&data);
        let tampered = json!({"status":"HACKED"});
        assert!(!verify_signature(&pem, &tampered, &sig));
    }

    #[test]
    fn verify_signature_rong_false() {
        let data = json!({"a":1});
        let (pem, _) = sign(&data);
        assert!(!verify_signature(&pem, &data, ""));
    }

    // ── Task 2: 4 lớp guard ──
    const NOW: i64 = 1_700_000_000;
    const TS: &str = "2023-11-14T22:13:20Z"; // == NOW

    /// Bọc `data` thành phản hồi `{data, signature}` đã ký; trả `(pem, result)`.
    fn make_response(data: &serde_json::Value) -> (String, serde_json::Value) {
        let sk = SigningKey::random(&mut rand::rngs::OsRng);
        let pem = sk
            .verifying_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();
        let sig: Signature = sk.sign(canonical_json(data).as_bytes());
        let b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_der().as_bytes());
        (pem, json!({ "data": data.clone(), "signature": b64 }))
    }

    fn ok_data() -> serde_json::Value {
        json!({
            "status": "success", "license_valid": true, "nonce": "N1", "timestamp": TS,
            "license_info": { "key": "KEY1", "device_id": "DEV1", "expires_at": "2027-01-01T00:00:00Z", "max_devices": 1 }
        })
    }

    fn check(pem: &str, result: &serde_json::Value) -> Result<LicenseInfo, GuardError> {
        check_license_response(result, pem, "N1", "DEV1", "KEY1", NOW, 86400)
    }

    #[test]
    fn guard_ok_tra_license_info() {
        let (pem, result) = make_response(&ok_data());
        let info = check(&pem, &result).unwrap();
        assert_eq!(info.key, "KEY1");
        assert_eq!(info.device_id, "DEV1");
    }

    #[test]
    fn guard_thieu_signature_loai() {
        let (pem, mut result) = make_response(&ok_data());
        result.as_object_mut().unwrap().remove("signature");
        assert!(matches!(check(&pem, &result), Err(GuardError::NoSignature)));
    }

    #[test]
    fn guard_nonce_sai_loai() {
        let (pem, result) = make_response(&ok_data());
        let r = check_license_response(&result, &pem, "WRONG", "DEV1", "KEY1", NOW, 86400);
        assert!(matches!(r, Err(GuardError::NonceMismatch)));
    }

    #[test]
    fn guard_device_sai_loai() {
        let (pem, result) = make_response(&ok_data());
        let r = check_license_response(&result, &pem, "N1", "OTHER", "KEY1", NOW, 86400);
        assert!(matches!(r, Err(GuardError::DeviceMismatch)));
    }

    #[test]
    fn guard_key_sai_loai() {
        let (pem, result) = make_response(&ok_data());
        let r = check_license_response(&result, &pem, "N1", "DEV1", "OTHER", NOW, 86400);
        assert!(matches!(r, Err(GuardError::KeyMismatch)));
    }

    #[test]
    fn guard_timestamp_qua_cu_loai() {
        let (pem, result) = make_response(&ok_data());
        // now lệch 90000s > 86400 -> TooOld
        let r = check_license_response(&result, &pem, "N1", "DEV1", "KEY1", NOW + 90_000, 86400);
        assert!(matches!(r, Err(GuardError::TooOld)));
        // trong ngưỡng -> Ok
        assert!(
            check_license_response(&result, &pem, "N1", "DEV1", "KEY1", NOW + 1000, 86400).is_ok()
        );
    }
}
