//! `lvc-engine`: bo dieu phoi (scheduler) cho luong tao video Dola (P2 Task 15-17).
//!
//! Task 15 (crate nay khoi tao): [`limiter::DynamicLimiter`] (spec §5.1) va [`error::EngineError`].
//! Task 16 (`AccountPool`) va Task 17 (`generate_video`/`SessionCache`) se them module vao day.

pub mod error;
pub mod limiter;

pub use error::EngineError;
pub use limiter::{DynamicLimiter, LimiterHandle, Permit};
