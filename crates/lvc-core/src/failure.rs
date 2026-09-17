use serde::{Deserialize, Serialize};

/// Mã lỗi của task — 15 giá trị (spec §6.2: 13 + `RATIO_UNAVAILABLE`, `AGENT_NO_VIDEO` theo
/// `KNOWN_FAILURE_CODES` Python HEAD 3d07c19 `server.py:404-406`, P2 Task 9 vòng 2; `DOLA_REVOKED_STREAK`
/// / `REAL_PERSON` để P3/P4). `Cancelled` là mã, KHÔNG phải trạng thái: huỷ = `TaskStatus::Failed` +
/// `FailureCode::Cancelled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailureCode {
    Cancelled,
    LoggedOut,
    RateLimited,
    AccountUnavailable,
    PromptViolation,
    RefUpload,
    ProfileBusy,
    ProxyDown,
    DurationUnavailable,
    AgentQuestion,
    ModeUnavailable,
    NoConversation,
    Restart,
    /// `RatioUnavailableError("RATIO_UNAVAILABLE: …")` `video_worker_ui.py:2207`.
    RatioUnavailable,
    /// `RuntimeError("AGENT_NO_VIDEO: …")` `video_worker_ui.py:1384` — chat chết (`DEAD_CHAT_CODES`).
    AgentNoVideo,
}

impl FailureCode {
    /// Chuỗi lưu trong cột `failure_code`, khớp từng ký tự với bản Python.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::LoggedOut => "LOGGED_OUT",
            Self::RateLimited => "429",
            Self::AccountUnavailable => "ACCOUNT_UNAVAILABLE",
            Self::PromptViolation => "PROMPT_VIOLATION",
            Self::RefUpload => "REF_UPLOAD",
            Self::ProfileBusy => "PROFILE_BUSY",
            Self::ProxyDown => "PROXY_DOWN",
            Self::DurationUnavailable => "DURATION_UNAVAILABLE",
            Self::AgentQuestion => "AGENT_QUESTION",
            Self::ModeUnavailable => "MODE_UNAVAILABLE",
            Self::NoConversation => "NO_CONVERSATION",
            Self::Restart => "RESTART",
            Self::RatioUnavailable => "RATIO_UNAVAILABLE",
            Self::AgentNoVideo => "AGENT_NO_VIDEO",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cancelled" => Some(Self::Cancelled),
            "LOGGED_OUT" => Some(Self::LoggedOut),
            "429" => Some(Self::RateLimited),
            "ACCOUNT_UNAVAILABLE" => Some(Self::AccountUnavailable),
            "PROMPT_VIOLATION" => Some(Self::PromptViolation),
            "REF_UPLOAD" => Some(Self::RefUpload),
            "PROFILE_BUSY" => Some(Self::ProfileBusy),
            "PROXY_DOWN" => Some(Self::ProxyDown),
            "DURATION_UNAVAILABLE" => Some(Self::DurationUnavailable),
            "AGENT_QUESTION" => Some(Self::AgentQuestion),
            "MODE_UNAVAILABLE" => Some(Self::ModeUnavailable),
            "NO_CONVERSATION" => Some(Self::NoConversation),
            "RESTART" => Some(Self::Restart),
            "RATIO_UNAVAILABLE" => Some(Self::RatioUnavailable),
            "AGENT_NO_VIDEO" => Some(Self::AgentNoVideo),
            _ => None,
        }
    }

    /// Chat cũ vô dụng — không được resume (`DEAD_CHAT_CODES` của bản Python `server.py:407`:
    /// `PROMPT_VIOLATION`, `AGENT_QUESTION`, `AGENT_NO_VIDEO`).
    #[must_use]
    pub fn is_dead_chat(self) -> bool {
        matches!(
            self,
            Self::PromptViolation | Self::AgentQuestion | Self::AgentNoVideo
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [FailureCode; 15] = [
        FailureCode::Cancelled,
        FailureCode::LoggedOut,
        FailureCode::RateLimited,
        FailureCode::AccountUnavailable,
        FailureCode::PromptViolation,
        FailureCode::RefUpload,
        FailureCode::ProfileBusy,
        FailureCode::ProxyDown,
        FailureCode::DurationUnavailable,
        FailureCode::AgentQuestion,
        FailureCode::ModeUnavailable,
        FailureCode::NoConversation,
        FailureCode::Restart,
        FailureCode::RatioUnavailable,
        FailureCode::AgentNoVideo,
    ];

    #[test]
    fn round_trip_parse_as_str_cho_ca_muoi_lam_gia_tri() {
        for code in ALL {
            assert_eq!(FailureCode::parse(code.as_str()), Some(code));
        }
        // Khong trung chuoi giua cac ma
        let mut strs: Vec<&str> = ALL.iter().map(|c| c.as_str()).collect();
        strs.sort_unstable();
        strs.dedup();
        assert_eq!(strs.len(), ALL.len());
    }

    #[test]
    fn chuoi_khong_hop_le_tra_none() {
        assert_eq!(FailureCode::parse("bogus"), None);
        // Python HEAD con DOLA_REVOKED_STREAK/REAL_PERSON (P3/P4) — chua co o day
        assert_eq!(FailureCode::parse("DOLA_REVOKED_STREAK"), None);
        assert_eq!(FailureCode::parse("REAL_PERSON"), None);
    }

    /// `server.py:407` `DEAD_CHAT_CODES = ("PROMPT_VIOLATION", "AGENT_QUESTION", "AGENT_NO_VIDEO")`.
    #[test]
    fn is_dead_chat_dung_ba_ma() {
        for code in ALL {
            let expect = matches!(
                code,
                FailureCode::PromptViolation
                    | FailureCode::AgentQuestion
                    | FailureCode::AgentNoVideo
            );
            assert_eq!(code.is_dead_chat(), expect, "{code:?}");
        }
    }

    /// Task 9 vong 2 (ruling M-1): 2 ma trong `KNOWN_FAILURE_CODES` Python HEAD (`server.py:404-406`)
    /// ma spec §6.2 ban dau thieu.
    #[test]
    fn hai_ma_them_tu_python_head_task_9() {
        assert_eq!(FailureCode::RatioUnavailable.as_str(), "RATIO_UNAVAILABLE");
        assert_eq!(FailureCode::AgentNoVideo.as_str(), "AGENT_NO_VIDEO");
        assert_eq!(
            FailureCode::parse("RATIO_UNAVAILABLE"),
            Some(FailureCode::RatioUnavailable)
        );
        assert_eq!(
            FailureCode::parse("AGENT_NO_VIDEO"),
            Some(FailureCode::AgentNoVideo)
        );
        assert!(!FailureCode::RatioUnavailable.is_dead_chat());
        assert!(FailureCode::AgentNoVideo.is_dead_chat());
    }

    #[test]
    fn ba_ma_moi_tu_python_head() {
        assert_eq!(FailureCode::ModeUnavailable.as_str(), "MODE_UNAVAILABLE");
        assert_eq!(FailureCode::NoConversation.as_str(), "NO_CONVERSATION");
        assert_eq!(FailureCode::Restart.as_str(), "RESTART");
        assert!(!FailureCode::Restart.is_dead_chat());
    }
}
