//! Loi cua `lvc-engine` (Task 15; dung tiep o Task 16 `AccountPool` / Task 17 `generate_video`):
//! boc `DolaError`/`BrowserError`/`StoreError` cua cac crate tang duoi + 4 bien the rieng cua
//! scheduler. `EngineError::failure_code` anh xa sang `lvc_core::FailureCode` de luu cot
//! `failure_code` trong DB — dinh nghia dung theo plan Task 16
//! (`docs/superpowers/plans/2026-09-17-p2-dola-video.md`, khoi `EngineError`) + ruling Task 15
//! them nhanh `Browser(Navigation)` voi `is_proxy_down()` -> `ProxyDown`.

use lvc_browser::{BrowserError, DolaError};
use lvc_core::{FailureCode, GraphError, Mode};
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
    /// P3 Task 6 (`resume::resume`): dong `task_id` TON TAI nhung thieu `conversation_id` va/hoac
    /// `account` — khong biet mo lai chat nao/tai khoan nao (`video_worker_ui.py:1787-1842` can ca
    /// hai). Khac [`Self::TaskNotFound`]: o day dong VAN duoc ghi `Failed` + `failure_code` (ruling
    /// controller task-6) vi day la ket qua "khong resume duoc", khong phai loi truy van.
    #[error("NOT_RESUMABLE: {task_id}: {why}")]
    NotResumable { task_id: String, why: String },
    /// P3 Task 7 (`dag::Engine::run_graph`): do thi khong hop le hoac loi lap ke hoach
    /// (`lvc_core::graph::plan`, da gom ca `validate`) — tra `Err` NGAY, KHONG tao dong `runs`
    /// (ruling task-7-brief.md). Khong dung `#[from]`: `Vec<GraphError>` khong `impl
    /// std::error::Error` (thiserror doi hoi de lam `source()`) — viet tay `impl From` ben duoi
    /// thay vi boc `Box<dyn Error>` chi de thoa man rang buoc do.
    #[error("do thi khong hop le: {} loi", .0.len())]
    Graph(Vec<GraphError>),
}

impl From<Vec<GraphError>> for EngineError {
    fn from(errs: Vec<GraphError>) -> Self {
        Self::Graph(errs)
    }
}

impl EngineError {
    /// Anh xa sang `lvc_core::FailureCode` de luu cot `failure_code` (Task 16 `AccountPool` /
    /// Task 17 `generate_video` dung ket qua nay). Bang phan loai (ruling Task 15):
    /// - `Dola(e)` -> `e.failure_code()` (`DolaError` da phan loai o Task 9); rieng
    ///   `Dola(Cancelled)` -> `Some(FailureCode::Cancelled)` (P3 R4, nguoi dung bam Dung — ghim o
    ///   test `failure_code_bang_anh_xa`, fix-round Minor M6).
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
            // TaskNotFound: loi nguoi goi, khong co dong tasks de ghi ma -> None nhu Store. P3
            // Task 7 (Graph): khong tao dong task nao ca (khong co run) -> cung None.
            Self::Browser(_) | Self::Store(_) | Self::TaskNotFound(_) | Self::Graph(_) => None,
            Self::AllAccountsLimited { .. } => Some(FailureCode::RateLimited),
            Self::PreferredUnavailable(_) => Some(FailureCode::AccountUnavailable),
            Self::ProxyDown { .. } => Some(FailureCode::ProxyDown),
            // P3 Task 6 (ruling): khong them ma Dola moi — tai su dung NO_CONVERSATION (gan nghia
            // nhat: khong the resume vi thieu du lieu de mo lai hoi thoai).
            Self::NotResumable { .. } => Some(FailureCode::NoConversation),
        }
    }

    /// P3 Task 6: `DolaError` -> `EngineError` dung CHUNG boi `generate::after_dola_error` va
    /// `resume::resume` — giong `#[from]` cho MOI bien the, TRU loi trinh duyet la proxy chet
    /// (`BrowserError::is_proxy_down`): khi do doi sang [`Self::ProxyDown`] (mang ten tai khoan,
    /// dung `failure_code` `PROXY_DOWN` giong `browser_pool.py` `proxy_precheck`) thay vi boc nguyen
    /// `Dola(Browser(..))` (se ra `failure_code = None`).
    #[must_use]
    pub(crate) fn from_dola(name: &str, e: DolaError) -> Self {
        if let DolaError::Browser(b) = &e {
            if b.is_proxy_down() {
                return Self::ProxyDown {
                    account: name.to_string(),
                    why: e.to_string(),
                };
            }
        }
        Self::from(e)
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
        assert_eq!(
            EngineError::NotResumable {
                task_id: "t1".into(),
                why: "thieu conversation_id".into(),
            }
            .to_string(),
            "NOT_RESUMABLE: t1: thieu conversation_id"
        );
        assert_eq!(
            EngineError::Graph(vec![
                GraphError::Empty,
                GraphError::DuplicateNodeId { id: "n1".into() }
            ])
            .to_string(),
            "do thi khong hop le: 2 loi"
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

    /// P3 Task 6: `from_dola` boc nguyen tru proxy chet (doi sang `ProxyDown` mang ten tai khoan).
    #[test]
    fn from_dola_boc_nguyen_tru_proxy_chet() {
        let e = EngineError::from_dola("acc1", DolaError::Timeout("t".into()));
        assert!(matches!(e, EngineError::Dola(DolaError::Timeout(_))));

        let proxy_err = DolaError::Browser(BrowserError::Navigation {
            url_host_path: "https://www.dola.com/chat".into(),
            net_error: "net::ERR_PROXY_CONNECTION_FAILED".into(),
        });
        let e = EngineError::from_dola("acc1", proxy_err);
        match e {
            EngineError::ProxyDown { account, why } => {
                assert_eq!(account, "acc1");
                assert!(why.contains("ERR_PROXY_CONNECTION_FAILED"), "{why}");
            }
            other => panic!("{other:?}"),
        }
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
            // Fix-round Minor M6 (review task-4-review.md finding 7): P3 R4 huy nguoi dung.
            (
                EngineError::Dola(DolaError::Cancelled),
                Some(FailureCode::Cancelled),
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
            // P3 Task 6: NotResumable -> tai su dung NO_CONVERSATION (khong them ma Dola moi)
            (
                EngineError::NotResumable {
                    task_id: "t1".into(),
                    why: "thieu conversation_id".into(),
                },
                Some(FailureCode::NoConversation),
            ),
            // P3 Task 7: khong co dong task nao de ghi ma -> None (giong Store/TaskNotFound).
            (EngineError::Graph(vec![GraphError::Empty]), None),
        ];
        assert_eq!(rows.len(), 16);
        for (e, want) in rows {
            assert_eq!(e.failure_code(), want, "{e}");
        }
    }
}
