//! `0012_node_states.sql`: trạng thái MỨC NODE của một run — MỖI node mỗi run có đúng một dòng
//! `(run_id, node_id)`. Khác `tasks` (chỉ `dola_video` mới có dòng per-scene): ở đây scheduler
//! (`lvc-engine` `dag.rs` `run_steps`) ghi inline cho MỌI kind node, để khôi phục lại trạng thái
//! (đã xong / đang chạy / lỗi + thông điệp lỗi) khi mở lại app. KHÔNG có tương đương Python.
//!
//! Theo đúng khuôn [`crate::RunRepo`] (`workflows.rs`): `upsert` idempotent theo khoá chính, không
//! để chuỗi trạng thái trôi tự do. `u32` ↔ cột `INTEGER` qua `i64` (giống các repo khác).

use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{Db, StoreError};

/// Một dòng `node_states` — trạng thái mức node của một run.
///
/// `Serialize` camelCase: lệnh IPC `run_node_states` trả thẳng kiểu này cho frontend; wire khớp
/// `NodeRunState` của `app/src/store/run.tsx` (`nodeId`/`state`/`done`/`total`/`ok`/`failed`/
/// `error`). `Deserialize` cho test/đọc lại fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStateRow {
    pub run_id: String,
    pub node_id: String,
    pub state: String,
    pub done: u32,
    pub total: u32,
    pub ok: u32,
    pub failed: u32,
    pub error: String,
    pub updated_at: f64,
}

fn row_to_node_state(r: &sqlx::sqlite::SqliteRow) -> Result<NodeStateRow, StoreError> {
    let read_u32 = |name: &str| -> Result<u32, StoreError> {
        let v: i64 = r.try_get(name)?;
        Ok(u32::try_from(v).unwrap_or(0))
    };
    Ok(NodeStateRow {
        run_id: r.try_get("run_id")?,
        node_id: r.try_get("node_id")?,
        state: r.try_get("state")?,
        done: read_u32("done")?,
        total: read_u32("total")?,
        ok: read_u32("ok")?,
        failed: read_u32("failed")?,
        error: r.try_get("error")?,
        updated_at: r.try_get("updated_at")?,
    })
}

#[derive(Debug, Clone)]
pub struct NodeStateRepo {
    db: Db,
}

impl NodeStateRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Tạo mới hoặc đè (idempotent theo `(run_id, node_id)`) — scheduler gọi ở mỗi chuyển trạng
    /// thái node. `counts = (done, total, ok, failed)`. `error = None` ghi thành `''` (cột
    /// `NOT NULL DEFAULT ''`, không dùng `NULL` để khỏi xử lý `Option` ở tầng đọc).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn upsert(
        &self,
        run_id: &str,
        node_id: &str,
        state: &str,
        counts: (u32, u32, u32, u32),
        error: Option<&str>,
        updated_at: f64,
    ) -> Result<(), StoreError> {
        let (done, total, ok, failed) = counts;
        sqlx::query(
            "INSERT INTO node_states
               (run_id, node_id, state, done, total, ok, failed, error, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(run_id, node_id) DO UPDATE SET
               state = excluded.state, done = excluded.done, total = excluded.total,
               ok = excluded.ok, failed = excluded.failed, error = excluded.error,
               updated_at = excluded.updated_at",
        )
        .bind(run_id)
        .bind(node_id)
        .bind(state)
        .bind(i64::from(done))
        .bind(i64::from(total))
        .bind(i64::from(ok))
        .bind(i64::from(failed))
        .bind(error.unwrap_or(""))
        .bind(updated_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Mọi node của một run (thứ tự `node_id` cho ổn định test).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn list_for_run(&self, run_id: &str) -> Result<Vec<NodeStateRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM node_states WHERE run_id = ?1 ORDER BY node_id")
            .bind(run_id)
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_to_node_state).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::NodeStateRepo;
    use crate::Db;

    /// Ghi một dòng rồi đọc lại — đúng `node_id`/`state`/`total`, `error` mặc định rỗng.
    #[tokio::test]
    async fn upsert_roi_doc_lai_mot_dong() {
        let db = Db::open_memory().await.unwrap();
        let repo = NodeStateRepo::new(&db);
        repo.upsert("r1", "im_scene_1", "running", (0, 3, 0, 0), None, 1.0)
            .await
            .unwrap();
        let rows = repo.list_for_run("r1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].node_id, "im_scene_1");
        assert_eq!(rows[0].state, "running");
        assert_eq!(rows[0].total, 3);
        assert_eq!(rows[0].error, "");
    }

    /// Upsert lần hai cùng khoá → đè lên, vẫn một dòng, mang trạng thái mới.
    #[tokio::test]
    async fn upsert_de_len_trang_thai_moi() {
        let db = Db::open_memory().await.unwrap();
        let repo = NodeStateRepo::new(&db);
        repo.upsert("r1", "n1", "running", (0, 3, 0, 0), None, 1.0)
            .await
            .unwrap();
        repo.upsert("r1", "n1", "done", (3, 3, 3, 0), None, 2.0)
            .await
            .unwrap();
        let rows = repo.list_for_run("r1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, "done");
        assert_eq!(rows[0].done, 3);
    }

    /// `error` được ghi khi node lỗi.
    #[tokio::test]
    async fn upsert_ghi_error_khi_failed() {
        let db = Db::open_memory().await.unwrap();
        let repo = NodeStateRepo::new(&db);
        repo.upsert("r1", "v1", "failed", (1, 1, 0, 1), Some("echo ref"), 3.0)
            .await
            .unwrap();
        let rows = repo.list_for_run("r1").await.unwrap();
        assert_eq!(rows[0].error, "echo ref");
        assert_eq!(rows[0].failed, 1);
    }

    /// `list_for_run` chỉ trả node của đúng run.
    #[tokio::test]
    async fn list_for_run_loc_dung_run() {
        let db = Db::open_memory().await.unwrap();
        let repo = NodeStateRepo::new(&db);
        repo.upsert("r1", "a", "done", (1, 1, 1, 0), None, 1.0)
            .await
            .unwrap();
        repo.upsert("r2", "b", "done", (1, 1, 1, 0), None, 1.0)
            .await
            .unwrap();
        let rows = repo.list_for_run("r1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].node_id, "a");
    }

    /// Wire string camelCase (khớp `NodeRunState` của frontend): có `nodeId`/`runId`, không `node_id`.
    #[test]
    fn node_state_row_serde_camelcase() {
        let row = super::NodeStateRow {
            run_id: "r1".into(),
            node_id: "n1".into(),
            state: "done".into(),
            done: 3,
            total: 3,
            ok: 3,
            failed: 0,
            error: String::new(),
            updated_at: 1.0,
        };
        let v = serde_json::to_value(&row).unwrap();
        assert!(v.get("nodeId").is_some());
        assert!(v.get("runId").is_some());
        assert!(v.get("node_id").is_none());
    }
}
