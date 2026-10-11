//! Client HTTP + điều phối. API key CHỈ qua tham số hàm, KHÔNG lưu/KHÔNG log.

use crate::error::GrokError;
use crate::image;
use crate::req::ImageReq;

const DETAIL_MAX: usize = 300;

/// Client Grok dùng chung. Rẻ để `clone`.
#[derive(Debug, Clone)]
pub struct GrokClient {
    http: reqwest::Client,
}

impl Default for GrokClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    #[must_use]
    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Tạo ảnh Grok → bytes.
    ///
    /// # Errors
    /// [`GrokError`] khi tham số sai / API lỗi.
    pub async fn generate_image_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        req: &ImageReq,
    ) -> Result<Vec<u8>, GrokError> {
        if api_key.trim().is_empty() {
            return Err(GrokError::Credential("API key rong".to_string()));
        }
        if req.prompt.trim().is_empty() {
            return Err(GrokError::BadRequest("prompt rong".to_string()));
        }
        image::generate(&self.http, api_key, base_url, req).await
    }
}

/// Ánh xạ mã HTTP -> [`GrokError`]. `body` CẮT NGẮN — không log nguyên văn.
///
/// # Errors
/// [`GrokError`] khi `status` không phải 2xx.
pub fn check_status(status: u16, body: &str) -> Result<(), GrokError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(GrokError::Auth(status)),
        429 => Err(GrokError::Quota),
        s => Err(GrokError::Http {
            status: s,
            detail: body.trim().chars().take(DETAIL_MAX).collect(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status_phan_loai_dung() {
        assert!(check_status(200, "").is_ok());
        assert_eq!(check_status(401, "x").unwrap_err(), GrokError::Auth(401));
        assert_eq!(check_status(429, "x").unwrap_err(), GrokError::Quota);
        match check_status(500, "loi").unwrap_err() {
            GrokError::Http { status, detail } => {
                assert_eq!(status, 500);
                assert_eq!(detail, "loi");
            }
            other => panic!("mong Http, gap {other:?}"),
        }
    }
}
