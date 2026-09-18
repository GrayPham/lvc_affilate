//! `lvc-engine`: bo dieu phoi (scheduler) cho luong tao video Dola (P2 Task 15-17).
//!
//! Task 15 (crate nay khoi tao): [`limiter::DynamicLimiter`] (spec §5.1) va [`error::EngineError`].
//! Task 16: [`pool::AccountPool`] (spec §5.2 — chon/xoay/khoa/han muc tai khoan).
//! Task 17: [`generate::Engine`] (`generate_video` — dieu phoi mot video: slot → tai khoan → phien
//! trinh duyet → gui → cho video → ghi DB) va [`session_cache::SessionCache`] (phien am theo tai
//! khoan + reaper).
//! Task 18: [`cli`] — bo dieu khien 1 video (`examples/gen_one.rs` la `main` mong).
//!
//! P3 Task 4: [`cancel::CancelRegistry`] (huy bang `CancellationToken`, R4) va
//! [`events::EngineEvents`] (`EngineEvent` qua broadcast, R5) — `Engine::cancel`, `Engine::events`.
//! P3 Task 5: [`settings`] — `EngineConfig::from_settings` + `Engine::apply_settings`, cau hinh tu
//! `AppSettings` (bang `settings`, `lvc-store::settings`).
//! P3 Task 6: [`resume`] — `Engine::resume` (tiep tuc task da co `conversation_id` sau khi khoi
//! dong lai, KHONG gui lai prompt/KHONG claim) va `Engine::requeue_on_start` (chia orphan thanh
//! requeue-ngay vs ung-vien-resume, khong tu chay).
//! P3 Task 7: [`dag`] — `Engine::run_graph` (chay ca do thi: fan-out video qua `DynamicLimiter`,
//! gom ket qua, Save, phat `NodeState`/`RunDone`, ghi `runs`) va `Engine::cancel_run`.
//! P3 Task 10: [`accounts`] — `Engine::{list_with_status, import_cookies, verify, set_scheduling,
//! set_proxy, delete, import_legacy}` (tab Tai khoan UI: liet ke + trang thai, nhap cookie, kiem tra
//! dang nhap, bat/tat lich, sua proxy, xoa, nhap du lieu ban Python cu).
//! P3c Task 1: [`scripts`] — `Engine::{run_script, import_txt_to_script, delete_script}` (tab Kich
//! ban UI: chay/chay lai cac canh da chon qua DAG co san, nhap TXT thay toan bo canh, xoa kich ban
//! co bao ve canh dang chay).

pub mod accounts;
pub mod cancel;
pub mod cli;
pub mod dag;
pub mod error;
pub mod events;
pub mod generate;
pub mod limiter;
pub mod pool;
pub mod resume;
pub mod scripts;
pub mod session_cache;
pub mod settings;

pub use accounts::{
    AccountError, AccountStatus, ImportCookiesReport, LegacyImportReport, VerifyReport,
};
pub use cancel::{CancelGuard, CancelRegistry};
pub use dag::{HooksFactory, NodeOutputs, RunHandle, noop_hooks_factory};
pub use error::EngineError;
pub use events::{EVENT_CAPACITY, EngineEvent, EngineEvents, next_event};
pub use generate::{Engine, EngineConfig, GenerateOutcome, now_unix};
pub use limiter::{DynamicLimiter, LimiterHandle, Permit};
pub use pool::{AccountPool, Lease};
pub use resume::RequeueReport;
pub use scripts::{ScriptError, ScriptRunError, ScriptRunReport, ScriptRunTask};
pub use session_cache::SessionCache;
