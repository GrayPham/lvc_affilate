//! Tạo video Veo — bất đối xứng: `:predictLongRunning` trả về TÊN operation, rồi GET operation tới
//! khi `done`, lấy `response.generateVideoResponse.generatedSamples[].video.uri`. Key ở QUERY
//! `?key=`. Tải URI video cũng phải kèm `?key=` (xem `client::download_video`). Xem MEMORY.md.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use crate::error::GoogleError;
use crate::req::{VideoReq, default_model};

const DEFAULT_BASE: &str = "https://generativelanguage.googleapis.com";

/// Kết quả một lần poll operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoPoll {
    pub done: bool,
    /// URI video (khi `done` và thành công) — cần thêm `?key=` để tải.
    pub uri: Option<String>,
}

fn model_of(req: &VideoReq) -> &str {
    let m = req.model.trim();
    if m.is_empty() {
        default_model::VIDEO
    } else {
        m
    }
}

/// URL `:predictLongRunning` cho `base` + model (không chứa key).
#[must_use]
pub fn submit_url(base: Option<&str>, req: &VideoReq) -> String {
    format!(
        "{}/v1beta/models/{}:predictLongRunning",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/'),
        model_of(req)
    )
}

/// URL GET operation theo `name` (dạng `models/.../operations/...`).
#[must_use]
pub fn poll_url(base: Option<&str>, operation_name: &str) -> String {
    format!(
        "{}/v1beta/{}",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/'),
        operation_name.trim_start_matches('/')
    )
}

/// Body submit (THUẦN). i2v: thêm `instances[].image.{bytesBase64Encoded,mimeType}`.
#[must_use]
pub fn submit_body(req: &VideoReq) -> Value {
    let mut instance = serde_json::json!({ "prompt": req.prompt });
    if let Some(img) = &req.image {
        instance["image"] = serde_json::json!({
            "bytesBase64Encoded": STANDARD.encode(&img.bytes),
            "mimeType": img.mime,
        });
    }
    serde_json::json!({
        "instances": [ instance ],
        "parameters": {
            "sampleCount": req.sample_count.max(1),
            "aspectRatio": req.aspect.as_ratio()
        }
    })
}

/// Submit, trả về TÊN operation (`name`).
///
/// # Errors
/// [`GoogleError`] theo mã HTTP / phản hồi (thiếu `name` = [`GoogleError::Empty`]).
pub async fn submit(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    req: &VideoReq,
) -> Result<String, GoogleError> {
    let resp = http
        .post(submit_url(base, req))
        .header("x-goog-api-key", api_key)
        .json(&submit_body(req))
        .send()
        .await
        .map_err(|e| GoogleError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| GoogleError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse_submit(&text)
}

/// Poll một lần.
///
/// # Errors
/// [`GoogleError`] theo mã HTTP / phản hồi, hoặc [`GoogleError::Operation`] khi operation báo lỗi.
pub async fn poll_once(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    operation_name: &str,
) -> Result<VideoPoll, GoogleError> {
    let resp = http
        .get(poll_url(base, operation_name))
        .header("x-goog-api-key", api_key)
        .send()
        .await
        .map_err(|e| GoogleError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| GoogleError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse_poll(&text)
}

pub(crate) fn parse_submit(raw: &str) -> Result<String, GoogleError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| GoogleError::Decode(e.to_string()))?;
    v.get("name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or(GoogleError::Empty)
}

pub(crate) fn parse_poll(raw: &str) -> Result<VideoPoll, GoogleError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| GoogleError::Decode(e.to_string()))?;
    if let Some(err) = v.get("error") {
        let msg = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("operation loi");
        return Err(GoogleError::Operation(msg.chars().take(300).collect()));
    }
    let done = v.get("done").and_then(Value::as_bool).unwrap_or(false);
    if !done {
        return Ok(VideoPoll {
            done: false,
            uri: None,
        });
    }
    // response.generateVideoResponse.generatedSamples[0].video.uri
    let uri = v
        .pointer("/response/generateVideoResponse/generatedSamples/0/video/uri")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(VideoPoll { done: true, uri })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Aspect;

    fn req() -> VideoReq {
        VideoReq {
            model: String::new(),
            prompt: "a cat on a roof".into(),
            aspect: Aspect::Landscape,
            sample_count: 1,
            image: None,
        }
    }

    #[test]
    fn submit_url_dung_model_mac_dinh() {
        assert!(submit_url(None, &req()).ends_with("veo-3.1-generate-preview:predictLongRunning"));
    }

    #[test]
    fn submit_body_t2v_khong_co_image() {
        let b = submit_body(&req());
        assert_eq!(b["instances"][0]["prompt"], "a cat on a roof");
        assert!(b["instances"][0].get("image").is_none());
        assert_eq!(b["parameters"]["aspectRatio"], "16:9");
    }

    #[test]
    fn submit_body_i2v_co_image_base64() {
        let mut r = req();
        r.image = Some(crate::req::ImageInput {
            bytes: b"hi".to_vec(),
            mime: "image/png".into(),
        });
        let b = submit_body(&r);
        assert_eq!(b["instances"][0]["image"]["bytesBase64Encoded"], "aGk=");
        assert_eq!(b["instances"][0]["image"]["mimeType"], "image/png");
    }

    #[test]
    fn parse_submit_lay_name() {
        assert_eq!(
            parse_submit(r#"{"name":"models/veo/operations/abc"}"#).unwrap(),
            "models/veo/operations/abc"
        );
    }

    #[test]
    fn parse_poll_chua_xong() {
        assert_eq!(
            parse_poll(r#"{"done":false}"#).unwrap(),
            VideoPoll {
                done: false,
                uri: None
            }
        );
    }

    #[test]
    fn parse_poll_xong_co_uri() {
        let raw = r#"{"done":true,"response":{"generateVideoResponse":{"generatedSamples":[{"video":{"uri":"https://x/v.mp4"}}]}}}"#;
        assert_eq!(
            parse_poll(raw).unwrap(),
            VideoPoll {
                done: true,
                uri: Some("https://x/v.mp4".into())
            }
        );
    }

    #[test]
    fn parse_poll_loi_bao_operation() {
        let err = parse_poll(r#"{"error":{"message":"quota"}}"#).unwrap_err();
        assert!(matches!(err, GoogleError::Operation(_)));
    }
}
