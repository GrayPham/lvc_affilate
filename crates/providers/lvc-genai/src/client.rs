//! Client HTTP + điều phối theo provider. Giữ `reqwest::Client` (pool kết nối); API key CHỈ đi qua
//! tham số hàm, KHÔNG lưu vào struct, KHÔNG log.

use crate::error::TextGenError;
use crate::req::{Provider, TextGenReq};
use crate::{gemini, openai};

/// Số ký tự tối đa của thân phản hồi lỗi giữ lại trong [`TextGenError::Http`] (tránh log dài + lỡ
/// dính dữ liệu nhạy cảm).
const DETAIL_MAX: usize = 300;

/// Client sinh text dùng chung. Rẻ để `clone` (bọc `reqwest::Client`).
#[derive(Debug, Clone)]
pub struct GenAiClient {
    http: reqwest::Client,
}

impl Default for GenAiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GenAiClient {
    /// Client mặc định (reqwest với cấu hình chuẩn của workspace).
    #[must_use]
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Dùng sẵn một `reqwest::Client` (vd caller muốn cắm proxy/timeout).
    #[must_use]
    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Sinh text với `api_key` cho sẵn (engine đã resolve từ vault). `base_url` `None` = endpoint
    /// chính thức. KHÔNG log key/prompt.
    ///
    /// # Errors
    /// [`TextGenError::BadRequest`] khi tham số sai; các biến thể khác theo phản hồi API.
    pub async fn generate_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        req: &TextGenReq,
    ) -> Result<String, TextGenError> {
        if api_key.trim().is_empty() {
            return Err(TextGenError::Credential("API key rong".to_string()));
        }
        if req.num_scenes == 0 {
            return Err(TextGenError::BadRequest("num_scenes phai >= 1".to_string()));
        }
        if req.prompt.trim().is_empty() {
            return Err(TextGenError::BadRequest("thieu chu de/brief".to_string()));
        }
        match req.provider {
            Provider::Gemini => gemini::generate(&self.http, api_key, base_url, req).await,
            Provider::OpenAi => openai::generate(&self.http, api_key, base_url, req).await,
        }
    }
}

/// Ánh xạ mã HTTP -> [`TextGenError`] (dùng chung Gemini/`OpenAI`). `body` chỉ dùng cho `detail` khi
/// lỗi khác 401/403/429 và được CẮT NGẮN — không log nguyên văn.
///
/// # Errors
/// [`TextGenError`] khi `status` không phải 2xx.
pub fn check_status(status: u16, body: &str) -> Result<(), TextGenError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(TextGenError::Auth(status)),
        429 => Err(TextGenError::Quota),
        s => Err(TextGenError::Http {
            status: s,
            detail: clip(body, DETAIL_MAX),
        }),
    }
}

/// Cắt theo ký tự (không cắt giữa code point) + gỡ khoảng trắng thừa.
fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status_phan_loai_dung() {
        assert!(check_status(200, "").is_ok());
        assert_eq!(check_status(401, "x").unwrap_err(), TextGenError::Auth(401));
        assert_eq!(check_status(403, "x").unwrap_err(), TextGenError::Auth(403));
        assert_eq!(check_status(429, "x").unwrap_err(), TextGenError::Quota);
        match check_status(500, "loi may chu").unwrap_err() {
            TextGenError::Http { status, detail } => {
                assert_eq!(status, 500);
                assert_eq!(detail, "loi may chu");
            }
            other => panic!("mong Http, gap {other:?}"),
        }
    }
}
