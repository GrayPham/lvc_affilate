//! `lvc-app`: vo Tauri 2 boc `lvc-engine` — TAT CA giao tiep frontend/backend di qua lenh Tauri
//! (`invoke_handler`) va su kien (`emit`), KHONG may chu HTTP nao (ADR P3). Task 8: scaffold — state
//! (`state.rs`), duong dan (`paths.rs`), khoi dong tang duoi (`bootstrap.rs`), cau su kien
//! (`events.rs`), chinh sach log (`log.rs`). Task 9: lenh workflow/run/task/cai dat (`commands/`)
//! dang ky vao `invoke_handler` duoi day; Task 10 them lenh tai khoan (xem `commands/mod.rs`); Task
//! 11 la giao dien React trong `app/` (build ra `app/dist`, xem `build.rs`).
//!
//! **Task 12a (P3) — che do dev khong ton credit:** `cargo tauri dev --features fake-driver`
//! (chay tu `crates/lvc-app`) doi driver trinh duyet sang `lvc_browser::fake::FakeDriver::endless_ok`
//! (xem `state::AppDriver` / `bootstrap::make_driver`) — moi submit/poll deu Ok, video "tai xong"
//! sau vai giay (file placeholder VAI BYTE, KHONG PHAI VIDEO THAT), KHONG bao gio goi Dola. Log se
//! co dong `tracing::warn!` "CHE DO GIA LAP" ngay dau `logs/app.log` de nhan biet. Can it nhat MOT
//! tai khoan de engine chon (`FakeDriver` khong tu bia tai khoan) — tao bang tab Tai khoan (import
//! cookie bat ky, vi `FakeDriver::probe_session` luon tra `LoggedIn`) hoac thu cong
//! `mkdir accounts/<ten>` + mot dong trong bang `accounts`. Ban release KHONG bao gio bat feature
//! nay (`fake-driver` tat mac dinh trong `Cargo.toml`).

mod bootstrap;
mod commands;
mod events;
mod log;
mod museai;
mod paths;
pub mod registry;
mod state;
mod textgen;
mod veo3;

use tauri::Manager;

/// Boc `WorkerGuard` de quan ly trong Tauri state (`app.manage`) thay vi `Box::leak` — song dung
/// het doi `App`, `Drop` flush kenh ghi log non-blocking khi ung dung thoat (ca khi thoat binh
/// thuong lan khi `.setup()` loi som truoc khi co cua so nao, vi `App` bi drop ngay khi do).
#[allow(
    dead_code,
    reason = "chi giu WorkerGuard song qua RAII (Tauri state), truong .0 khong can doc truc tiep"
)]
struct LogGuard(tracing_appender::non_blocking::WorkerGuard);

/// Diem vao thuc su — `main.rs` chi goi ham nay (quy uoc Tauri 2: lib ten `<package>_lib`).
///
/// # Panics
/// Khoi dong tang duoi (mo/migrate DB, tao `Engine`) hoac chinh `tauri::Builder::run` that bai ->
/// panic co chu dinh: ung dung khong the chay thieu tang duoi, va day la giai doan `setup()`/`run()`
/// truoc khi co man hinh nao de bao loi cho nguoi dung (Task 9 se xet doi thanh man hinh loi thay vi
/// panic cung khi co giao dien). Loi khoi dong tang duoi (`bootstrap::run_bootstrap`) duoc log bang
/// `tracing::error!` TRUOC khi propagate — ban release khong co console (xem `main.rs`) nen day dong
/// duy nhat con lai trong `logs/app.log` giai thich vi sao ung dung khong mo duoc.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        // P8 (auto-update): check()/downloadAndInstall() goi tu frontend qua
        // @tauri-apps/plugin-updater — xem tauri.conf.json plugins.updater.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let handle = app.handle().clone();

            let base = paths::app_data_dir(&handle)?;
            let guard = log::init(&paths::logs_dir(&base))?;
            // Quan ly trong Tauri state (thay vi Box::leak) — xem doc `LogGuard`. Phai quan ly
            // TRUOC khi goi bootstrap de guard chac chan con song (va se flush luc App bi drop)
            // du bootstrap co loi hay khong.
            app.manage(LogGuard(guard));

            tracing::info!(base = %base.display(), "lvc-app: bat dau khoi dong tang duoi");
            tauri::async_runtime::block_on(bootstrap::run_bootstrap(&handle)).inspect_err(|e| {
                tracing::error!(error = %e, "khoi dong that bai");
            })?;

            let state = handle.state::<state::AppState>();
            events::spawn_bridge(handle.clone(), &state.engine);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::workflow::node_catalog,
            commands::workflow::workflow_validate,
            commands::workflow::workflow_save,
            commands::workflow::workflow_list,
            commands::workflow::workflow_get,
            commands::workflow::workflow_delete,
            commands::workflow::run_start,
            commands::workflow::run_cancel,
            commands::workflow::run_list,
            commands::workflow::run_node_states,
            commands::workflow::run_node_outputs,
            commands::tasks::task_list,
            commands::tasks::task_cancel,
            commands::tasks::task_resume,
            commands::tasks::task_open_file,
            commands::scripts::script_list,
            commands::scripts::script_create,
            commands::scripts::script_update,
            commands::scripts::script_delete,
            commands::scripts::script_get,
            commands::scripts::script_put_scenes,
            commands::scripts::script_run,
            commands::scripts::script_reset_scenes,
            commands::scripts::script_import_txt,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::pick_directory,
            commands::settings::pick_files,
            commands::media::frame_save_png,
            commands::media::trim_video,
            commands::accounts::account_list,
            commands::accounts::account_import_cookies,
            commands::accounts::account_import_cookie_files,
            commands::accounts::account_verify,
            commands::accounts::account_set_scheduling,
            commands::accounts::account_set_proxy,
            commands::accounts::account_delete,
            commands::accounts::account_open_viewer,
            commands::accounts::legacy_import,
            commands::proxy::proxy_test,
            commands::credentials::credential_list,
            commands::credentials::credential_add,
            commands::credentials::credential_update,
            commands::credentials::credential_verify,
            commands::credentials::credential_delete,
            commands::credentials::credential_open_viewer,
            commands::credentials::credential_set_node_types,
            commands::credentials::credential_set_daily_limit,
            commands::login::cookie::login_import_cookie_files,
            commands::login::cookie::login_cookie_text,
            commands::login::interactive::login_open_interactive,
            commands::templates::template_list,
            commands::templates::template_get,
            commands::templates::template_save_private,
            commands::templates::template_delete_private,
            commands::publish::publish_workflow,
            commands::updates::workflow_update_list,
            commands::updates::workflow_set_origin,
            commands::updates::workflow_apply_update,
            commands::device::device_id,
            commands::cache::cache_info,
            commands::cache::cache_clear,
            commands::cache::open_app_folder,
            commands::queue::queue_list,
            commands::queue::queue_add,
            commands::queue::queue_remove,
            commands::queue::queue_reorder,
            commands::queue::queue_clear_finished,
            commands::queue::queue_start,
            commands::queue::queue_stop,
        ])
        .run(tauri::generate_context!())
        .expect("loi khi chay ung dung Tauri");
}
