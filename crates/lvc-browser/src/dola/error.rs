//! Loi co MA cua luong Dola (P2). Tien to `Display` = ma loi ma Python nem trong `RuntimeError`
//! (`LOGGED_OUT: ...`, `MODE_UNAVAILABLE: ...`, `RATIO_UNAVAILABLE: ...`, `DURATION_UNAVAILABLE: ...`,
//! `REF_UPLOAD: ...`, `NO_CONVERSATION: ...`) hoac cac ngoai le rieng (`AccountLoggedOutError`,
//! `RatioUnavailableError`, `DurationUnavailableError`, `ReferenceUploadError`...). Task 6 tao DU
//! bien the theo plan Task 9 (ruling controller) de Task 9 chi them `failure_code`/`is_dead_chat`/
//! `rotates_account`.

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
}
