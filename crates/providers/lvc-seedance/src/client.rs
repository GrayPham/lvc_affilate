//! Client HTTP + điều phối. Giữ `reqwest::Client`; API key CHỈ qua tham số hàm, KHÔNG lưu/KHÔNG log.

use crate::error::SeedanceError;
use crate::req::{ImageReq, VideoReq};
use crate::video::VideoPoll;
use crate::{image, video};

const DETAIL_MAX: usize = 300;

/// Client Seedance/Seedream dùng chung. Rẻ để `clone`.
#[derive(Debug, Clone)]
pub struct SeedanceClient {
    http: reqwest::Client,
}

impl Default for SeedanceClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SeedanceClient {
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

    /// Tạo ảnh Seedream → bytes.
    ///
    /// # Errors
    /// [`SeedanceError`] khi tham số sai / API lỗi.
    pub async fn generate_image_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        req: &ImageReq,
    ) -> Result<Vec<u8>, SeedanceError> {
        validate_key(api_key)?;
        if req.prompt.trim().is_empty() {
            return Err(SeedanceError::BadRequest("prompt rong".to_string()));
        }
        image::generate(&self.http, api_key, base_url, req).await
    }

    /// Submit task video → `id`.
    ///
    /// # Errors
    /// [`SeedanceError`] khi tham số sai / API lỗi.
    pub async fn submit_video_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        req: &VideoReq,
    ) -> Result<String, SeedanceError> {
        validate_key(api_key)?;
        if req.prompt.trim().is_empty() {
            return Err(SeedanceError::BadRequest("prompt rong".to_string()));
        }
        video::submit(&self.http, api_key, base_url, req).await
    }

    /// Poll task video một lần.
    ///
    /// # Errors
    /// [`SeedanceError`] theo phản hồi / task lỗi.
    pub async fn poll_video_once_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        task_id: &str,
    ) -> Result<VideoPoll, SeedanceError> {
        validate_key(api_key)?;
        video::poll_once(&self.http, api_key, base_url, task_id).await
    }

    /// Tải bytes từ `url` (`video_url` ký tạm của Volcengine — GET thường, không cần key).
    ///
    /// # Errors
    /// [`SeedanceError::Network`]/[`SeedanceError::Http`] khi tải lỗi.
    pub async fn download_bytes(&self, url: &str) -> Result<Vec<u8>, SeedanceError> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| SeedanceError::Network(e.without_url().to_string()))?;
        let status = resp.status().as_u16();
        if !(200..=299).contains(&status) {
            return Err(SeedanceError::Http {
                status,
                detail: "tai video that bai".to_string(),
            });
        }
        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| SeedanceError::Network(e.without_url().to_string()))
    }
}

fn validate_key(api_key: &str) -> Result<(), SeedanceError> {
    if api_key.trim().is_empty() {
        Err(SeedanceError::Credential("API key rong".to_string()))
    } else {
        Ok(())
    }
}

/// Ánh xạ mã HTTP -> [`SeedanceError`]. `body` CẮT NGẮN — không log nguyên văn.
///
/// # Errors
/// [`SeedanceError`] khi `status` không phải 2xx.
pub fn check_status(status: u16, body: &str) -> Result<(), SeedanceError> {
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err(SeedanceError::Auth(status)),
        429 => Err(SeedanceError::Quota),
        s => Err(SeedanceError::Http {
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
        assert_eq!(
            check_status(403, "x").unwrap_err(),
            SeedanceError::Auth(403)
        );
        assert_eq!(check_status(429, "x").unwrap_err(), SeedanceError::Quota);
        match check_status(500, "loi").unwrap_err() {
            SeedanceError::Http { status, detail } => {
                assert_eq!(status, 500);
                assert_eq!(detail, "loi");
            }
            other => panic!("mong Http, gap {other:?}"),
        }
    }
}
