//! Hàng chờ workflow — bảng `queue_items` (migration `0009`). Màn "Tác vụ".
//!
//! Khác bảng `runs` (một lần chạy ĐÃ bắt đầu): đây là DANH SÁCH CHỜ do người dùng xếp, engine lấy
//! từng mục theo `position` tăng dần rồi chạy tuần tự. `graph` là BẢN CHỤP lúc thêm — sửa workflow
//! gốc sau đó không làm đổi mục đang chờ.

use sqlx::Row;

use crate::{Db, StoreError};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Trạng thái một mục trong hàng chờ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Waiting,
    Running,
    Done,
    Failed,
    /// Bị bỏ qua (thiếu tài khoản, hoặc lỗi + chính sách "Bỏ qua, chạy tiếp").
    Skipped,
}

impl QueueStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    /// Chuỗi lạ → `Waiting` (dữ liệu cũ/hỏng không làm sập danh sách). Tên `parse` theo đúng quy
    /// ước của crate này (xem `RunStatus::parse`), KHÔNG đặt `from_str` để khỏi lẫn với trait
    /// `std::str::FromStr`.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "running" => Self::Running,
            "done" => Self::Done,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => Self::Waiting,
        }
    }

    /// Đã chạy xong (không còn chờ tới lượt nữa).
    #[must_use]
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Skipped)
    }
}

/// Một mục trong hàng chờ.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueRow {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    /// Workflow JSON (bản chụp lúc thêm).
    pub graph: String,
    pub position: i64,
    pub status: QueueStatus,
    pub run_id: Option<String>,
    pub error: String,
    pub created_at: f64,
    pub started_at: Option<f64>,
    pub finished_at: Option<f64>,
}

fn row_from(r: &sqlx::sqlite::SqliteRow) -> Result<QueueRow, StoreError> {
    Ok(QueueRow {
        id: r.try_get("id")?,
        workflow_id: r.try_get("workflow_id")?,
        name: r.try_get("name")?,
        graph: r.try_get("graph")?,
        position: r.try_get("position")?,
        status: QueueStatus::parse(&r.try_get::<String, _>("status")?),
        run_id: r.try_get("run_id")?,
        error: r.try_get("error")?,
        created_at: r.try_get("created_at")?,
        started_at: r.try_get("started_at")?,
        finished_at: r.try_get("finished_at")?,
    })
}

/// Mục mới để thêm vào cuối hàng chờ.
#[derive(Debug, Clone, PartialEq)]
pub struct NewQueueItem {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub graph: String,
}

/// Repo bảng `queue_items`.
#[derive(Debug, Clone)]
pub struct QueueRepo {
    db: Db,
}

impl QueueRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Thêm vào CUỐI hàng chờ (`position` = max hiện có + 1).
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn push(&self, item: &NewQueueItem) -> Result<QueueRow, StoreError> {
        let next: i64 = sqlx::query("SELECT COALESCE(MAX(position), 0) + 1 AS p FROM queue_items")
            .fetch_one(self.db.pool())
            .await?
            .try_get("p")?;
        sqlx::query(
            "INSERT INTO queue_items (id, workflow_id, name, graph, position, status, created_at) \
             VALUES (?, ?, ?, ?, ?, 'waiting', ?)",
        )
        .bind(&item.id)
        .bind(&item.workflow_id)
        .bind(&item.name)
        .bind(&item.graph)
        .bind(next)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        // `get` ngay sau `INSERT` thanh cong — `RowNotFound` o day la bat thuong that su.
        self.get(&item.id)
            .await?
            .ok_or(StoreError::Sqlx(sqlx::Error::RowNotFound))
    }

    /// Toàn bộ hàng chờ theo thứ tự chạy.
    ///
    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn list(&self) -> Result<Vec<QueueRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM queue_items ORDER BY position, created_at")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_from).collect()
    }

    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn get(&self, id: &str) -> Result<Option<QueueRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM queue_items WHERE id = ?")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_from).transpose()
    }

    /// Mục KẾ TIẾP cần chạy — `waiting` có `position` nhỏ nhất. `None` = hết việc.
    ///
    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn next_waiting(&self) -> Result<Option<QueueRow>, StoreError> {
        let row = sqlx::query(
            "SELECT * FROM queue_items WHERE status = 'waiting' ORDER BY position, created_at LIMIT 1",
        )
        .fetch_optional(self.db.pool())
        .await?;
        row.as_ref().map(row_from).transpose()
    }

    /// Đánh dấu bắt đầu chạy (gắn `run_id`).
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn mark_running(&self, id: &str, run_id: &str) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE queue_items SET status = 'running', run_id = ?, started_at = ?, error = '' \
             WHERE id = ?",
        )
        .bind(run_id)
        .bind(now())
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Kết thúc một mục (`done`/`failed`/`skipped`) + lý do (rỗng khi thành công).
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn finish(
        &self,
        id: &str,
        status: QueueStatus,
        error: &str,
    ) -> Result<(), StoreError> {
        sqlx::query("UPDATE queue_items SET status = ?, error = ?, finished_at = ? WHERE id = ?")
            .bind(status.as_str())
            .bind(error)
            .bind(now())
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Xoá một mục khỏi hàng chờ. `false` = không có gì để xoá.
    ///
    /// # Errors
    /// `StoreError` khi xoá lỗi.
    pub async fn remove(&self, id: &str) -> Result<bool, StoreError> {
        let r = sqlx::query("DELETE FROM queue_items WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected() > 0)
    }

    /// Xoá mọi mục ĐÃ kết thúc (done/failed/skipped) — nút "dọn hàng chờ". Trả số dòng đã xoá.
    ///
    /// # Errors
    /// `StoreError` khi xoá lỗi.
    pub async fn clear_finished(&self) -> Result<u64, StoreError> {
        let r = sqlx::query("DELETE FROM queue_items WHERE status IN ('done','failed','skipped')")
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected())
    }

    /// Sắp lại TOÀN BỘ thứ tự theo danh sách id truyền vào (kéo thả). Id không có trong bảng bị bỏ
    /// qua; mục không được nhắc tới giữ nguyên `position` cũ nên luôn nằm sau.
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi (cả giao dịch rollback).
    pub async fn reorder(&self, ids: &[String]) -> Result<(), StoreError> {
        let mut tx = self.db.pool().begin().await?;
        for (i, id) in ids.iter().enumerate() {
            let pos = i64::try_from(i).unwrap_or(i64::MAX);
            sqlx::query("UPDATE queue_items SET position = ? WHERE id = ?")
                .bind(pos)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Đưa mọi mục `running` về `waiting` — gọi lúc KHỞI ĐỘNG app: app tắt giữa chừng thì mục đang
    /// chạy không bao giờ tự kết thúc, để nguyên sẽ kẹt hàng chờ mãi. Trả số dòng đã sửa.
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn requeue_running(&self) -> Result<u64, StoreError> {
        let r = sqlx::query(
            "UPDATE queue_items SET status = 'waiting', run_id = NULL, started_at = NULL \
             WHERE status = 'running'",
        )
        .execute(self.db.pool())
        .await?;
        Ok(r.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    async fn repo() -> QueueRepo {
        QueueRepo::new(&Db::open_memory().await.unwrap())
    }

    fn item(id: &str, name: &str) -> NewQueueItem {
        NewQueueItem {
            id: id.to_string(),
            workflow_id: format!("wf-{id}"),
            name: name.to_string(),
            graph: r#"{"version":2,"nodes":[],"edges":[]}"#.to_string(),
        }
    }

    #[tokio::test]
    async fn push_xep_cuoi_hang_theo_thu_tu_them() {
        let r = repo().await;
        r.push(&item("a", "Alpha")).await.unwrap();
        r.push(&item("b", "Beta")).await.unwrap();
        r.push(&item("c", "Gamma")).await.unwrap();

        let rows = r.list().await.unwrap();
        assert_eq!(
            rows.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert!(rows.iter().all(|x| x.status == QueueStatus::Waiting));
    }

    #[tokio::test]
    async fn next_waiting_lay_muc_dau_va_bo_qua_muc_da_xong() {
        let r = repo().await;
        r.push(&item("a", "Alpha")).await.unwrap();
        r.push(&item("b", "Beta")).await.unwrap();

        assert_eq!(r.next_waiting().await.unwrap().unwrap().id, "a");

        r.mark_running("a", "run-1").await.unwrap();
        // Dang chay thi KHONG duoc lay lai -> muc ke tiep la "b".
        assert_eq!(r.next_waiting().await.unwrap().unwrap().id, "b");

        r.finish("a", QueueStatus::Done, "").await.unwrap();
        r.finish("b", QueueStatus::Done, "").await.unwrap();
        assert!(r.next_waiting().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn mark_running_roi_finish_ghi_dung_trang_thai_va_moc_thoi_gian() {
        let r = repo().await;
        r.push(&item("a", "Alpha")).await.unwrap();

        r.mark_running("a", "run-9").await.unwrap();
        let row = r.get("a").await.unwrap().unwrap();
        assert_eq!(row.status, QueueStatus::Running);
        assert_eq!(row.run_id.as_deref(), Some("run-9"));
        assert!(row.started_at.unwrap() > 0.0);

        r.finish("a", QueueStatus::Failed, "thiếu tài khoản Video")
            .await
            .unwrap();
        let row = r.get("a").await.unwrap().unwrap();
        assert_eq!(row.status, QueueStatus::Failed);
        assert_eq!(row.error, "thiếu tài khoản Video");
        assert!(row.finished_at.unwrap() > 0.0);
        // `run_id` giu lai de con mo duoc log lan chay do.
        assert_eq!(row.run_id.as_deref(), Some("run-9"));
    }

    #[tokio::test]
    async fn reorder_doi_dung_thu_tu_chay() {
        let r = repo().await;
        r.push(&item("a", "Alpha")).await.unwrap();
        r.push(&item("b", "Beta")).await.unwrap();
        r.push(&item("c", "Gamma")).await.unwrap();

        r.reorder(&["c".to_string(), "a".to_string(), "b".to_string()])
            .await
            .unwrap();

        let rows = r.list().await.unwrap();
        assert_eq!(
            rows.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
        assert_eq!(r.next_waiting().await.unwrap().unwrap().id, "c");
    }

    #[tokio::test]
    async fn remove_va_clear_finished() {
        let r = repo().await;
        r.push(&item("a", "Alpha")).await.unwrap();
        r.push(&item("b", "Beta")).await.unwrap();
        r.push(&item("c", "Gamma")).await.unwrap();

        assert!(r.remove("b").await.unwrap());
        assert!(!r.remove("b").await.unwrap(), "xoa lan hai la idempotent");

        r.finish("a", QueueStatus::Done, "").await.unwrap();
        assert_eq!(r.clear_finished().await.unwrap(), 1);

        let rows = r.list().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "c");
    }

    #[tokio::test]
    async fn requeue_running_khi_mo_lai_app() {
        let r = repo().await;
        r.push(&item("a", "Alpha")).await.unwrap();
        r.mark_running("a", "run-1").await.unwrap();

        // Gia lap app tat giua chung roi mo lai.
        assert_eq!(r.requeue_running().await.unwrap(), 1);

        let row = r.get("a").await.unwrap().unwrap();
        assert_eq!(row.status, QueueStatus::Waiting);
        assert_eq!(row.run_id, None);
        assert_eq!(row.started_at, None);
        // Lay lai duoc de chay tiep.
        assert_eq!(r.next_waiting().await.unwrap().unwrap().id, "a");
    }

    #[tokio::test]
    async fn graph_la_ban_chup_khong_doi_theo_workflow_goc() {
        let r = repo().await;
        let mut it = item("a", "Alpha");
        it.graph = r#"{"version":2,"nodes":[{"id":"n1"}],"edges":[]}"#.to_string();
        r.push(&it).await.unwrap();

        let row = r.get("a").await.unwrap().unwrap();
        assert!(row.graph.contains("n1"), "giu nguyen do thi luc them");
        assert_eq!(row.workflow_id, "wf-a");
    }
}
