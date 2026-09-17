//! Driver trinh duyet cho LVC Seedance (ADR-R002): tach mien thuan (`lvc-core`) khoi
//! CDP (chromiumoxide) sau mot trait de doi duoc driver neu bi Dola chan.
//!
//! **Chinh sach log (P2):** KHONG bat `chromiumoxide` o muc `debug`/`trace` — crate do in nguyen
//! payload CDP, gom ket qua `Network.getCookies` (gia tri cookie/sessionid) va URL co chu ky.
//! P3 khi cai `tracing_subscriber` phai noi directive `chromiumoxide=warn` SAU directive cua
//! nguoi dung. Ma trong crate nay khong bao gio log gia tri cookie (chi ten) o bat ky muc nao.

pub mod chromium;
pub mod driver;
pub mod error;
pub mod executable;
pub mod ops;
pub mod profile;

pub use driver::{BrowserDriver, LaunchOpts, SessionProbe};
pub use error::BrowserError;
pub use executable::{
    SearchRoots, ms_playwright_chromium_dirs, resolve_executable, resolve_executable_in,
};
