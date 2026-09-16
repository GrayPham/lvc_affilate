//! Tầng lưu trữ: `SQLite` qua sqlx, migration, và bộ nhập dữ liệu Python cũ.

pub mod accounts;
pub mod db;
pub mod error;
pub mod settings;
pub mod tasks;

pub use accounts::{AccountRepo, AccountRow};
pub use db::Db;
pub use error::StoreError;
pub use settings::SettingsRepo;
pub use tasks::{BatchRef, TaskRepo, TaskRow};
