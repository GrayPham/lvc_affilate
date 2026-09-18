//! Loi cua `lvc-engine` (Task 15; dung tiep o Task 16 `AccountPool` / Task 17 `generate_video`):
//! boc `DolaError`/`BrowserError`/`StoreError` cua cac crate tang duoi + 4 bien the rieng cua
//! scheduler. `EngineError::failure_code` anh xa sang `lvc_core::FailureCode` de luu cot
//! `failure_code` trong DB — dinh nghia dung theo plan Task 16
//! (`docs/superpowers/plans/2026-09-17-p2-dola-video.md`, khoi `EngineError`) + ruling Task 15
//! them nhanh `Browser(Navigation)` voi `is_proxy_down()` -> `ProxyDown`.

use lvc_browser::{BrowserError, DolaError};
use lvc_core::{FailureCode, Mode};
use lvc_store::StoreError;

/// Loi cua scheduler (`lvc-engine`). Ba bien the dau boc nguyen loi tang duoi (`#[error(transparent)]`
/// giu `Display` cua loi goc); bon bien the sau la loi rieng cua Task 16/17 (khong tim duoc tai
/// khoan/proxy chet...).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Dola(#[from] DolaError),
    #[error(transparent)]
    Browser(#[from] BrowserError),
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Het ca cai vi (candidates rong) cho `mode` — Task 16 `candidates_for`.
    #[error("429: all accounts limited for {}", .mode.as_str())]
    AllAccountsLimited { mode: Mode },
    /// `prefer_account` duoc chi dinh nhung tai khoan do khong dung duoc (thieu / bi khoa cung).
    #[error("ACCOUNT_UNAVAILABLE: {0}")]
    PreferredUnavailable(String),
    /// Moi tai khoan ung vien deu dang ban profile trinh duyet (khong `try_lease` duoc con nao).
    #[error("PROFILE_BUSY: no account with a free browser profile")]
    NoFreeProfile,
    /// Dieu huong that bai do proxy chet (`BrowserError::is_proxy_down`) da gan cho 1 tai khoan cu the.
    #[error("PROXY_DOWN: {account}: {why}")]
    ProxyDown { account: String, why: String },
    /// Review cuoi P2 M-10: `task_id` khong co dong trong `tasks` — loi cua nguoi goi, KHONG
    /// chay browser (tranh tieu credit ma khong co dong nao de ghi ket qua).
    #[error("TASK_NOT_FOUND: {0}")]
    TaskNotFound(String),
}

impl EngineError {
    /// Anh xa sang `lvc_core::FailureCode` de luu cot `failure_code` (Task 16 `AccountPool` /
    /// Task 17 `generate_video` dung ket qua nay). Bang phan loai (ruling Task 15):
    /// - `Dola(e)` -> `e.failure_code()` (`DolaError` da phan loai o Task 9).
    /// - `Browser(ProfileBusy)` -> `ProfileBusy`; `Browser` khac ma `is_proxy_down()` -> `ProxyDown`
    ///   (vi du `Navigation` voi ma loi `ERR_PROXY_*`/`ERR_TUNNEL_*`/`ERR_SOCKS_*`); `Browser` con
    ///   lai -> `None` (loi CDP/I-O khong co ma nghiep vu).
    /// - `Store(_)` -> `None` (loi ha tang, khong co ma nghiep vu).
    /// - `AllAccountsLimited` -> `RateLimited` (ma `"429"`).
    /// - `PreferredUnavailable` -> `AccountUnavailable`.
    /// - `NoFreeProfile` -> `ProfileBusy`.
    /// - `ProxyDown` -> `ProxyDown`.
    #[must_use]
    pub fn failure_code(&self) -> Option<FailureCode> {
        match self {
            Self::Dola(e) => e.failure_code(),
            // Browser(ProfileBusy) (candidates_for tim thay tai khoan nhung profile dang bi tien
            // trinh khac giu) va NoFreeProfile (het candidates vi TAT CA deu ban) cung mot ma.
            Self::Browser(BrowserError::ProfileBusy(_)) | Self::NoFreeProfile => {
                Some(FailureCode::ProfileBusy)
            }
            Self::Browser(e) if e.is_proxy_down() => Some(FailureCode::ProxyDown),
            // TaskNotFound: loi nguoi goi, khong co dong tasks de ghi ma -> None nhu Store.
            Self::Browser(_) | Self::Store(_) | Self::TaskNotFound(_) => None,
            Self::AllAccountsLimited { .. } => Some(FailureCode::RateLimited),
            Self::PreferredUnavailable(_) => Some(FailureCode::AccountUnavailable),
            Self::ProxyDown { .. } => Some(FailureCode::ProxyDown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tien to `Display` cho 4 bien the rieng cua `lvc-engine` (khop chuoi trong plan Task 16).
    #[test]
    fn display_dung_tien_to() {
        assert_eq!(
            EngineError::AllAccountsLimited { mode: Mode::Fast }.to_string(),
            "429: all accounts limited for fast"
        );
        assert_eq!(
            EngineError::AllAccountsLimited { mode: Mode::Agent }.to_string(),
            "429: all accounts limited for agent"
        );
        assert_eq!(
            EngineError::PreferredUnavailable("acc1".into()).to_string(),
            "ACCOUNT_UNAVAILABLE: acc1"
        );
        assert_eq!(
            EngineError::NoFreeProfile.to_string(),
            "PROFILE_BUSY: no account with a free browser profile"
        );
        assert_eq!(
            EngineError::ProxyDown {
                account: "acc1".into(),
                why: "het thoi gian cho".into(),
            }
            .to_string(),
            "PROXY_DOWN: acc1: het thoi gian cho"
        );
    }

    /// `#[error(transparent)]` giu nguyen thong diep cua loi tang duoi + `?` chuyen doi duoc.
    #[test]
    fn transparent_giu_thong_diep_goc() {
        fn f() -> Result<(), EngineError> {
            Err::<(), DolaError>(DolaError::Timeout("t".into()))?;
            Ok(())
        }
        let e = f().unwrap_err();
        assert!(matches!(e, EngineError::Dola(DolaError::Timeout(_))));
        assert_eq!(e.to_string(), "TIMEOUT: t");
    }

    /// Bang anh xa `failure_code` (ruling Task 15): moi bien the trong 1 dong.
    #[test]
    fn failure_code_bang_anh_xa() {
        let rows: Vec<(EngineError, Option<FailureCode>)> = vec![
            // Dola(e) -> e.failure_code()
            (
                EngineError::Dola(DolaError::NoConversation("x".into())),
                Some(FailureCode::NoConversation),
            ),
            (EngineError::Dola(DolaError::Timeout("x".into())), None),
            (
                EngineError::Dola(DolaError::AgentNoVideo("x".into())),
                Some(FailureCode::AgentNoVideo),
            ),
            // Browser(ProfileBusy) -> ProfileBusy
            (
                EngineError::Browser(BrowserError::ProfileBusy("p".into())),
                Some(FailureCode::ProfileBusy),
            ),
            // Browser(Navigation) proxy chet -> ProxyDown
            (
                EngineError::Browser(BrowserError::Navigation {
                    url_host_path: "https://www.dola.com/chat".into(),
                    net_error: "net::ERR_PROXY_CONNECTION_FAILED".into(),
                }),
                Some(FailureCode::ProxyDown),
            ),
            // Browser(Navigation) loi mang khac -> None
            (
                EngineError::Browser(BrowserError::Navigation {
                    url_host_path: "https://www.dola.com/chat".into(),
                    net_error: "net::ERR_CONNECTION_REFUSED".into(),
                }),
                None,
            ),
            // Browser con lai -> None
            (
                EngineError::Browser(BrowserError::Timeout("t".into())),
                None,
            ),
            (EngineError::Browser(BrowserError::Disconnected), None),
            // Store -> None
            (EngineError::Store(StoreError::BadPath("x".into())), None),
            // 4 bien the rieng
            (
                EngineError::AllAccountsLimited { mode: Mode::Fast },
                Some(FailureCode::RateLimited),
            ),
            (
                EngineError::PreferredUnavailable("acc1".into()),
                Some(FailureCode::AccountUnavailable),
            ),
            (EngineError::NoFreeProfile, Some(FailureCode::ProfileBusy)),
            (
                EngineError::ProxyDown {
                    account: "acc1".into(),
                    why: "down".into(),
                },
                Some(FailureCode::ProxyDown),
            ),
        ];
        assert_eq!(rows.len(), 13);
        for (e, want) in rows {
            assert_eq!(e.failure_code(), want, "{e}");
        }
    }
}
