//! Tầng lưu trữ: `SQLite` qua sqlx, migration, và bộ nhập dữ liệu Python cũ.

pub mod db;
pub mod error;
pub mod tasks;

pub use db::Db;
pub use error::StoreError;
pub use tasks::{BatchRef, TaskRepo, TaskRow};
