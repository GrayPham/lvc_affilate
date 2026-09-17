//! Loi co MA cua luong Dola (P2). Tien to `Display` = ma loi ma Python nem trong `RuntimeError`
//! (`LOGGED_OUT: ...`, `MODE_UNAVAILABLE: ...`, `RATIO_UNAVAILABLE: ...`, `DURATION_UNAVAILABLE: ...`,
//! `REF_UPLOAD: ...`, `NO_CONVERSATION: ...`) hoac cac ngoai le rieng (`AccountLoggedOutError`,
//! `RatioUnavailableError`, `DurationUnavailableError`, `ReferenceUploadError`...). Task 6 tao DU
//! bien the theo plan Task 9 (ruling controller); Task 9 them phan loai cho engine (Task 15-17):
//! [`DolaError::failure_code`] (ma luu DB — 13 gia tri Python + `None`), [`DolaError::is_dead_chat`]
//! (`DEAD_CHAT_CODES`: chat cu vo dung, khong resume), [`DolaError::rotates_account`] (pool dong
//! Chrome + xoay tai khoan — `generate_video` `except (AccountLoggedOutError, ProfileBusyError,
//! RiskControlError, AccountLimitedError)` `video_worker_ui.py:2507-2509` + `CreditInsufficientError`
//! o pool).

use lvc_core::FailureCode;

use crate::BrowserError;

#[derive(Debug, thiserror::Error)]
pub enum DolaError {
    /// `session_state.py` `AccountLoggedOutError(reason, revoked)`: `revoked=true` khi Dola TU
    /// dang xuat (`from_logout=1` / nut dang nhap hien giua chung) — pool danh dau "Dola chan",
    /// KHONG auto-verify lai; `false` = mat phien thuong (cookie het han).
    #[error("LOGGED_OUT: {account}: {why}")]
    LoggedOut {
        account: String,
        why: String,
        revoked: bool,
    },
    #[error("MODE_UNAVAILABLE: {0}")]
    ModeUnavailable(String),
    /// `video_worker_ui.py:2049` `RuntimeError(f"Cannot switch Dola mode to {mode} ({want}); button shows {current!r}")`.
    #[error("Cannot switch Dola mode to {want}; button shows {shows:?}")]
    ModeSwitch { want: String, shows: String },
    #[error("RATIO_UNAVAILABLE: {0}")]
    RatioUnavailable(String),
    #[error("DURATION_UNAVAILABLE: {0}")]
    DurationUnavailable(String),
    #[error("REF_UPLOAD: {0}")]
    RefUpload(String),
    #[error("NO_CONVERSATION: {0}")]
    NoConversation(String),
    /// Captcha (`bdcaptcha.html`) — khong co ma Python; cooldown 1800 s.
    #[error("RISK_CONTROL: {0}")]
    RiskControl(String),
    #[error("PROMPT_VIOLATION: {0}")]
    PromptViolation(String),
    #[error("AGENT_QUESTION: {0}")]
    AgentQuestion(String),
    #[error("AGENT_NO_VIDEO: {0}")]
    AgentNoVideo(String),
    /// Han muc ngay — khong ma; pool danh dau han muc.
    #[error("LIMIT: {0}")]
    AccountLimited(String),
    /// Het credit — khong ma; pool xoay tai khoan.
    #[error("CREDIT: {0}")]
    Credit(String),
    /// Het gio cho video — khong ma.
    #[error("TIMEOUT: {0}")]
    Timeout(String),
    #[error("BROWSER_CLOSED: {0}")]
    BrowserClosed(String),
    #[error(transparent)]
    Browser(#[from] BrowserError),
    #[error("HTTP: {0}")]
    Http(String),
}

impl DolaError {
    /// Ma loi luu cot `failure_code` (spec §6.2): 7 bien the co ma Python + `Browser(ProfileBusy)` ->
    /// `PROFILE_BUSY` (ruling Task 9 vong 1: Python `ProfileBusyError` -> `"PROFILE_BUSY"`); con lai
    /// `None` (loi "khong ma": captcha, het credit, timeout poll, CDP khac...).
    #[must_use]
    pub fn failure_code(&self) -> Option<FailureCode> {
        match self {
            Self::LoggedOut { .. } => Some(FailureCode::LoggedOut),
            Self::ModeUnavailable(_) => Some(FailureCode::ModeUnavailable),
            Self::DurationUnavailable(_) => Some(FailureCode::DurationUnavailable),
            Self::RefUpload(_) => Some(FailureCode::RefUpload),
            Self::NoConversation(_) => Some(FailureCode::NoConversation),
            Self::PromptViolation(_) => Some(FailureCode::PromptViolation),
            Self::AgentQuestion(_) => Some(FailureCode::AgentQuestion),
            Self::Browser(BrowserError::ProfileBusy(_)) => Some(FailureCode::ProfileBusy),
            Self::ModeSwitch { .. }
            | Self::RatioUnavailable(_)
            | Self::RiskControl(_)
            | Self::AgentNoVideo(_)
            | Self::AccountLimited(_)
            | Self::Credit(_)
            | Self::Timeout(_)
            | Self::BrowserClosed(_)
            | Self::Browser(_)
            | Self::Http(_) => None,
        }
    }

    /// Chat cu VO DUNG — khong duoc resume/poll lai (`DEAD_CHAT_CODES` Python + `AGENT_NO_VIDEO`:
    /// agent da tra loi bang chu, khong co video).
    #[must_use]
    pub fn is_dead_chat(&self) -> bool {
        matches!(
            self,
            Self::PromptViolation(_) | Self::AgentQuestion(_) | Self::AgentNoVideo(_)
        )
    }

    /// Pool phai DONG Chrome va XOAY tai khoan (`video_worker_ui.py:2507-2509`): mat phien, captcha
    /// (cooldown 1800 s), han muc ngay, het credit, profile dang bi tien trinh khac giu. Loi
    /// `Browser(_)` khac (CDP/timeout) -> khong xoay (thu lai cung tai khoan).
    #[must_use]
    pub fn rotates_account(&self) -> bool {
        matches!(
            self,
            Self::LoggedOut { .. }
                | Self::RiskControl(_)
                | Self::AccountLimited(_)
                | Self::Credit(_)
                | Self::Browser(BrowserError::ProfileBusy(_))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tien to `Display` theo plan Task 9 (ma loi Python: `LOGGED_OUT: ...`, `MODE_UNAVAILABLE: ...`).
    #[test]
    fn display_dung_tien_to_ma_loi() {
        let cases: Vec<(DolaError, &str)> = vec![
            (
                DolaError::LoggedOut {
                    account: "acc1".into(),
                    why: "no sessionid cookie".into(),
                    revoked: false,
                },
                "LOGGED_OUT: acc1: no sessionid cookie",
            ),
            (
                DolaError::ModeUnavailable("x".into()),
                "MODE_UNAVAILABLE: x",
            ),
            (
                DolaError::ModeSwitch {
                    want: "fast (高速/Nhanh/Fast)".into(),
                    shows: "プロ".into(),
                },
                "Cannot switch Dola mode to fast (高速/Nhanh/Fast); button shows \"プロ\"",
            ),
            (
                DolaError::RatioUnavailable("x".into()),
                "RATIO_UNAVAILABLE: x",
            ),
            (
                DolaError::DurationUnavailable("x".into()),
                "DURATION_UNAVAILABLE: x",
            ),
            (DolaError::RefUpload("x".into()), "REF_UPLOAD: x"),
            (DolaError::NoConversation("x".into()), "NO_CONVERSATION: x"),
            (DolaError::RiskControl("x".into()), "RISK_CONTROL: x"),
            (
                DolaError::PromptViolation("x".into()),
                "PROMPT_VIOLATION: x",
            ),
            (DolaError::AgentQuestion("x".into()), "AGENT_QUESTION: x"),
            (DolaError::AgentNoVideo("x".into()), "AGENT_NO_VIDEO: x"),
            (DolaError::AccountLimited("x".into()), "LIMIT: x"),
            (DolaError::Credit("x".into()), "CREDIT: x"),
            (DolaError::Timeout("x".into()), "TIMEOUT: x"),
            (DolaError::BrowserClosed("x".into()), "BROWSER_CLOSED: x"),
            (DolaError::Http("x".into()), "HTTP: x"),
        ];
        for (e, want) in cases {
            assert_eq!(e.to_string(), want);
        }
    }

    /// `Browser(#[from] BrowserError)` la `transparent`: thong diep = thong diep cua `BrowserError`,
    /// `?` chuyen doi duoc.
    #[test]
    fn browser_transparent_va_from() {
        fn f() -> Result<(), DolaError> {
            Err::<(), BrowserError>(BrowserError::Timeout("t".into()))?;
            Ok(())
        }
        let e = f().unwrap_err();
        assert!(matches!(e, DolaError::Browser(BrowserError::Timeout(_))));
        assert_eq!(e.to_string(), "qua thoi gian cho: t");
    }

    #[test]
    fn logged_out_revoked_giu_co() {
        let e = DolaError::LoggedOut {
            account: "a".into(),
            why: "w".into(),
            revoked: true,
        };
        assert!(matches!(e, DolaError::LoggedOut { revoked: true, .. }));
    }

    fn logged_out() -> DolaError {
        DolaError::LoggedOut {
            account: "a".into(),
            why: "w".into(),
            revoked: false,
        }
    }

    /// Bang phan loai Task 9 (plan + ruling vong 1): `failure_code` (7 bien the co ma +
    /// `Browser(ProfileBusy)` -> `PROFILE_BUSY`, con lai `None`), `is_dead_chat`
    /// (`PromptViolation | AgentQuestion | AgentNoVideo`), `rotates_account` (`LoggedOut | RiskControl |
    /// AccountLimited | Credit | Browser(ProfileBusy)`). 17 bien the + 2 dong `Browser(_)` khac.
    #[test]
    fn phan_loai_bang_17_dong() {
        use FailureCode as F;
        let rows: Vec<(DolaError, Option<FailureCode>, bool, bool)> = vec![
            (logged_out(), Some(F::LoggedOut), false, true),
            (
                DolaError::ModeUnavailable("x".into()),
                Some(F::ModeUnavailable),
                false,
                false,
            ),
            (
                DolaError::ModeSwitch {
                    want: "w".into(),
                    shows: "s".into(),
                },
                None,
                false,
                false,
            ),
            (DolaError::RatioUnavailable("x".into()), None, false, false),
            (
                DolaError::DurationUnavailable("x".into()),
                Some(F::DurationUnavailable),
                false,
                false,
            ),
            (
                DolaError::RefUpload("x".into()),
                Some(F::RefUpload),
                false,
                false,
            ),
            (
                DolaError::NoConversation("x".into()),
                Some(F::NoConversation),
                false,
                false,
            ),
            (DolaError::RiskControl("x".into()), None, false, true),
            (
                DolaError::PromptViolation("x".into()),
                Some(F::PromptViolation),
                true,
                false,
            ),
            (
                DolaError::AgentQuestion("x".into()),
                Some(F::AgentQuestion),
                true,
                false,
            ),
            (DolaError::AgentNoVideo("x".into()), None, true, false),
            (DolaError::AccountLimited("x".into()), None, false, true),
            (DolaError::Credit("x".into()), None, false, true),
            (DolaError::Timeout("x".into()), None, false, false),
            (DolaError::BrowserClosed("x".into()), None, false, false),
            // Ruling Task 9 vong 1: spec §6.2 co PROFILE_BUSY, Python ProfileBusyError -> "PROFILE_BUSY"
            (
                DolaError::Browser(BrowserError::ProfileBusy("p".into())),
                Some(F::ProfileBusy),
                false,
                true,
            ),
            (DolaError::Http("x".into()), None, false, false),
            // Browser(_) KHONG phai ProfileBusy -> khong ma, khong xoay
            (
                DolaError::Browser(BrowserError::Timeout("t".into())),
                None,
                false,
                false,
            ),
            (
                DolaError::Browser(BrowserError::Launch("l".into())),
                None,
                false,
                false,
            ),
        ];
        assert_eq!(rows.len(), 19);
        for (e, code, dead, rotates) in rows {
            assert_eq!(e.failure_code(), code, "failure_code {e}");
            assert_eq!(e.is_dead_chat(), dead, "is_dead_chat {e}");
            assert_eq!(e.rotates_account(), rotates, "rotates_account {e}");
        }
    }

    /// Ma loi tra ve phai la 1 trong 13 gia tri Python (`FailureCode::as_str`), khong bia them.
    #[test]
    fn failure_code_khop_chuoi_python() {
        assert_eq!(
            logged_out().failure_code().map(FailureCode::as_str),
            Some("LOGGED_OUT")
        );
        assert_eq!(
            DolaError::NoConversation("x".into())
                .failure_code()
                .map(FailureCode::as_str),
            Some("NO_CONVERSATION")
        );
        assert_eq!(
            DolaError::AgentQuestion("x".into())
                .failure_code()
                .map(FailureCode::as_str),
            Some("AGENT_QUESTION")
        );
        assert_eq!(
            DolaError::Browser(BrowserError::ProfileBusy("p".into()))
                .failure_code()
                .map(FailureCode::as_str),
            Some("PROFILE_BUSY")
        );
    }
}
