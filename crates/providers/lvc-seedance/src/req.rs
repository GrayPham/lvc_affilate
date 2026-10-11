//! Kiểu yêu cầu THUẦN cho client Seedance/Seedream. Không I/O, test riêng.

use serde::{Deserialize, Serialize};

/// Tỉ lệ khung — ánh xạ sang `size` (Seedream ảnh) và `--ratio` (Seedance video).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Aspect {
    Portrait,
    Landscape,
    Square,
}

impl Aspect {
    /// `size` cho Seedream `images/generations` (xấp xỉ 1 megapixel, đúng tỉ lệ).
    #[must_use]
    pub fn as_size(self) -> &'static str {
        match self {
            Self::Portrait => "864x1152",
            Self::Landscape => "1152x864",
            Self::Square => "1024x1024",
        }
    }

    /// Chuỗi `--ratio` cho Seedance video (command trong `content.text`).
    #[must_use]
    pub fn as_ratio(self) -> &'static str {
        match self {
            Self::Portrait => "9:16",
            Self::Landscape => "16:9",
            Self::Square => "1:1",
        }
    }

    #[must_use]
    pub fn from_tag(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "portrait" => Some(Self::Portrait),
            "landscape" => Some(Self::Landscape),
            "square" => Some(Self::Square),
            _ => None,
        }
    }
}

/// Model mặc định khi tham số `model` rỗng. LƯU Ý (xem MEMORY.md): Volcengine thường yêu cầu "endpoint
/// id" riêng theo tài khoản — người dùng nên nhập model/endpoint của chính họ ở tham số node.
pub mod default_model {
    /// Seedream (ảnh). Người dùng thường phải thay bằng id của tài khoản.
    pub const IMAGE: &str = "doubao-seedream-3-0-t2i-250415";
    /// Seedance (video).
    pub const VIDEO: &str = "doubao-seedance-1-0-pro-250528";
}

/// Yêu cầu tạo ảnh (Seedream `images/generations`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReq {
    pub model: String,
    pub prompt: String,
    pub aspect: Aspect,
}

/// Yêu cầu tạo video (Seedance `contents/generations/tasks`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoReq {
    pub model: String,
    pub prompt: String,
    pub aspect: Aspect,
    /// URL ảnh gốc cho image-to-video (`None` = text-to-video). Seedance nhận `image_url.url` —
    /// data URL base64 cũng hợp lệ (xem MEMORY.md).
    pub image_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_di_ve_khop() {
        assert_eq!(Aspect::from_tag("Portrait"), Some(Aspect::Portrait));
        assert_eq!(Aspect::Portrait.as_size(), "864x1152");
        assert_eq!(Aspect::Landscape.as_ratio(), "16:9");
        assert_eq!(Aspect::from_tag("x"), None);
    }
}
