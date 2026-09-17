//! Lop Dola tren `ops` (P2): nhan/JS/regex chep nguyen van tu Python (`labels`), loi co ma
//! (`error`), luong phien (`session`), composer (`compose`, Task 7), anh tham chieu (`refs`, Task 8),
//! gui prompt Nhanh -> conversation id (`submit`, Task 9); Task 10-12 them http/poll/skill.

pub mod compose;
pub mod error;
pub mod labels;
#[cfg(test)]
pub(crate) mod py_src;
pub mod refs;
pub mod session;
pub mod submit;

pub use error::DolaError;
pub use submit::{
    ConversationId, SubmitCtx, SubmitRequest, captcha_error, has_captcha, submit, submit_at,
    submit_timeout,
};
