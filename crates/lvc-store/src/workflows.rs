//! Task 3 (P3) — `0002_p3.sql`: `workflows` (đồ thị React Flow người dùng vẽ
//! trong app Tauri) và `runs` (một lần chạy workflow). Khái niệm mới của P3,
//! KHÔNG có tương đương trong bản Python (bản cũ chỉ có task đơn, không có
//! workflow) — DAG executor (Task 7), resume (Task 6) và Tauri commands
//! (Task 9) dùng hai repo này.

use sqlx::Row;

use crate::{Db, StoreError};

/// Một dòng `workflows` đầy đủ, gồm cả `json` (đồ thị React Flow nguyên văn —
/// store không hiểu/không chuẩn hoá cấu trúc node/cạnh).
#[derive(Debug, Clone)]
pub struct WorkflowRow {
    pub id: String,
    pub name: String,
    pub json: String,
    pub updated_at: f64,
}

/// Siêu dữ liệu workflow KHÔNG kèm `json` — dùng cho danh sách (Tauri command
/// liệt kê workflow không cần tải cả đồ thị).
#[derive(Debug, Clone)]
pub struct WorkflowMeta {
    pub id: String,
    pub name: String,
    pub updated_at: f64,
}

#[derive(Debug, Clone)]
pub struct WorkflowRepo {
    db: Db,
}

impl WorkflowRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Tạo mới hoặc ghi đè (đè cả `name`/`json`/`updated_at`) — Tauri command
    /// "lưu workflow" gọi hàm này cho cả lần đầu lẫn các lần sửa sau.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn upsert(
        &self,
        id: &str,
        name: &str,
        json: &str,
        updated_at: f64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO workflows (id, name, json, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name, json = excluded.json, updated_at = excluded.updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(json)
        .bind(updated_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get(&self, id: &str) -> Result<Option<WorkflowRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM workflows WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.map(|r| {
            Ok(WorkflowRow {
                id: r.try_get("id")?,
                name: r.try_get("name")?,
                json: r.try_get("json")?,
                updated_at: r.try_get("updated_at")?,
            })
        })
        .transpose()
    }

    /// Danh sách workflow, mới sửa gần đây nhất trước — không kèm `json` (dùng
    /// cho màn hình chọn workflow; [`WorkflowRepo::get`] tải đồ thị đầy đủ khi
    /// người dùng mở một workflow cụ thể).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn list_meta(&self) -> Result<Vec<WorkflowMeta>, StoreError> {
        let rows =
            sqlx::query("SELECT id, name, updated_at FROM workflows ORDER BY updated_at DESC")
                .fetch_all(self.db.pool())
                .await?;
        rows.iter()
            .map(|r| {
                Ok(WorkflowMeta {
                    id: r.try_get("id")?,
                    name: r.try_get("name")?,
                    updated_at: r.try_get("updated_at")?,
                })
            })
            .collect()
    }

    /// `true` khi có đúng một dòng khớp `id` bị xoá, `false` khi `id` không tồn
    /// tại.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn delete(&self, id: &str) -> Result<bool, StoreError> {
        let result = sqlx::query("DELETE FROM workflows WHERE id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Một dòng `runs` — một lần chạy workflow (DAG executor Task 7 cập nhật khi
/// chạy; resume Task 6 đọc lại sau khi khởi động lại app).
#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub started_at: f64,
    pub finished_at: f64,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct RunRepo {
    db: Db,
}

impl RunRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Bắt đầu một lần chạy mới — `status = "running"`, `finished_at = 0`,
    /// `error = ""`. DAG executor (Task 7) gọi khi bắt đầu thực thi workflow.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn create(
        &self,
        id: &str,
        workflow_id: &str,
        started_at: f64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO runs (id, workflow_id, status, started_at, finished_at, error)
             VALUES (?1, ?2, 'running', ?3, 0, '')",
        )
        .bind(id)
        .bind(workflow_id)
        .bind(started_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// `error = None` ghi thành `''` (cột `NOT NULL DEFAULT ''` — không dùng
    /// `NULL` để khỏi phải xử lý `Option` ở tầng đọc). `true` khi có dòng khớp
    /// `id`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_status(
        &self,
        id: &str,
        status: &str,
        error: Option<&str>,
        finished_at: f64,
    ) -> Result<bool, StoreError> {
        let result =
            sqlx::query("UPDATE runs SET status = ?2, error = ?3, finished_at = ?4 WHERE id = ?1")
                .bind(id)
                .bind(status)
                .bind(error.unwrap_or(""))
                .bind(finished_at)
                .execute(self.db.pool())
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get(&self, id: &str) -> Result<Option<RunRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM runs WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.map(|r| {
            Ok(RunRow {
                id: r.try_get("id")?,
                workflow_id: r.try_get("workflow_id")?,
                status: r.try_get("status")?,
                started_at: r.try_get("started_at")?,
                finished_at: r.try_get("finished_at")?,
                error: r.try_get("error")?,
            })
        })
        .transpose()
    }

    /// `limit` được kẹp vào [1, 200] như [`crate::TaskRepo::list_recent`].
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<RunRow>, StoreError> {
        let limit = limit.clamp(1, 200);
        let rows = sqlx::query("SELECT * FROM runs ORDER BY started_at DESC LIMIT ?1")
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?;
        rows.iter()
            .map(|r| {
                Ok(RunRow {
                    id: r.try_get("id")?,
                    workflow_id: r.try_get("workflow_id")?,
                    status: r.try_get("status")?,
                    started_at: r.try_get("started_at")?,
                    finished_at: r.try_get("finished_at")?,
                    error: r.try_get("error")?,
                })
            })
            .collect()
    }
}
