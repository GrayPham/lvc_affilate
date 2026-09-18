//! Tang lenh IPC (Task 9-10, P3): MOI giao tiep frontend/backend la mot `#[tauri::command]` dang
//! ky trong `lib.rs::invoke_handler` — khong may chu HTTP nao (ADR P3, xem doc `lib.rs`).
//!
//! [`IpcError`] la kieu loi CHUNG cho MOI lenh — `Serialize` camelCase de frontend (Task 11) doc
//! truc tiep, khong can parse chuoi loi Rust. `From<EngineError>`/`From<StoreError>`/
//! `From<AccountError>` (Task 10) anh xa loi tang duoi sang `code` on dinh (xem doc ba impl duoi).
//!
//! [`Ctx`] la phan cua `AppState` (Task 8) ma CAC HAM LOI cua tung lenh nhan — generic tren
//! `D: BrowserDriver` (khong phai `tauri::State<AppState>` cu the `ChromiumDriver`) de test duoc
//! bang `FakeDriver` + DB tam, KHONG can runtime Tauri that (giong `bootstrap::build_engine`, Task
//! 8). Quy uoc dat ten: moi lenh co HAI ham —
//! - `xxx_impl(ctx: &Ctx<D>, ..) -> Result<T, IpcError>` (hoac khong `Result` khi khong the loi):
//!   THAN LOI, test o day.
//! - `#[tauri::command] async fn xxx(state: tauri::State<'_, AppState>, ..) -> Result<T, IpcError>`:
//!   lop Tauri MONG, chi goi `xxx_impl(&state, ..)` — KHONG test duoc rieng (can `AppHandle` that),
//!   nen giu cang mong cang tot.
//!
//! `AppState` (`state.rs`) la alias `Ctx<ChromiumDriver>` — moi `xxx_impl` deu chay dung tren
//! `AppState` do (deref tu `tauri::State`).

pub mod accounts;
pub mod settings;
pub mod tasks;
pub mod workflow;

#[cfg(test)]
mod fixtures;

use std::sync::Arc;

use dashmap::DashMap;
use lvc_browser::BrowserDriver;
use lvc_core::AppSettings;
use lvc_engine::{AccountError, Engine, EngineError, LimiterHandle, RunHandle};
use lvc_store::{Db, StoreError};
use serde::Serialize;
use tokio::sync::RwLock;

/// Loi tra ve cho MOI lenh IPC. `Serialize` camelCase (`details` bo qua khi `None` de JSON gon —
/// KHONG, brief doi field co mat luon de frontend khong phai kiem `"details" in err`; giu
/// `Option<Value>` tuong minh, khong `skip_serializing_if`).
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code}: {message}")]
pub struct IpcError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl IpcError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// `EngineError::Graph(errs)` -> ma `GRAPH_INVALID`, `details` = `errs` da serialize (UI React Flow
/// to node/canh loi tu day, Task 11). `TaskNotFound` -> `TASK_NOT_FOUND`. Con lai -> `failure_code()`
/// khi co (vd `PROXY_DOWN`, `429`), khong thi ma chung `ENGINE`.
impl From<EngineError> for IpcError {
    fn from(e: EngineError) -> Self {
        match &e {
            EngineError::Graph(errs) => {
                let details = serde_json::to_value(errs).unwrap_or(serde_json::Value::Null);
                Self::new("GRAPH_INVALID", e.to_string()).with_details(details)
            }
            EngineError::TaskNotFound(_) => Self::new("TASK_NOT_FOUND", e.to_string()),
            _ => {
                let code = e
                    .failure_code()
                    .map_or_else(|| "ENGINE".to_string(), |c| c.as_str().to_string());
                Self::new(code, e.to_string())
            }
        }
    }
}

/// Loi tang luu tru -> ma chung `STORE` (khong co `failure_code` rieng — loi ha tang, khong phai
/// loi nghiep vu).
impl From<StoreError> for IpcError {
    fn from(e: StoreError) -> Self {
        Self::new("STORE", e.to_string())
    }
}

/// Task 10 (`lvc_engine::accounts`): moi lenh tai khoan dung `AccountError::code()` lam nguon DUY
/// NHAT cho `code` (KHONG suy tu `Display`/chuoi loi — xem doc `AccountError::code`).
impl From<AccountError> for IpcError {
    fn from(e: AccountError) -> Self {
        Self::new(e.code(), e.to_string())
    }
}

/// Phan cua `AppState` (Task 8) ma cac lenh IPC doc/ghi — xem doc dau module ve ly do generic tren
/// `D`. `runs`: `run_id` -> [`RunHandle`] (Task 7) cua lan chay dang co — `run_start` chen vao,
/// `run_cancel` lay ra (`remove`) roi spawn mot task NEN doi `join` (khong chan lenh) thay vi giu
/// mai; run hoan tat KHONG bi huy (khong ai goi `run_cancel`) hien KHONG tu don khoi map nay — chap
/// nhan duoc trong pham vi Task 9 (moi `RunHandle` chi la `String` + `CancellationToken` +
/// `JoinHandle`, xem bao cao "concerns").
pub struct Ctx<D: BrowserDriver + 'static> {
    pub engine: Arc<Engine<D>>,
    /// Ban sao `LimiterHandle` (`Clone`, cung mot `watch::Sender` voi ban Engine giu noi bo) — Task
    /// 9 KHONG dung truc tiep (`settings::settings_set_impl` chi goi `Engine::apply_settings`, da
    /// tu `set_threads` qua ban NOI BO cua no — goi lai o day se trung, xem doc do). Task 10 (lenh
    /// tai khoan) CUNG khong dung truc tiep truong nay: `Engine::{set_scheduling, delete,
    /// import_cookies, import_legacy}` deu tu goi `Engine::refresh_account_count` (dung ban
    /// `LimiterHandle` NOI BO cua Engine, khong phai ban sao nay) — giu lai phong khi mot lenh
    /// tuong lai can doi `threads`/`accounts` truc tiep tu `Ctx` ma khong qua `Engine`.
    #[allow(
        dead_code,
        reason = "chua co lenh nao (Task 9-10) doc/ghi truc tiep - moi thay doi accounts/threads deu qua Engine::apply_settings/refresh_account_count"
    )]
    pub handle: LimiterHandle,
    pub db: Db,
    /// Cai dat hien hanh, cache trong bo nho (dong bo boi `settings::settings_set_impl` khi luu) —
    /// tranh doc lai DB moi lan UI can hien thi.
    pub settings: RwLock<AppSettings>,
    pub runs: DashMap<String, RunHandle>,
}

/// Ho tro dung chung cho test cac lenh (`workflow.rs`/`tasks.rs`/`settings.rs`) — dung
/// `bootstrap::build_engine` (Task 8, THUAN + `FakeDriver`) roi tu boc thanh `Ctx`, tranh moi file
/// test phai chep lai logic tao thu muc tam/DB/engine.
#[cfg(test)]
pub(crate) mod test_support {
    use lvc_browser::FakeDriver;
    use lvc_store::AccountRepo;

    use super::{Arc, Ctx, DashMap, RwLock};
    use crate::bootstrap::build_engine;

    /// `Ctx<FakeDriver>` + thu muc tam (giu song de DB/thu muc con hieu luc) + driver (doc
    /// `driver.calls` khi test can). `accounts`: ten tai khoan duoc tao SAU khi engine da dung (mo
    /// phong nguoi dung them tai khoan) roi goi `refresh_account_count` — can it nhat 1 tai khoan de
    /// `run_graph` chay video that su (khong roi vao `AllAccountsLimited`/khong tai khoan).
    pub(crate) async fn ctx_with(
        steps: Vec<lvc_browser::FakeStep>,
        accounts: &[&str],
    ) -> (tempfile::TempDir, Ctx<FakeDriver>, Arc<FakeDriver>) {
        let dir = tempfile::tempdir().unwrap();
        let driver = Arc::new(FakeDriver::new(steps));
        let boot = build_engine(dir.path(), Arc::clone(&driver)).await.unwrap();

        if !accounts.is_empty() {
            let repo = AccountRepo::new(&boot.db);
            for name in accounts {
                std::fs::create_dir_all(dir.path().join("accounts").join(name)).unwrap();
                repo.upsert(name).await.unwrap();
            }
            boot.engine.refresh_account_count().await.unwrap();
        }

        let ctx = Ctx {
            engine: boot.engine,
            handle: boot.handle,
            db: boot.db,
            settings: RwLock::new(boot.settings),
            runs: DashMap::new(),
        };
        (dir, ctx, driver)
    }
}

#[cfg(test)]
mod tests {
    use lvc_core::graph::GraphError;
    use lvc_engine::EngineError;
    use lvc_store::StoreError;

    use super::*;

    #[test]
    fn ipc_error_new_khong_details() {
        let e = IpcError::new("FOO", "bar");
        assert_eq!(e.code, "FOO");
        assert_eq!(e.message, "bar");
        assert_eq!(e.details, None);
    }

    #[test]
    fn ipc_error_serialize_camel_case() {
        let e = IpcError::new("FOO", "bar").with_details(serde_json::json!({"x": 1}));
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"code": "FOO", "message": "bar", "details": {"x": 1}})
        );
    }

    #[test]
    fn engine_error_graph_ra_graph_invalid_kem_details() {
        let errs = vec![GraphError::Empty];
        let e: IpcError = EngineError::Graph(errs.clone()).into();
        assert_eq!(e.code, "GRAPH_INVALID");
        assert_eq!(e.details, Some(serde_json::to_value(&errs).unwrap()));
    }

    #[test]
    fn engine_error_task_not_found_ra_ma_rieng() {
        let e: IpcError = EngineError::TaskNotFound("t1".to_string()).into();
        assert_eq!(e.code, "TASK_NOT_FOUND");
        assert_eq!(e.details, None);
    }

    #[test]
    fn engine_error_khac_dung_failure_code_khi_co() {
        let e: IpcError = EngineError::NoFreeProfile.into();
        assert_eq!(e.code, "PROFILE_BUSY");
    }

    #[test]
    fn engine_error_khong_failure_code_ra_ma_chung_engine() {
        let e: IpcError = EngineError::Store(StoreError::BadPath("x".to_string())).into();
        assert_eq!(e.code, "ENGINE");
    }

    #[test]
    fn store_error_ra_ma_chung_store() {
        let e: IpcError = StoreError::BadPath("x".to_string()).into();
        assert_eq!(e.code, "STORE");
    }

    #[test]
    fn account_error_dung_ma_tu_code() {
        let e: IpcError = lvc_engine::AccountError::AccountBusy("acc1".to_string()).into();
        assert_eq!(e.code, "ACCOUNT_BUSY");
        assert!(e.message.contains("acc1"));
    }
}
