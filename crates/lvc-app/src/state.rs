//! Trang thai dung chung toan ung dung (`app.manage`, P3 Task 8) — Tauri command (Task 9-10) doc
//! qua `tauri::State<AppState>`. Task 9: `AppState` la ALIAS cua [`commands::Ctx`]`<ChromiumDriver>`
//! — struct that su (`engine`/`handle`/`db`/`settings`/`runs`) nam o `commands/mod.rs`, generic tren
//! `BrowserDriver` de than logic cua tung lenh test duoc bang `FakeDriver` khong can runtime Tauri
//! (xem doc `Ctx`). Dinh nghia tai day chi con la lop dat ten mong cho `ChromiumDriver` that.

use lvc_browser::chromium::ChromiumDriver;

use crate::commands::Ctx;

pub type AppState = Ctx<ChromiumDriver>;
