//! `OpenAI` `chat/completions` — key gửi qua header `Authorization: Bearer`, body JSON.

use serde_json::Value;

use crate::error::TextGenError;
use crate::prompt::{build_system, build_user};
use crate::req::TextGenReq;

/// Model mặc định (rẻ). Xem MEMORY.md nếu `OpenAI` đổi tên model.
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_BASE: &str = "https://api.openai.com";

/// Body request (THUẦN — test riêng).
#[must_use]
pub fn body(req: &TextGenReq) -> Value {
    serde_json::json!({
        "model": DEFAULT_MODEL,
        "messages": [
            { "role": "system", "content": build_system(req) },
            { "role": "user", "content": build_user(req) }
        ],
        "temperature": 0.9
    })
}

/// URL `chat/completions` cho `base`.
#[must_use]
pub fn url(base: Option<&str>) -> String {
    format!(
        "{}/v1/chat/completions",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/')
    )
}

/// Gọi `OpenAI`. `api_key` đi qua header `Authorization` (không vào URL/log).
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
        .bearer_auth(api_key)
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

/// Rút text từ `choices[0].message.content`.
fn parse(raw: &str) -> Result<String, TextGenError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| TextGenError::Decode(e.to_string()))?;
    let content = v
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(Value::as_str);
    match content {
        Some(s) if !s.trim().is_empty() => Ok(s.to_string()),
        _ => Err(TextGenError::Empty),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Provider;

    fn req() -> TextGenReq {
        TextGenReq {
            credential_name: "k".into(),
            provider: Provider::OpenAi,
            system: String::new(),
            prompt: "giay chay bo".into(),
            num_scenes: 2,
            language: "vi".into(),
            style: String::new(),
        }
    }

    #[test]
    fn body_co_hai_message() {
        let b = body(&req());
        assert_eq!(b["messages"][0]["role"], "system");
        assert_eq!(b["messages"][1]["role"], "user");
        assert_eq!(b["model"], "gpt-4o-mini");
    }

    #[test]
    fn parse_rut_content() {
        let raw = r#"{"choices":[{"message":{"content":"canh 1\ncanh 2"}}]}"#;
        assert_eq!(parse(raw).unwrap(), "canh 1\ncanh 2");
    }

    #[test]
    fn parse_rong_bao_empty() {
        assert_eq!(parse(r#"{"choices":[]}"#).unwrap_err(), TextGenError::Empty);
    }
}
