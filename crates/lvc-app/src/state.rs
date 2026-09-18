//! Trang thai dung chung toan ung dung (`app.manage`, P3 Task 8) — Tauri command (Task 9-10) doc
//! qua `tauri::State<AppState>`. Task 9: `AppState` la ALIAS cua [`commands::Ctx`]`<AppDriver>`
//! — struct that su (`engine`/`handle`/`db`/`settings`/`runs`) nam o `commands/mod.rs`, generic tren
//! `BrowserDriver` de than logic cua tung lenh test duoc bang `FakeDriver` khong can runtime Tauri
//! (xem doc `Ctx`). Dinh nghia tai day chi con la lop dat ten mong cho driver THAT (`AppDriver`).
//!
//! Task 12a (P3, che do dev khong ton credit): `AppDriver` doi KIEU luc BIEN DICH theo cargo
//! feature `fake-driver` cua crate nay (KHONG phai runtime flag) — ban thuong (`cargo tauri dev`,
//! ban release) dung `ChromiumDriver` that (Dola that, TON CREDIT); bat feature
//! (`cargo tauri dev --features fake-driver`) doi sang `lvc_browser::fake::FakeDriver`, du
//! `bootstrap::make_driver` khoi tao bang [`FakeDriver::endless_ok`] (kich ban "moi thu deu Ok
//! sau vai giay", xem doc ham do) — luong UI (submit -> poll -> tai video) thu duoc DAY DU ma
//! KHONG goi Dola. Ban release build KHONG bao gio bat feature nay (`[features] fake-driver = []`
//! trong `Cargo.toml`, mac dinh tat).

#[cfg(feature = "fake-driver")]
pub type AppDriver = lvc_browser::fake::FakeDriver;
#[cfg(not(feature = "fake-driver"))]
pub type AppDriver = lvc_browser::chromium::ChromiumDriver;

use crate::commands::Ctx;

pub type AppState = Ctx<AppDriver>;
