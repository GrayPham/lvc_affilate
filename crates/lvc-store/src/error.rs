#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("loi sqlite: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("loi migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("duong dan khong hop le: {0}")]
    BadPath(String),
    #[error("loi JSON: {0}")]
    Json(#[from] serde_json::Error),
}
