//! Tầng lưu trữ: `SQLite` qua sqlx, migration, và bộ nhập dữ liệu Python cũ.

pub mod accounts;
pub mod db;
pub mod error;
pub mod legacy;
pub mod settings;
pub mod tasks;
pub mod workflows;

pub use accounts::{AccountRepo, AccountRow};
pub use db::Db;
pub use error::StoreError;
pub use legacy::{ImportReport, import_legacy};
pub use settings::SettingsRepo;
pub use tasks::{BatchRef, TaskRepo, TaskRow};
pub use workflows::{RunRepo, RunRow, WorkflowMeta, WorkflowRepo, WorkflowRow};
