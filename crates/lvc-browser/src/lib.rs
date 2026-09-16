//! Driver trinh duyet cho LVC Seedance (ADR-R002): tach mien thuan (`lvc-core`) khoi
//! CDP (chromiumoxide) sau mot trait de doi duoc driver neu bi Dola chan.

pub mod chromium;
pub mod driver;
pub mod error;
pub mod profile;

pub use driver::{BrowserDriver, LaunchOpts, SessionProbe};
pub use error::BrowserError;
