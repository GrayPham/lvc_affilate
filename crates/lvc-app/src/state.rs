//! Trang thai dung chung toan ung dung (`app.manage`, P3 Task 8) — Tauri command (Task 9-10) doc
//! qua `tauri::State<AppState>`.

use std::sync::Arc;

use dashmap::DashMap;
use lvc_browser::chromium::ChromiumDriver;
use lvc_core::AppSettings;
use lvc_engine::{Engine, LimiterHandle};
use lvc_store::Db;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Trang thai dung chung.
///
/// `runs`: `run_id` -> token huy chay DAG (Task 7, `lvc_engine::dag::RunHandle`, dang lam song
/// song voi Task 8 nay nen CHUA co trong cay luc viet file nay) — dung tam
/// `CancellationToken` de Task 9 goi huy duoc theo `run_id`; Task 9 se thay gia tri map nay bang
/// `RunHandle` that (giu ca handle theo doi tien do, khong chi token) khi Task 7 xong.
#[allow(
    dead_code,
    reason = "Task 8 chi khai bao state; handle/db/settings/runs duoc Tauri command doc/ghi tu Task 9-10 (invoke_handler con rong o Task 8)"
)]
pub struct AppState {
    pub engine: Arc<Engine<ChromiumDriver>>,
    /// Doi `threads` luc dang chay (`LimiterHandle::set_threads`, ADR-016) — Task 9 goi tu lenh
    /// luu cai dat.
    pub handle: LimiterHandle,
    pub db: Db,
    /// Cai dat hien hanh, cache trong bo nho — Task 9 doc/ghi qua day (dong bo voi bang `settings`
    /// khi luu) thay vi truy van DB moi lan UI can hien thi.
    pub settings: RwLock<AppSettings>,
    pub runs: DashMap<String, CancellationToken>,
}
