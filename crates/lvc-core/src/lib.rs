//! Miền thuần của LVC Seedance: chế độ, cảnh, định dạng TXT.
//! Không I/O, không DB, không trình duyệt — test được không tốn lượt Dola.

pub mod error;
pub mod limits;

pub use error::{CoreError, SceneError};
pub use limits::Limits;
