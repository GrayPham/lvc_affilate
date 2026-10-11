//! Tạo ảnh Seedream — `POST {base}/api/v3/images/generations`, `Authorization: Bearer <KEY>`. Ép
//! `response_format=b64_json` để nhận bytes trực tiếp (không phải tải URL tạm). Phản hồi
//! `data[].b64_json`. Xem MEMORY.md.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use crate::error::SeedanceError;
use crate::req::{ImageReq, default_model};

const DEFAULT_BASE: &str = "https://ark.cn-beijing.volces.com";

fn model_of(req: &ImageReq) -> &str {
    let m = req.model.trim();
    if m.is_empty() {
        default_model::IMAGE
    } else {
        m
    }
}

/// URL `images/generations` cho `base`.
#[must_use]
pub fn url(base: Option<&str>) -> String {
    format!(
        "{}/api/v3/images/generations",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/')
    )
}

/// Body request (THUẦN).
#[must_use]
pub fn body(req: &ImageReq) -> Value {
    serde_json::json!({
        "model": model_of(req),
        "prompt": req.prompt,
        "size": req.aspect.as_size(),
        "response_format": "b64_json",
        "watermark": false
    })
}

/// Gọi Seedream, trả về bytes ảnh (giải mã base64).
///
/// # Errors
/// [`SeedanceError`] theo mã HTTP / phản hồi.
pub async fn generate(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    req: &ImageReq,
) -> Result<Vec<u8>, SeedanceError> {
    let resp = http
        .post(url(base))
        .bearer_auth(api_key)
        .json(&body(req))
        .send()
        .await
        .map_err(|e| SeedanceError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| SeedanceError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse(&text)
}

pub(crate) fn parse(raw: &str) -> Result<Vec<u8>, SeedanceError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| SeedanceError::Decode(e.to_string()))?;
    let b64 = v
        .pointer("/data/0/b64_json")
        .and_then(Value::as_str)
        .ok_or(SeedanceError::Empty)?;
    STANDARD
        .decode(b64)
        .map_err(|e| SeedanceError::Decode(format!("base64 anh: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Aspect;

    fn req() -> ImageReq {
        ImageReq {
            model: String::new(),
            prompt: "a cat".into(),
            aspect: Aspect::Square,
        }
    }

    #[test]
    fn body_co_b64_va_size() {
        let b = body(&req());
        assert_eq!(b["response_format"], "b64_json");
        assert_eq!(b["size"], "1024x1024");
        assert_eq!(b["prompt"], "a cat");
    }

    #[test]
    fn parse_giai_ma_base64() {
        assert_eq!(parse(r#"{"data":[{"b64_json":"aGk="}]}"#).unwrap(), b"hi");
    }

    #[test]
    fn parse_rong_bao_empty() {
        assert_eq!(parse(r#"{"data":[]}"#).unwrap_err(), SeedanceError::Empty);
    }
}
