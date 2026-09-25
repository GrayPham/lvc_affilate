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
//!
//! **Ten tham so IPC (M2, fix-round 1)**: khong `#[tauri::command]` nao o day dung `rename_all` —
//! Tauri 2 TU DOI ten tham so Rust (`snake_case`) sang camelCase cho phia JS (vd `run_start(workflow_id,
//! graph)` goi tu JS la `invoke("run_start", { workflowId, graph })`); `settings_set`'s tham so `s`
//! (khong doi ten `settings` — Task 11 dang mirror `{ s }`, xem review fix-round 1 M2) van goi la
//! `invoke("settings_set", { s })`. Ten LENH (command) thi giu nguyen `snake_case`, khong doi.

pub mod accounts;
pub mod credentials;
pub mod scripts;
pub mod settings;
pub mod tasks;
pub mod templates;
pub mod workflow;

#[cfg(test)]
mod fixtures;

use std::sync::Arc;

use dashmap::{DashMap, DashSet};
use lvc_browser::BrowserDriver;
use lvc_core::AppSettings;
use lvc_engine::{AccountError, Engine, EngineError, LimiterHandle};
use lvc_store::{Db, StoreError};
use lvc_vault::{Vault, VaultRepoError};
use serde::Serialize;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Loi tra ve cho MOI lenh IPC. `Serialize` camelCase — `details` LUON co mat trong JSON (`null`
/// khi khong co, KHONG `skip_serializing_if`) de frontend khong phai kiem `"details" in err`.
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
/// to node/canh loi tu day, Task 11). `TaskNotFound` -> `TASK_NOT_FOUND`. `Store` -> `STORE` (M1,
/// fix-round 1 — CUNG loi ha tang, KHONG duoc ra ma khac tuy no di qua `EngineError` hay
/// `StoreError` truc tiep, dung LAI `From<StoreError>` thay vi tu ghi lai). Con lai -> `failure_code()`
/// khi co (vd `PROXY_DOWN`, `429`), khong thi ma chung `ENGINE`.
impl From<EngineError> for IpcError {
    fn from(e: EngineError) -> Self {
        let msg = e.to_string();
        match e {
            EngineError::Graph(errs) => {
                let details = serde_json::to_value(&errs).unwrap_or(serde_json::Value::Null);
                Self::new("GRAPH_INVALID", msg).with_details(details)
            }
            EngineError::TaskNotFound(_) => Self::new("TASK_NOT_FOUND", msg),
            EngineError::Store(store_err) => Self::from(store_err),
            other => {
                let code = other
                    .failure_code()
                    .map_or_else(|| "ENGINE".to_string(), |c| c.as_str().to_string());
                Self::new(code, msg)
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

/// P4c (`lvc_vault`): loi kho tai khoan AI — `code()` on dinh (giong `AccountError`).
impl From<VaultRepoError> for IpcError {
    fn from(e: VaultRepoError) -> Self {
        Self::new(e.code(), e.to_string())
    }
}

/// P3c Task 1 (`lvc_engine::scripts`): `Engine::import_txt_to_script`/`Engine::delete_script`
/// dung loi rieng nay (khong phai `EngineError`, xem doc `lvc_engine::scripts::ScriptError`) —
/// `Store` di qua CUNG anh xa `STORE` voi moi `StoreError` khac trong file nay.
impl From<lvc_engine::ScriptError> for IpcError {
    fn from(e: lvc_engine::ScriptError) -> Self {
        use lvc_engine::ScriptError as E;
        match e {
            E::Store(se) => Self::from(se),
            E::SceneInvalid(msg) => Self::new("SCENE_INVALID", msg),
            E::SceneRunning(ids) => Self::new(
                "SCENE_RUNNING",
                format!("canh dang chay: {}", ids.join(", ")),
            ),
        }
    }
}

/// P3c Task 1 (`lvc_store::scripts`): `script_put_scenes` goi thang `ScriptRepo::replace_scenes`
/// (khong qua `Engine`, giong `workflow_save`/`workflow_get`) nen can anh xa rieng — CUNG hai
/// nhanh `STORE`/`SCENE_RUNNING` nhu `From<lvc_engine::ScriptError>` o tren.
impl From<lvc_store::ReplaceScenesError> for IpcError {
    fn from(e: lvc_store::ReplaceScenesError) -> Self {
        use lvc_store::ReplaceScenesError as E;
        match e {
            E::Store(se) => Self::from(se),
            E::SceneRunning(ids) => Self::new(
                "SCENE_RUNNING",
                format!("canh dang chay: {}", ids.join(", ")),
            ),
        }
    }
}

/// Phan cua `AppState` (Task 8) ma cac lenh IPC doc/ghi — xem doc dau module ve ly do generic tren
/// `D`. `runs`: `run_id` -> `CancellationToken` (fix-round 1, I1) cua lan chay dang co — CHI de
/// `run_cancel` huy nhanh; KHONG giu `JoinHandle`/`RunHandle` day du nua (`Engine` da tu quan ly
/// dieu do qua `run_tokens` rieng cua no, xem `lvc_engine::dag`) de tranh trung lap trang thai.
/// `run_start` chen token vao roi spawn MOT task "reaper" doi `join` cua `RunHandle` (Task 7) roi
/// tu `remove` khoi map nay — chay tren CA HAI duong ket thuc (hoan tat tu nhien LAN bi huy), nen
/// map luon rong lai sau khi mot run xong, khong can `run_cancel` dong gi ca (xem
/// `workflow::run_start_impl`/`workflow::run_cancel_impl`). Boc `Arc` vi task "reaper" can giu
/// tham chieu 'static toi map nay ma khong the muon `&Ctx`.
pub struct Ctx<D: BrowserDriver + 'static> {
    pub engine: Arc<Engine<D>>,
    /// P4c: kho tai khoan AI (DPAPI). Khong generic tren D (khong dung trinh duyet).
    pub vault: Arc<Vault>,
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
    pub runs: Arc<DashMap<String, CancellationToken>>,
    /// I3 (fix-round 1, final-review): `task_id` dang co MOT lan `task_resume` chay nen (giua luc
    /// spawn va luc `Engine::resume` ket thuc) — chan lan goi THU HAI cung `task_id` trong luc do
    /// (xem doc `tasks::task_resume_impl`): hai lan resume dong thoi se chia se CUNG token huy qua
    /// `CancelRegistry::register_guarded` (get-or-insert), roi `CancelGuard::drop` cua lan XONG
    /// TRUOC se xoa token do trong khi lan sau van con dang poll — huy nham job dang chay that.
    /// `DashSet` (khong phai `HashSet` sau `Mutex`) vi nhieu lenh IPC co the goi dong thoi tu nhieu
    /// luong Tauri; insert/remove don gian hon can mot registry rieng nhu `CancelRegistry`.
    pub resuming: Arc<DashSet<String>>,
}

/// Ho tro dung chung cho test cac lenh (`workflow.rs`/`tasks.rs`/`settings.rs`) — dung
/// `bootstrap::build_engine` (Task 8, THUAN + `FakeDriver`) roi tu boc thanh `Ctx`, tranh moi file
/// test phai chep lai logic tao thu muc tam/DB/engine.
#[cfg(test)]
pub(crate) mod test_support {
    use lvc_browser::FakeDriver;
    use lvc_store::AccountRepo;

    use super::{Arc, Ctx, DashMap, DashSet, RwLock};
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
            vault: Arc::new(lvc_vault::Vault::new(&boot.db, lvc_vault::default_box())),
            handle: boot.handle,
            db: boot.db,
            settings: RwLock::new(boot.settings),
            runs: Arc::new(DashMap::new()),
            resuming: Arc::new(DashSet::new()),
        };
        (dir, ctx, driver)
    }
}

#[cfg(test)]
mod tests {
    use lvc_browser::BrowserError;
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

    /// M1 (fix-round 1): `EngineError::Store` phai ra CUNG ma `STORE` voi `StoreError` truc tiep
    /// (test `store_error_ra_ma_chung_store` duoi day) — truoc do bi roi vao nhanh mac dinh `ENGINE`.
    #[test]
    fn engine_error_store_ra_ma_chung_store() {
        let e: IpcError = EngineError::Store(StoreError::BadPath("x".to_string())).into();
        assert_eq!(e.code, "STORE");
    }

    #[test]
    fn engine_error_khong_failure_code_ra_ma_chung_engine() {
        let e: IpcError = EngineError::Browser(BrowserError::Timeout("t".to_string())).into();
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
