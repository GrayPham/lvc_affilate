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
}
