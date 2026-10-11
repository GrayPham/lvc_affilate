//! Kiểu yêu cầu THUẦN cho client Google (ảnh/video). Không I/O, test riêng.

use serde::{Deserialize, Serialize};

/// Tỉ lệ khung — ánh xạ sang chuỗi `aspectRatio` của API Google (Imagen/Veo dùng CHUNG dạng này).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Aspect {
    Portrait,
    Landscape,
    Square,
}

impl Aspect {
    /// Chuỗi `aspectRatio` cho API Google.
    #[must_use]
    pub fn as_ratio(self) -> &'static str {
        match self {
            Self::Portrait => "9:16",
            Self::Landscape => "16:9",
            Self::Square => "1:1",
        }
    }

    /// Parse từ tham số node (không phân biệt hoa/thường). `None` = không nhận ra.
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

/// Model mặc định khi tham số `model` rỗng.
pub mod default_model {
    /// Nano Banana (Gemini image) — Imagen `:predict` ĐÃ bị tắt trên Gemini Developer API
    /// (2026-10-01). Sửa hằng này nếu Google đổi tên — xem MEMORY.md.
    pub const IMAGE: &str = "gemini-3.1-flash-image";
    /// Veo 3.1 (tạo video, long-running operation) — `veo-3.0-*` đã deprecated. Sửa nếu Google đổi.
    pub const VIDEO: &str = "veo-3.1-generate-preview";
}

/// Yêu cầu tạo ảnh (Imagen `:predict`). `model` rỗng = [`default_model::IMAGE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReq {
    pub model: String,
    pub prompt: String,
    pub aspect: Aspect,
    /// Số ảnh trong một lần gọi (`sampleCount`); ép >= 1 ở client.
    pub sample_count: u32,
}

/// Yêu cầu tạo video (Veo `:predictLongRunning`). `model` rỗng = [`default_model::VIDEO`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoReq {
    pub model: String,
    pub prompt: String,
    pub aspect: Aspect,
    pub sample_count: u32,
    /// Ảnh gốc cho image-to-video (base64 + mime) — `None` = text-to-video.
    pub image: Option<ImageInput>,
}

/// Ảnh đầu vào cho i2v: bytes đã đọc + mime (vd `image/png`). Client tự base64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageInput {
    pub bytes: Vec<u8>,
    pub mime: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_di_ve_khop() {
        assert_eq!(Aspect::from_tag("Portrait"), Some(Aspect::Portrait));
        assert_eq!(Aspect::from_tag(" landscape "), Some(Aspect::Landscape));
        assert_eq!(Aspect::from_tag("square"), Some(Aspect::Square));
        assert_eq!(Aspect::from_tag("khac"), None);
        assert_eq!(Aspect::Portrait.as_ratio(), "9:16");
        assert_eq!(Aspect::Landscape.as_ratio(), "16:9");
    }
}
