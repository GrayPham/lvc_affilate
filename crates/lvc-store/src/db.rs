use std::path::Path;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::StoreError;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Bọc pool `SQLite`. Bản Python dùng MỘT connection + khoá toàn cục gọi đồng bộ
/// từ event loop; ở đây là pool async thật.
#[derive(Debug, Clone)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Mở (tạo nếu chưa có) DB tại `path`, bật WAL, chạy migration.
    ///
    /// # Errors
    /// `StoreError::Sqlx` / `StoreError::Migrate` khi mở hoặc migrate thất bại.
    pub async fn open(path: &Path) -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    /// DB trong bộ nhớ, dùng cho test.
    ///
    /// # Errors
    /// Như [`Db::open`].
    pub async fn open_memory() -> Result<Self, StoreError> {
        let opts = SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        // max_connections=1: moi connection in-memory la mot DB rieng.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            // DB trong bo nho nam trong connection; neu connection dong thi DB bi xoa.
            // Chien va max_lifetime phai None de dam bao connection khong ket thuc.
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(opts)
            .await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
