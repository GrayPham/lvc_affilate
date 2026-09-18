//! Miền thuần của LVC Seedance: chế độ, cảnh, định dạng TXT.
//! Không I/O, không DB, không trình duyệt — test được không tốn lượt Dola.

pub mod browser_kind;
pub mod bulk;
pub mod chain;
pub mod cookie;
pub mod dola_text;
pub mod error;
pub mod failure;
pub mod graph;
pub mod limits;
pub mod mode;
pub mod pro_prompt;
pub mod progress;
pub mod proxy;
pub mod scene;
pub mod scene_txt;
pub mod scheduling;
pub mod session;
pub mod settings;
pub mod value;
pub mod video_url;

pub use browser_kind::BrowserKind;
pub use bulk::{bulk_scenes, split_bulk};
pub use chain::{
    ChainPoll, ConvPick, LOGIN_INVALID_CODES, VIDEO_ID_RE, chain_http_status, link_key,
    parse_chain, pick_new_conv_id, video_ids,
};
pub use cookie::{
    Cookie, CookieError, DOLA_URL, has_dola_sessionid, parse_netscape, sessionid_expires,
    validate_dola_cookies,
};
pub use dola_text::{
    ConfirmKind, DolaTextClass, PORTRAIT_NOTE, REFUSAL_NOTE, agent_question, classify_dola_text,
    confirmation_reply, is_generating, is_portrait_hint, looks_like_json, needs_confirmation,
    policy_rejection, refusal_snippet,
};
pub use error::{CoreError, SceneError};
pub use failure::FailureCode;
pub use graph::{
    Edge, Graph, GraphError, Node, NodeKind, PortSpec, PortType, RunPlan, SaveJob, Step, VideoJob,
    plan, topo_order, validate,
};
pub use limits::Limits;
pub use mode::{Mode, route_30s, video_timeout};
pub use pro_prompt::{agent_prompt, pro_max_prompt, pro_prompt};
pub use progress::{BatchProgress, TaskStatus, batch_progress};
pub use proxy::{DIRECT, ProxySpec, effective_proxy, mask_proxy, parse_proxy};
pub use scene::{DEFAULT_DURATION, DURATIONS, RawScene, Scene};
pub use scene_txt::{format_txt, normalize_prompt_line, parse_txt};
pub use scheduling::{
    LIMIT_RESET_TZ_OFFSET_S, fit_duration_to_mode, mode_fallback_chain, next_limit_reset, tokyo_day,
};
pub use session::{
    DOLA_REGION_RESTRICTED, REVOKED_HINT, SESSION_PROBE_JS, SessionState, classify_session,
    dola_self_logout, has_login_marker, is_region_restricted,
};
pub use settings::AppSettings;
pub use value::{LogoMode, Model, Ratio};
pub use video_url::{
    RAW_PARAMS, RawCandidate, decode_main_url, extract_unwatermarked_url, find_fallback_apis,
    is_watermarked_url, pick_raw_video, replace_query,
};
