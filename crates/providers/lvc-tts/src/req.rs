//! Nhà cung cấp TTS + kiểu yêu cầu tổng hợp giọng nói (cấp CLIENT — không mang `scene_index`/
//! `credential_id`, những thứ đó là mối quan tâm định tuyến của [`crate::TtsJob`]).

use serde::{Deserialize, Serialize};

/// Nhà cung cấp AI text-to-speech. `kind`/tham số node dùng chuỗi `"elevenlabs"`/`"minimax"` (khớp
/// `CredentialProvider` phía UI); [`Provider::from_tag`] chuyển từ chuỗi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    ElevenLabs,
    Minimax,
}

impl Provider {
    /// Chuỗi ổn định (khớp provider của vault/UI).
    #[must_use]
    pub fn as_tag(self) -> &'static str {
        match self {
            Self::ElevenLabs => "elevenlabs",
            Self::Minimax => "minimax",
        }
    }

    /// Parse từ chuỗi tham số node (không phân biệt hoa/thường). `None` = không nhận ra.
    #[must_use]
    pub fn from_tag(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "elevenlabs" => Some(Self::ElevenLabs),
            "minimax" => Some(Self::Minimax),
            _ => None,
        }
    }
}

/// Một lần gọi API tổng hợp giọng nói (cấp HTTP body — không mang bí mật).
#[derive(Debug, Clone, PartialEq)]
pub struct SynthReq {
    pub provider: Provider,
    pub text: String,
    /// Id giọng đọc — tự do theo từng provider (vd `ElevenLabs` `21m00Tcm4TlvDq8ikWAM`, `MiniMax`
    /// `English_expressive_narrator`).
    pub voice_id: String,
    /// Model — rỗng = mặc định của provider (xem `elevenlabs::DEFAULT_MODEL`/`minimax::DEFAULT_MODEL`).
    pub model: String,
    /// Tốc độ đọc (1.0 = mặc định).
    pub speed: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_di_ve_khop() {
        assert_eq!(Provider::from_tag("ElevenLabs"), Some(Provider::ElevenLabs));
        assert_eq!(Provider::from_tag(" minimax "), Some(Provider::Minimax));
        assert_eq!(Provider::from_tag("khac"), None);
        assert_eq!(Provider::ElevenLabs.as_tag(), "elevenlabs");
        assert_eq!(Provider::Minimax.as_tag(), "minimax");
    }
}
