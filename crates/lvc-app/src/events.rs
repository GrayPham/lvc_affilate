//! Cau (bridge) su kien engine -> Tauri (P3 Task 8, xem `lvc_engine::events` dau file). Forward
//! TUNG [`lvc_engine::EngineEvent`] tu `EngineEvents::subscribe()` thanh `app.emit(EVENT_CHANNEL,
//! ev)`; `Lagged` (receiver cham, mat su kien cu) da duoc `lvc_engine::events::next_event` nuot +
//! log warn NEN o day chi con nhanh dung: kenh dong (`Closed`, `Engine` bi drop — binh thuong luc
//! ung dung thoat) thi vong lap ket thuc, task nen tu ket thuc theo.

use std::sync::Arc;

use lvc_browser::BrowserDriver;
use lvc_engine::Engine;
use lvc_engine::events::next_event;
use tauri::{AppHandle, Emitter};
use tracing::warn;

/// Ten kenh IPC phia frontend lang nghe (`ipc/types.ts`, Task 11).
pub const EVENT_CHANNEL: &str = "engine://event";

/// Spawn mot task nen forward moi su kien cua `engine` qua [`EVENT_CHANNEL`]. Chay den khi kenh
/// dong (khong co cach huy tu ben ngoai — vong doi gan voi `engine`, dung khi engine bi drop).
pub fn spawn_bridge<D: BrowserDriver + 'static>(app: AppHandle, engine: &Arc<Engine<D>>) {
    let engine = Arc::clone(engine);
    tauri::async_runtime::spawn(async move {
        let mut rx = engine.events().subscribe();
        while let Some(ev) = next_event(&mut rx).await {
            if let Err(e) = app.emit(EVENT_CHANNEL, ev) {
                warn!(error = %e, "khong emit duoc su kien engine cho frontend");
            }
        }
    });
}
