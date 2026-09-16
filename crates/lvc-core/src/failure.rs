use serde::{Deserialize, Serialize};

/// Mã lỗi của task — đúng 13 giá trị của bản Python. `Cancelled` là mã, KHÔNG
/// phải trạng thái: huỷ = `TaskStatus::Failed` + `FailureCode::Cancelled`.
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
            _ => None,
        }
    }

    /// Chat cũ vô dụng — không được resume (`DEAD_CHAT_CODES` của bản Python).
    #[must_use]
    pub fn is_dead_chat(self) -> bool {
        matches!(self, Self::PromptViolation | Self::AgentQuestion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [FailureCode; 13] = [
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
    ];

    #[test]
    fn round_trip_parse_as_str_cho_ca_muoi_ba_gia_tri() {
        for code in ALL {
            assert_eq!(FailureCode::parse(code.as_str()), Some(code));
        }
    }

    #[test]
    fn chuoi_khong_hop_le_tra_none() {
        assert_eq!(FailureCode::parse("bogus"), None);
    }

    #[test]
    fn is_dead_chat_dung_hai_ma() {
        for code in ALL {
            let expect = matches!(
                code,
                FailureCode::PromptViolation | FailureCode::AgentQuestion
            );
            assert_eq!(code.is_dead_chat(), expect, "{code:?}");
        }
    }

    #[test]
    fn ba_ma_moi_tu_python_head() {
        assert_eq!(FailureCode::ModeUnavailable.as_str(), "MODE_UNAVAILABLE");
        assert_eq!(FailureCode::NoConversation.as_str(), "NO_CONVERSATION");
        assert_eq!(FailureCode::Restart.as_str(), "RESTART");
        assert!(!FailureCode::Restart.is_dead_chat());
    }
}
