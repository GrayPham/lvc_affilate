//! Miền thuần của LVC Seedance: chế độ, cảnh, định dạng TXT.
//! Không I/O, không DB, không trình duyệt — test được không tốn lượt Dola.

pub mod error;
pub mod limits;
pub mod mode;
pub mod scene;
pub mod scene_txt;
pub mod value;

pub use error::{CoreError, SceneError};
pub use limits::Limits;
pub use mode::Mode;
pub use scene::{DEFAULT_DURATION, DURATIONS, RawScene, Scene};
pub use scene_txt::{format_txt, parse_txt};
pub use value::{LogoMode, Model, Ratio};
