//! Gemini `generateContent` — key gửi qua query `?key=` (không header), body JSON. Xem MEMORY.md.

use serde_json::Value;

use crate::error::TextGenError;
use crate::prompt::{build_system, build_user};
use crate::req::TextGenReq;

/// Model mặc định (rẻ, đủ tốt cho sinh kịch bản ngắn). Xem MEMORY.md nếu Google đổi tên model.
const DEFAULT_MODEL: &str = "gemini-2.0-flash";
const DEFAULT_BASE: &str = "https://generativelanguage.googleapis.com";

/// Body request (THUẦN — test riêng, không lộ key vì key đi ở query).
#[must_use]
pub fn body(req: &TextGenReq) -> Value {
    serde_json::json!({
        "system_instruction": { "parts": [ { "text": build_system(req) } ] },
        "contents": [ { "role": "user", "parts": [ { "text": build_user(req) } ] } ],
        "generationConfig": { "temperature": 0.9 }
    })
}

/// URL `generateContent` cho `base` (không chứa key — key thêm ở query lúc gửi).
#[must_use]
pub fn url(base: Option<&str>) -> String {
    format!(
        "{}/v1beta/models/{DEFAULT_MODEL}:generateContent",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/')
    )
}

/// Gọi Gemini. `api_key` đi qua query `?key=` (KHÔNG log URL — xem `send` ở client).
///
/// # Errors
/// [`TextGenError`] theo mã HTTP / phản hồi.
pub async fn generate(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    req: &TextGenReq,
) -> Result<String, TextGenError> {
    let resp = http
        .post(url(base))
        .query(&[("key", api_key)])
        .json(&body(req))
        .send()
        .await
        .map_err(|e| TextGenError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| TextGenError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse(&text)
}

/// Rút text từ `candidates[0].content.parts[*].text`.
fn parse(raw: &str) -> Result<String, TextGenError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| TextGenError::Decode(e.to_string()))?;
    let parts = v
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(Value::as_array);
    let Some(parts) = parts else {
        return Err(TextGenError::Empty);
    };
    let joined: String = parts
        .iter()
        .filter_map(|p| p.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    if joined.trim().is_empty() {
        Err(TextGenError::Empty)
    } else {
        Ok(joined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Provider;

    fn req() -> TextGenReq {
        TextGenReq {
            credential_name: "k".into(),
            provider: Provider::Gemini,
            system: String::new(),
            prompt: "giay chay bo".into(),
            num_scenes: 2,
            language: "vi".into(),
            style: String::new(),
        }
    }

    #[test]
    fn body_co_system_va_user() {
        let b = body(&req());
        assert!(b["system_instruction"]["parts"][0]["text"].is_string());
        assert_eq!(
            b["contents"][0]["parts"][0]["text"],
            "Chu de/san pham: giay chay bo"
        );
    }

    #[test]
    fn parse_rut_text_ghep_parts() {
        let raw = r#"{"candidates":[{"content":{"parts":[{"text":"canh 1\ncanh 2"}]}}]}"#;
        assert_eq!(parse(raw).unwrap(), "canh 1\ncanh 2");
    }

    #[test]
    fn parse_rong_bao_empty() {
        assert_eq!(
            parse(r#"{"candidates":[]}"#).unwrap_err(),
            TextGenError::Empty
        );
    }
}
