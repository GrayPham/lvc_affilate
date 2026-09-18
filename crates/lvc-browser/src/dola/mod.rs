//! Lop Dola tren `ops` (P2): nhan/JS/regex chep nguyen van tu Python (`labels`), loi co ma
//! (`error`), luong phien (`session`), composer (`compose`, Task 7), anh tham chieu (`refs`, Task 8),
//! gui prompt Nhanh -> conversation id (`submit`, Task 9), HTTP poll/fetch/download bang reqwest
//! (`http`, Task 10), vong poll + quyet dinh tai (`poll`, Task 11); Task 12 them skill. `util`:
//! dau thoi gian ten file (UTC).

pub mod compose;
pub mod error;
pub mod http;
pub mod labels;
pub mod poll;
#[cfg(test)]
pub(crate) mod py_src;
pub mod refs;
pub mod session;
pub mod submit;
pub(crate) mod util;

pub use error::DolaError;
pub use http::{DolaHttp, POLL_PARAMS, PullOutcome, output_path};
pub use poll::{
    NoopHooks, PollHooks, PollOpts, PollPage, VideoArtifact, poll_and_download,
    poll_and_download_with,
};
pub use submit::{
    ConversationId, SubmitCtx, SubmitRequest, captcha_error, has_captcha, submit, submit_at,
    submit_timeout,
};
