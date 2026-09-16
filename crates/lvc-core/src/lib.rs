//! Miền thuần của LVC Seedance: chế độ, cảnh, định dạng TXT.
//! Không I/O, không DB, không trình duyệt — test được không tốn lượt Dola.

pub mod bulk;
pub mod cookie;
pub mod error;
pub mod failure;
pub mod limits;
pub mod mode;
pub mod pro_prompt;
pub mod progress;
pub mod scene;
pub mod scene_txt;
pub mod scheduling;
pub mod session;
pub mod value;

pub use bulk::{bulk_scenes, split_bulk};
pub use cookie::{Cookie, CookieError, parse_netscape, sessionid_expires, validate_dola_cookies};
pub use error::{CoreError, SceneError};
pub use failure::FailureCode;
pub use limits::Limits;
pub use mode::Mode;
pub use pro_prompt::{agent_prompt, pro_max_prompt, pro_prompt};
pub use progress::{BatchProgress, TaskStatus, batch_progress};
pub use scene::{DEFAULT_DURATION, DURATIONS, RawScene, Scene};
pub use scene_txt::{format_txt, parse_txt};
pub use scheduling::{fit_duration_to_mode, mode_fallback_chain};
pub use session::{SESSION_PROBE_JS, SessionState, classify_session, has_login_marker};
pub use value::{LogoMode, Model, Ratio};
