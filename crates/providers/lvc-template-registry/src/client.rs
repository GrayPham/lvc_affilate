//! Client HTTP — một GET duy nhất tới `index.json`, không auth, không cache (cache thuộc tầng gọi,
//! xem `lvc-app::commands::templates`).

use crate::error::RegistryError;
use crate::model::RemoteTemplate;

/// Số ký tự tối đa giữ lại của thân lỗi khi log/hiển thị.
const DETAIL_MAX: usize = 300;

/// Nguồn mặc định: GitHub raw content của repo mẫu `GrayPham/lvc-template-registry`, nhánh
/// `master`. Có thể trỏ tới registry khác bằng cách truyền `index_url` khác cho
/// [`RegistryClient::new`] (vd fork riêng của khách hàng).
pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/GrayPham/lvc-template-registry/master/index.json";

/// Client đọc kho template cộng đồng. Rẻ để `clone`.
#[derive(Debug, Clone)]
pub struct RegistryClient {
    http: reqwest::Client,
    index_url: String,
}

impl Default for RegistryClient {
    fn default() -> Self {
        Self::new(DEFAULT_INDEX_URL)
    }
}

impl RegistryClient {
    #[must_use]
    pub fn new(index_url: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            index_url: index_url.into(),
        }
    }

    #[must_use]
    pub fn with_client(http: reqwest::Client, index_url: impl Into<String>) -> Self {
        Self {
            http,
            index_url: index_url.into(),
        }
    }

    /// Tải toàn bộ danh mục (mảng `RemoteTemplate`, kèm `graph`).
    ///
    /// # Errors
    /// [`RegistryError`] khi mạng lỗi, HTTP không phải 2xx, hoặc JSON sai hình dạng.
    pub async fn fetch_index(&self) -> Result<Vec<RemoteTemplate>, RegistryError> {
        let resp = self
            .http
            .get(&self.index_url)
            .send()
            .await
            .map_err(|e| RegistryError::Network(e.to_string()))?;
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| RegistryError::Network(e.to_string()))?;
        if !(200..=299).contains(&status) {
            return Err(RegistryError::Http {
                status,
                detail: body.trim().chars().take(DETAIL_MAX).collect(),
            });
        }
        serde_json::from_str(&body).map_err(|e| RegistryError::Decode(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tro_dung_repo_mau() {
        assert!(DEFAULT_INDEX_URL.contains("lvc-template-registry"));
        assert!(DEFAULT_INDEX_URL.ends_with("index.json"));
    }
}
