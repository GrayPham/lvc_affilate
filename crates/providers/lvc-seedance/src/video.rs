//! Tạo video Seedance — bất đối xứng: `POST {base}/api/v3/contents/generations/tasks` trả `id`,
//! rồi `GET .../tasks/{id}` tới khi `status=succeeded`, lấy `content.video_url`. Tỉ lệ khung đưa
//! vào `content.text` dạng lệnh `--ratio 16:9`. Auth header `Bearer`. Xem MEMORY.md.

use serde_json::Value;

use crate::error::SeedanceError;
use crate::req::{VideoReq, default_model};

const DEFAULT_BASE: &str = "https://ark.cn-beijing.volces.com";

/// Kết quả một lần poll task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoPoll {
    pub done: bool,
    /// URL video (khi `done`) — tải bằng GET thường (không cần key).
    pub url: Option<String>,
}

fn model_of(req: &VideoReq) -> &str {
    let m = req.model.trim();
    if m.is_empty() {
        default_model::VIDEO
    } else {
        m
    }
}

/// URL tạo task.
#[must_use]
pub fn submit_url(base: Option<&str>) -> String {
    format!(
        "{}/api/v3/contents/generations/tasks",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/')
    )
}

/// URL GET task theo `id`.
#[must_use]
pub fn poll_url(base: Option<&str>, task_id: &str) -> String {
    format!(
        "{}/api/v3/contents/generations/tasks/{task_id}",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/')
    )
}

/// Body submit (THUẦN). i2v: thêm phần tử `image_url`.
#[must_use]
pub fn submit_body(req: &VideoReq) -> Value {
    let text = format!("{} --ratio {}", req.prompt.trim(), req.aspect.as_ratio());
    let mut content = vec![serde_json::json!({ "type": "text", "text": text })];
    if let Some(url) = &req.image_url {
        content.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": url }
        }));
    }
    serde_json::json!({ "model": model_of(req), "content": content })
}

/// Submit, trả về `id` task.
///
/// # Errors
/// [`SeedanceError`] theo mã HTTP / phản hồi.
pub async fn submit(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    req: &VideoReq,
) -> Result<String, SeedanceError> {
    let resp = http
        .post(submit_url(base))
        .bearer_auth(api_key)
        .json(&submit_body(req))
        .send()
        .await
        .map_err(|e| SeedanceError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| SeedanceError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse_submit(&text)
}

/// Poll một lần.
///
/// # Errors
/// [`SeedanceError`] theo phản hồi / [`SeedanceError::Task`] khi `status=failed`.
pub async fn poll_once(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    task_id: &str,
) -> Result<VideoPoll, SeedanceError> {
    let resp = http
        .get(poll_url(base, task_id))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| SeedanceError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| SeedanceError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse_poll(&text)
}

pub(crate) fn parse_submit(raw: &str) -> Result<String, SeedanceError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| SeedanceError::Decode(e.to_string()))?;
    v.get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or(SeedanceError::Empty)
}

pub(crate) fn parse_poll(raw: &str) -> Result<VideoPoll, SeedanceError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| SeedanceError::Decode(e.to_string()))?;
    let status = v
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    match status.as_str() {
        "succeeded" => {
            let url = v
                .pointer("/content/video_url")
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok(VideoPoll { done: true, url })
        }
        "failed" | "cancelled" | "canceled" => {
            let msg = v
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or(&status);
            Err(SeedanceError::Task(msg.chars().take(300).collect()))
        }
        // queued | running | rỗng -> chưa xong
        _ => Ok(VideoPoll {
            done: false,
            url: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Aspect;

    fn req() -> VideoReq {
        VideoReq {
            model: String::new(),
            prompt: "a cat".into(),
            aspect: Aspect::Landscape,
            image_url: None,
        }
    }

    #[test]
    fn submit_body_t2v_co_ratio_trong_text() {
        let b = submit_body(&req());
        assert_eq!(b["content"][0]["text"], "a cat --ratio 16:9");
        assert_eq!(b["content"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn submit_body_i2v_them_image_url() {
        let mut r = req();
        r.image_url = Some("https://x/a.png".into());
        let b = submit_body(&r);
        assert_eq!(b["content"][1]["type"], "image_url");
        assert_eq!(b["content"][1]["image_url"]["url"], "https://x/a.png");
    }

    #[test]
    fn parse_submit_lay_id() {
        assert_eq!(parse_submit(r#"{"id":"cgt-1"}"#).unwrap(), "cgt-1");
    }

    #[test]
    fn parse_poll_running_chua_xong() {
        assert_eq!(
            parse_poll(r#"{"status":"running"}"#).unwrap(),
            VideoPoll {
                done: false,
                url: None
            }
        );
    }

    #[test]
    fn parse_poll_succeeded_co_url() {
        let raw = r#"{"status":"succeeded","content":{"video_url":"https://x/v.mp4"}}"#;
        assert_eq!(
            parse_poll(raw).unwrap(),
            VideoPoll {
                done: true,
                url: Some("https://x/v.mp4".into())
            }
        );
    }

    #[test]
    fn parse_poll_failed_bao_task() {
        let err = parse_poll(r#"{"status":"failed","error":{"message":"loi"}}"#).unwrap_err();
        assert!(matches!(err, SeedanceError::Task(_)));
    }
}
