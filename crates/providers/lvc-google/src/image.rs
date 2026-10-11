//! Tạo ảnh **Nano Banana** (Gemini image) — `POST /v1beta/models/<model>:generateContent`, header
//! `x-goog-api-key: <KEY>`. Imagen `:predict` ĐÃ BỊ GOOGLE TẮT trên Gemini Developer API (xác minh
//! 2026-10-01, xem MEMORY.md + spec `docs/superpowers/specs/2026-10-01-gemini-api-media-nodes-design.md`),
//! nên dùng Nano Banana thay thế. Body: `contents[].parts[].text` + `generationConfig.
//! {responseModalities:["IMAGE"], imageConfig:{aspectRatio,imageSize}}`; phản hồi
//! `candidates[0].content.parts[].inlineData.data` (base64).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use crate::error::GoogleError;
use crate::req::{ImageReq, default_model};

const DEFAULT_BASE: &str = "https://generativelanguage.googleapis.com";
/// `imageConfig.imageSize` mặc định (K viết HOA). Nano Banana 3.1-flash hỗ trợ tới 4K; để 2K cho
/// cân bằng chất lượng/chi phí. Model `*-lite-image` chỉ 1K sẽ tự hạ — xem MEMORY.md.
const DEFAULT_IMAGE_SIZE: &str = "2K";

/// Một ảnh đã giải mã từ phản hồi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub bytes: Vec<u8>,
    /// Mime theo phản hồi (vd `image/png`); rỗng = không rõ.
    pub mime: String,
}

fn model_of(req: &ImageReq) -> &str {
    let m = req.model.trim();
    if m.is_empty() {
        default_model::IMAGE
    } else {
        m
    }
}

/// URL `:generateContent` cho `base` + model (KHÔNG chứa key — key đi ở header).
#[must_use]
pub fn url(base: Option<&str>, req: &ImageReq) -> String {
    format!(
        "{}/v1beta/models/{}:generateContent",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/'),
        model_of(req)
    )
}

/// Body request (THUẦN — test riêng, không chứa key).
#[must_use]
pub fn body(req: &ImageReq) -> Value {
    serde_json::json!({
        "contents": [ { "parts": [ { "text": req.prompt } ] } ],
        "generationConfig": {
            "responseModalities": ["IMAGE"],
            "imageConfig": {
                "aspectRatio": req.aspect.as_ratio(),
                "imageSize": DEFAULT_IMAGE_SIZE
            }
        }
    })
}

/// Gọi Nano Banana, trả về các ảnh đã giải mã base64.
///
/// # Errors
/// [`GoogleError`] theo mã HTTP / phản hồi.
pub async fn generate(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    req: &ImageReq,
) -> Result<Vec<DecodedImage>, GoogleError> {
    let resp = http
        .post(url(base, req))
        .header("x-goog-api-key", api_key)
        .json(&body(req))
        .send()
        .await
        .map_err(|e| GoogleError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| GoogleError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse(&text)
}

/// Rút `candidates[0].content.parts[].inlineData.data` (+ `mimeType`) và giải mã base64.
pub(crate) fn parse(raw: &str) -> Result<Vec<DecodedImage>, GoogleError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| GoogleError::Decode(e.to_string()))?;
    let parts = v
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .ok_or(GoogleError::Empty)?;
    let mut out = Vec::new();
    for p in parts {
        let Some(data) = p.pointer("/inlineData/data").and_then(Value::as_str) else {
            continue;
        };
        let bytes = STANDARD
            .decode(data)
            .map_err(|e| GoogleError::Decode(format!("base64 anh: {e}")))?;
        let mime = p
            .pointer("/inlineData/mimeType")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        out.push(DecodedImage { bytes, mime });
    }
    if out.is_empty() {
        Err(GoogleError::Empty)
    } else {
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Aspect;

    fn req() -> ImageReq {
        ImageReq {
            model: String::new(),
            prompt: "a calico cat".into(),
            aspect: Aspect::Portrait,
            sample_count: 1,
        }
    }

    #[test]
    fn url_dung_model_mac_dinh_generatecontent() {
        assert!(url(None, &req()).ends_with("/models/gemini-3.1-flash-image:generateContent"));
    }

    #[test]
    fn body_co_prompt_va_image_modality() {
        let b = body(&req());
        assert_eq!(b["contents"][0]["parts"][0]["text"], "a calico cat");
        assert_eq!(b["generationConfig"]["responseModalities"][0], "IMAGE");
        assert_eq!(b["generationConfig"]["imageConfig"]["aspectRatio"], "9:16");
    }

    #[test]
    fn parse_giai_ma_inline_data() {
        // "hi" => base64 "aGk="
        let raw = r#"{"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"image/png","data":"aGk="}}]}}]}"#;
        let imgs = parse(raw).unwrap();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].bytes, b"hi");
        assert_eq!(imgs[0].mime, "image/png");
    }

    #[test]
    fn parse_rong_bao_empty() {
        assert_eq!(
            parse(r#"{"candidates":[{"content":{"parts":[{"text":"xin loi"}]}}]}"#).unwrap_err(),
            GoogleError::Empty
        );
    }
}
