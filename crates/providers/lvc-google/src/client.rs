//! Client HTTP + điều phối. Giữ `reqwest::Client` (pool kết nối); API key CHỈ đi qua tham số hàm,
//! KHÔNG lưu vào struct, KHÔNG log.

use crate::error::GoogleError;
use crate::image::DecodedImage;
use crate::req::{ImageReq, VideoReq};
use crate::video::VideoPoll;
use crate::{image, video};

/// Số ký tự tối đa giữ lại của thân lỗi (tránh log dài + lỡ dính dữ liệu).
const DETAIL_MAX: usize = 300;

/// Client Google dùng chung. Rẻ để `clone`.
#[derive(Debug, Clone)]
pub struct GoogleClient {
    http: reqwest::Client,
}

impl Default for GoogleClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GoogleClient {
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

    /// Tạo ảnh Imagen với `api_key`. `base_url` `None` = endpoint chính thức.
    ///
    /// # Errors
    /// [`GoogleError`] khi tham số sai / API lỗi.
    pub async fn generate_image_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        req: &ImageReq,
    ) -> Result<Vec<DecodedImage>, GoogleError> {
        validate_key(api_key)?;
        if req.prompt.trim().is_empty() {
            return Err(GoogleError::BadRequest("prompt rong".to_string()));
        }
        image::generate(&self.http, api_key, base_url, req).await
    }

    /// Submit tạo video Veo → trả về TÊN operation.
    ///
    /// # Errors
    /// [`GoogleError`] khi tham số sai / API lỗi.
    pub async fn submit_video_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        req: &VideoReq,
    ) -> Result<String, GoogleError> {
        validate_key(api_key)?;
        if req.prompt.trim().is_empty() {
            return Err(GoogleError::BadRequest("prompt rong".to_string()));
        }
        video::submit(&self.http, api_key, base_url, req).await
    }

    /// Poll operation video một lần.
    ///
    /// # Errors
    /// [`GoogleError`] theo phản hồi / operation lỗi.
    pub async fn poll_video_once_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        operation_name: &str,
    ) -> Result<VideoPoll, GoogleError> {
        validate_key(api_key)?;
        video::poll_once(&self.http, api_key, base_url, operation_name).await
    }

    /// Tải bytes video từ `uri` — Veo yêu cầu kèm header `x-goog-api-key` (URI KHÔNG tự ký) và có
    /// redirect sang CDN (reqwest tự follow mặc định). Trả về bytes để caller (engine) ghi file.
    ///
    /// # Errors
    /// [`GoogleError::Network`]/[`GoogleError::Http`] khi tải lỗi.
    pub async fn download_video_with_key(
        &self,
        api_key: &str,
        uri: &str,
    ) -> Result<Vec<u8>, GoogleError> {
        validate_key(api_key)?;
        let resp = self
            .http
            .get(uri)
            .header("x-goog-api-key", api_key)
            .send()
            .await
            .map_err(|e| GoogleError::Network(e.without_url().to_string()))?;
        let status = resp.status().as_u16();
        if !(200..=299).contains(&status) {
            return Err(GoogleError::Http {
                status,
                detail: "tai video that bai".to_string(),
            });
        }
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| GoogleError::Network(e.without_url().to_string()))
    }
}

fn validate_key(api_key: &str) -> Result<(), GoogleError> {
    if api_key.trim().is_empty() {
        Err(GoogleError::Credential("API key rong".to_string()))
    } else {
        Ok(())
    }
}

/// Ánh xạ mã HTTP -> [`GoogleError`] (dùng chung ảnh/video). `body` CẮT NGẮN — không log nguyên văn.
///
/// # Errors
/// [`GoogleError`] khi `status` không phải 2xx.
pub fn check_status(status: u16, body: &str) -> Result<(), GoogleError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(GoogleError::Auth(status)),
        429 => Err(GoogleError::Quota),
        s => Err(GoogleError::Http {
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
        assert_eq!(check_status(401, "x").unwrap_err(), GoogleError::Auth(401));
        assert_eq!(check_status(429, "x").unwrap_err(), GoogleError::Quota);
        match check_status(500, "loi").unwrap_err() {
            GoogleError::Http { status, detail } => {
                assert_eq!(status, 500);
                assert_eq!(detail, "loi");
            }
            other => panic!("mong Http, gap {other:?}"),
        }
    }
}
