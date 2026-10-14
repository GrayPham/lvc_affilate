//! Bảng `workflow_origin` (migration `0010`) — workflow nào đến từ mục nào của kho cộng đồng, ở
//! phiên bản nào, và đồ thị lúc tải về ra sao.
//!
//! Tách file riêng (không nhập vào `workflows.rs` vốn đã giữ `WorkflowRepo` + `RunRepo`): đây là
//! một trách nhiệm độc lập — bỏ tính năng "Cập nhật workflow" chỉ cần bỏ file này, không chạm vào
//! đường lưu/chạy workflow đang hoạt động.

use sqlx::Row;

use crate::{Db, StoreError};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Một dòng nguồn gốc.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowOrigin {
    pub workflow_id: String,
    pub source_id: String,
    pub source_version: String,
    /// Đồ thị (JSON) NGAY LÚC tải về — so với bản hiện tại để biết đã sửa trên máy chưa.
    pub source_json: String,
    pub installed_at: f64,
}

fn row_from(r: &sqlx::sqlite::SqliteRow) -> Result<WorkflowOrigin, StoreError> {
    Ok(WorkflowOrigin {
        workflow_id: r.try_get("workflow_id")?,
        source_id: r.try_get("source_id")?,
        source_version: r.try_get("source_version")?,
        source_json: r.try_get("source_json")?,
        installed_at: r.try_get("installed_at")?,
    })
}

/// Repo bảng `workflow_origin`.
#[derive(Debug, Clone)]
pub struct WorkflowOriginRepo {
    db: Db,
}

impl WorkflowOriginRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Ghi (hoặc ghi lại) nguồn gốc của một workflow. Gọi khi người dùng dùng một mẫu cộng đồng và
    /// khi áp bản cập nhật.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi lỗi.
    pub async fn set(
        &self,
        workflow_id: &str,
        source_id: &str,
        source_version: &str,
        source_json: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO workflow_origin (workflow_id, source_id, source_version, source_json, installed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(workflow_id) DO UPDATE SET
               source_id = excluded.source_id,
               source_version = excluded.source_version,
               source_json = excluded.source_json,
               installed_at = excluded.installed_at",
        )
        .bind(workflow_id)
        .bind(source_id)
        .bind(source_version)
        .bind(source_json)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// `None` = workflow do người dùng tự dựng (không có nguồn gốc) — trạng thái BÌNH THƯỜNG.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi đọc lỗi.
    pub async fn get(&self, workflow_id: &str) -> Result<Option<WorkflowOrigin>, StoreError> {
        let row = sqlx::query("SELECT * FROM workflow_origin WHERE workflow_id = ?1")
            .bind(workflow_id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_from).transpose()
    }

    /// Mọi workflow có nguồn gốc. Dùng để đối chiếu với cache kho cộng đồng tìm bản mới.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi đọc lỗi.
    pub async fn list(&self) -> Result<Vec<WorkflowOrigin>, StoreError> {
        let rows = sqlx::query("SELECT * FROM workflow_origin ORDER BY installed_at DESC")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_from).collect()
    }

    /// Xoá nguồn gốc của một workflow — gọi khi xoá chính workflow đó, để không còn dòng mồ côi
    /// làm "Cập nhật workflow" báo bản mới cho một workflow đã biến mất.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi lỗi.
    pub async fn remove(&self, workflow_id: &str) -> Result<bool, StoreError> {
        let res = sqlx::query("DELETE FROM workflow_origin WHERE workflow_id = ?1")
            .bind(workflow_id)
            .execute(self.db.pool())
            .await?;
        Ok(res.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db() -> (tempfile::TempDir, Db) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("t.db")).await.unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn ghi_roi_doc_lai_dung_gia_tri() {
        let (_d, db) = db().await;
        let repo = WorkflowOriginRepo::new(&db);
        repo.set("wf1", "review-mon-an", "1.0.0", "{\"nodes\":[]}")
            .await
            .unwrap();

        let got = repo.get("wf1").await.unwrap().unwrap();
        assert_eq!(got.source_id, "review-mon-an");
        assert_eq!(got.source_version, "1.0.0");
        assert_eq!(got.source_json, "{\"nodes\":[]}");
        assert!(got.installed_at > 0.0);
    }

    #[tokio::test]
    async fn khong_co_nguon_goc_tra_none_khong_phai_loi() {
        let (_d, db) = db().await;
        assert!(
            WorkflowOriginRepo::new(&db)
                .get("tu-dung")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn set_lan_hai_ghi_de_khong_tao_dong_thu_hai() {
        let (_d, db) = db().await;
        let repo = WorkflowOriginRepo::new(&db);
        repo.set("wf1", "a", "1.0.0", "cu").await.unwrap();
        repo.set("wf1", "a", "1.0.1", "moi").await.unwrap();

        assert_eq!(repo.list().await.unwrap().len(), 1);
        let got = repo.get("wf1").await.unwrap().unwrap();
        assert_eq!(got.source_version, "1.0.1");
        assert_eq!(got.source_json, "moi");
    }

    #[tokio::test]
    async fn remove_xoa_dung_mot_dong() {
        let (_d, db) = db().await;
        let repo = WorkflowOriginRepo::new(&db);
        repo.set("wf1", "a", "1.0.0", "{}").await.unwrap();
        repo.set("wf2", "b", "1.0.0", "{}").await.unwrap();

        assert!(repo.remove("wf1").await.unwrap());
        assert!(
            !repo.remove("wf1").await.unwrap(),
            "xoa lan hai = khong co gi"
        );
        let left = repo.list().await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].workflow_id, "wf2");
    }
}
