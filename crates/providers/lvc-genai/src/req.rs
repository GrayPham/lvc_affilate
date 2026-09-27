//! Kiểu yêu cầu sinh text + enum nhà cung cấp.

use serde::{Deserialize, Serialize};

/// Nhà cung cấp AI text. `kind`/tham số node dùng chuỗi `"gemini"`/`"openai"` (khớp
/// `CredentialProvider` phía UI); [`Provider::from_tag`] chuyển từ chuỗi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Gemini,
    #[serde(rename = "openai")]
    OpenAi,
}

impl Provider {
    /// Chuỗi ổn định (khớp provider của vault/UI).
    #[must_use]
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::Gemini => "gemini",
            Self::OpenAi => "openai",
        }
    }

    /// Parse từ chuỗi tham số node (không phân biệt hoa/thường). `None` = không nhận ra.
    #[must_use]
    pub fn from_tag(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gemini" => Some(Self::Gemini),
            "openai" => Some(Self::OpenAi),
            _ => None,
        }
    }
}

/// Một yêu cầu sinh kịch bản. Chứa TÊN credential (label API key trong vault) — KHÔNG chứa bí mật.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextGenReq {
    /// Tên (label) API key trong kho — engine dùng để resolve, node KHÔNG bao giờ thấy key.
    pub credential_name: String,
    pub provider: Provider,
    /// Chỉ dẫn hệ thống tuỳ chọn; rỗng = dùng mặc định của provider (xem `prompt::build_system`).
    pub system: String,
    /// Mô tả sản phẩm/chủ đề (brief) người dùng cung cấp.
    pub prompt: String,
    /// Số phân cảnh muốn sinh.
    pub num_scenes: u32,
    /// Ngôn ngữ đầu ra (vd `"vi"`).
    pub language: String,
    /// Phong cách/tông (vd `"quảng cáo ngắn, năng động"`); có thể rỗng.
    pub style: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_di_ve_khop() {
        assert_eq!(Provider::from_tag("Gemini"), Some(Provider::Gemini));
        assert_eq!(Provider::from_tag(" openai "), Some(Provider::OpenAi));
        assert_eq!(Provider::from_tag("khac"), None);
        assert_eq!(Provider::Gemini.as_tag(), "gemini");
        assert_eq!(Provider::OpenAi.as_tag(), "openai");
    }

    #[test]
    fn serde_dung_chuoi_thuong() {
        assert_eq!(
            serde_json::to_string(&Provider::OpenAi).unwrap(),
            "\"openai\""
        );
        assert_eq!(
            serde_json::from_str::<Provider>("\"gemini\"").unwrap(),
            Provider::Gemini
        );
    }
}
