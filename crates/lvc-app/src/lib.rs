//! `lvc-app`: vo Tauri 2 boc `lvc-engine` — TAT CA giao tiep frontend/backend di qua lenh Tauri
//! (`invoke_handler`) va su kien (`emit`), KHONG may chu HTTP nao (ADR P3). Task 8: scaffold — state
//! (`state.rs`), duong dan (`paths.rs`), khoi dong tang duoi (`bootstrap.rs`), cau su kien
//! (`events.rs`), chinh sach log (`log.rs`). Task 9-10 them lenh vao `invoke_handler`; Task 11 la
//! giao dien React trong `app/` (build ra `app/dist`, xem `build.rs`).

mod bootstrap;
mod events;
mod log;
mod paths;
mod state;

use tauri::Manager;

/// Diem vao thuc su — `main.rs` chi goi ham nay (quy uoc Tauri 2: lib ten `<package>_lib`).
///
/// # Panics
/// Khoi dong tang duoi (mo/migrate DB, tao `Engine`) hoac chinh `tauri::Builder::run` that bai ->
/// panic co chu dinh: ung dung khong the chay thieu tang duoi, va day la giai doan `setup()`/`run()`
/// truoc khi co man hinh nao de bao loi cho nguoi dung (Task 9 se xet doi thanh man hinh loi thay vi
/// panic cung khi co giao dien).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();

            let base = paths::app_data_dir(&handle)?;
            let guard = log::init(&paths::logs_dir(&base))?;
            // `setup()` tra ve truoc khi ung dung chay tiep nen khong the giu `guard` trong bien
            // cuc bo (se bi drop ngay, dong kenh ghi log non-blocking). Leak co chu dinh de guard
            // song het doi tien trinh — cai gia la mat vai dong log cuoi neu tien trinh bi ket
            // thuc dot ngot (khong flush kip), chap nhan duoc cho ung dung desktop.
            let _ = Box::leak(Box::new(guard));

            tracing::info!(base = %base.display(), "lvc-app: bat dau khoi dong tang duoi");
            tauri::async_runtime::block_on(bootstrap::run_bootstrap(&handle))?;

            let state = handle.state::<state::AppState>();
            events::spawn_bridge(handle.clone(), &state.engine);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("loi khi chay ung dung Tauri");
}
