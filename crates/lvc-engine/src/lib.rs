//! `lvc-engine`: bo dieu phoi (scheduler) cho luong tao video Dola (P2 Task 15-17).
//!
//! Task 15 (crate nay khoi tao): [`limiter::DynamicLimiter`] (spec §5.1) va [`error::EngineError`].
//! Task 16: [`pool::AccountPool`] (spec §5.2 — chon/xoay/khoa/han muc tai khoan).
//! Task 17: [`generate::Engine`] (`generate_video` — dieu phoi mot video: slot → tai khoan → phien
//! trinh duyet → gui → cho video → ghi DB) va [`session_cache::SessionCache`] (phien am theo tai
//! khoan + reaper).

pub mod error;
pub mod generate;
pub mod limiter;
pub mod pool;
pub mod session_cache;

pub use error::EngineError;
pub use generate::{Engine, EngineConfig, GenerateOutcome, now_unix};
pub use limiter::{DynamicLimiter, LimiterHandle, Permit};
pub use pool::{AccountPool, Lease};
pub use session_cache::SessionCache;
