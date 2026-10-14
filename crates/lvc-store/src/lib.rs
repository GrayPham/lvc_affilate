//! Tầng lưu trữ: `SQLite` qua sqlx, migration, và bộ nhập dữ liệu Python cũ.

pub mod accounts;
pub mod credentials;
pub mod db;
pub mod error;
pub mod legacy;
pub mod queue;
pub mod remote_scripts;
pub mod remote_templates;
pub mod scripts;
pub mod settings;
pub mod tasks;
pub mod templates;
pub mod workflow_origin;
pub mod workflows;

pub use accounts::{AccountRepo, AccountRow};
pub use credentials::{CredentialRepo, CredentialRow, NewCredential};
pub use db::Db;
pub use error::StoreError;
pub use legacy::{ImportReport, import_legacy};
pub use queue::{NewQueueItem, QueueRepo, QueueRow, QueueStatus};
pub use remote_scripts::{NewRemoteScript, RemoteScriptRepo, RemoteScriptRow};
pub use remote_templates::{NewRemoteTemplate, RemoteTemplateRepo, RemoteTemplateRow};
pub use scripts::{ReplaceScenesError, SceneIn, SceneRow, ScriptMeta, ScriptRepo, ScriptRow};
pub use settings::SettingsRepo;
pub use tasks::{BatchRef, TaskRepo, TaskRow};
pub use templates::{TemplateRepo, TemplateRow};
pub use workflow_origin::{WorkflowOrigin, WorkflowOriginRepo};
pub use workflows::{RunRepo, RunRow, RunStatus, WorkflowMeta, WorkflowRepo, WorkflowRow};
