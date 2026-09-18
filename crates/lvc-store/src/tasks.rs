use std::path::Path;

use lvc_core::{FailureCode, Mode, Scene, TaskStatus};
use sqlx::Row;

use crate::{Db, StoreError};

/// Tham chiếu batch khi task thuộc một kịch bản nhiều cảnh.
#[derive(Debug, Clone)]
pub struct BatchRef {
    pub batch_id: String,
    pub batch_name: String,
    /// 1-based, giữ thứ tự cảnh.
    pub scene_index: i64,
}

#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: String,
    pub prompt: String,
    pub ratio: String,
    pub duration: i64,
    pub model: String,
    pub mode: String,
    pub logo: String,
    pub status: TaskStatus,
    pub failure_code: Option<FailureCode>,
    pub error: Option<String>,
    pub account: Option<String>,
    pub account_pref: Option<String>,
    pub conversation_id: Option<String>,
    pub video_url: Option<String>,
    pub local_path: Option<String>,
    pub reference_images: Vec<String>,
    pub batch_id: Option<String>,
    pub batch_name: Option<String>,
    pub scene_index: Option<i64>,
    pub retry_count: i64,
    pub retry_at: f64,
    pub created_at: f64,
    pub updated_at: f64,
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Result<TaskRow, StoreError> {
    let refs_json: String = row.try_get("reference_images")?;
    let status_str: String = row.try_get("status")?;
    Ok(TaskRow {
        id: row.try_get("id")?,
        prompt: row.try_get("prompt")?,
        ratio: row.try_get("ratio")?,
        duration: row.try_get("duration")?,
        model: row.try_get("model")?,
        mode: row.try_get("mode")?,
        logo: row.try_get("logo")?,
        // Moi duong ghi (repo + bo nhap du lieu cu) da chuan hoa status roi nen
        // unwrap_or o day chi la phong thu.
        status: TaskStatus::parse(&status_str).unwrap_or(TaskStatus::Failed),
        failure_code: row
            .try_get::<Option<String>, _>("failure_code")?
            .and_then(|s| FailureCode::parse(&s)),
        error: row.try_get("error")?,
        account: row.try_get("account")?,
        account_pref: row.try_get("account_pref")?,
        conversation_id: row.try_get("conversation_id")?,
        video_url: row.try_get("video_url")?,
        local_path: row.try_get("local_path")?,
        reference_images: serde_json::from_str(&refs_json)?,
        batch_id: row.try_get("batch_id")?,
        batch_name: row.try_get("batch_name")?,
        scene_index: row.try_get("scene_index")?,
        retry_count: row.try_get("retry_count")?,
        retry_at: row.try_get("retry_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

#[derive(Debug, Clone)]
pub struct TaskRepo {
    db: Db,
}

impl TaskRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại; `StoreError::Json` khi serialize refs thất bại.
    pub async fn create(
        &self,
        id: &str,
        scene: &Scene,
        batch: Option<&BatchRef>,
    ) -> Result<TaskRow, StoreError> {
        let ts = now();
        let refs = serde_json::to_string(&scene.reference_images)?;
        let account_pref = (!scene.account.is_empty()).then(|| scene.account.clone());
        sqlx::query(
            "INSERT INTO tasks (id, prompt, ratio, duration, model, mode, logo, status,
                 reference_images, account_pref, batch_id, batch_name, scene_index,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'queued', ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        )
        .bind(id)
        .bind(&scene.prompt)
        .bind(scene.ratio.as_str())
        .bind(i64::from(scene.duration))
        .bind(scene.model.as_str())
        .bind(scene.mode.as_str())
        .bind(scene.logo.as_str())
        .bind(&refs)
        .bind(account_pref)
        .bind(batch.map(|b| b.batch_id.clone()))
        .bind(batch.map(|b| b.batch_name.clone()))
        .bind(batch.map(|b| b.scene_index))
        .bind(ts)
        .execute(self.db.pool())
        .await?;

        self.get(id).await?.ok_or_else(|| {
            StoreError::BadPath(format!("task {id} vua tao nhung doc lai khong thay"))
        })
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get(&self, id: &str) -> Result<Option<TaskRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_to_task).transpose()
    }

    /// Huỷ task = `Failed` + `failure_code = "cancelled"` (không có trạng thái `cancelled`).
    ///
    /// Trả về `true` khi có đúng một dòng khớp `id` và được cập nhật, `false`
    /// khi `id` không tồn tại — bộ lập lịch ở phase sau cần biết việc chuyển
    /// trạng thái có thực sự xảy ra hay không.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_status(
        &self,
        id: &str,
        status: TaskStatus,
        failure_code: Option<FailureCode>,
        error: Option<&str>,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE tasks SET status = ?2, failure_code = ?3, error = ?4, updated_at = ?5
             WHERE id = ?1",
        )
        .bind(id)
        .bind(status.as_str())
        .bind(failure_code.map(FailureCode::as_str))
        .bind(error)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Task 17 — server.py:625-627 `on_account` (v0.55t): từ đây trình duyệt mở bằng tài khoản
    /// này. Chỉ ghi `account` + `updated_at`, không đụng status. `true` khi có dòng khớp `id`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_account(&self, id: &str, account: &str) -> Result<bool, StoreError> {
        let result = sqlx::query("UPDATE tasks SET account = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(account)
            .bind(now())
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Task 17 — server.py:612-619 `on_conversation_id`: đã có chat id → lưu để resume/Tiếp tục
    /// (spec §4.4). `true` khi có dòng khớp `id`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_conversation(
        &self,
        id: &str,
        conversation_id: &str,
    ) -> Result<bool, StoreError> {
        let result =
            sqlx::query("UPDATE tasks SET conversation_id = ?2, updated_at = ?3 WHERE id = ?1")
                .bind(id)
                .bind(conversation_id)
                .bind(now())
                .execute(self.db.pool())
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Task 17 — server.py:442-480 `_finish_task` (phần lưu kết quả; hậu kỳ logo/H.264 là P3):
    /// `video_url` + `local_path` (chuỗi `Path::display`). KHÔNG đổi status — engine gọi
    /// [`TaskRepo::set_status`]`(Completed)` riêng. `true` khi có dòng khớp `id`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_result(
        &self,
        id: &str,
        video_url: &str,
        local_path: &Path,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            "UPDATE tasks SET video_url = ?2, local_path = ?3, updated_at = ?4 WHERE id = ?1",
        )
        .bind(id)
        .bind(video_url)
        .bind(local_path.display().to_string())
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Task 17 — server.py:684-685 `store.update(task_id, mode=try_mode, duration=try_duration)`:
    /// hết lượt chế độ này → đổi sang chế độ còn lượt, thời lượng đã nắn theo chế độ mới.
    /// `true` khi có dòng khớp `id`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_mode_duration(
        &self,
        id: &str,
        mode: Mode,
        duration: u16,
    ) -> Result<bool, StoreError> {
        let result =
            sqlx::query("UPDATE tasks SET mode = ?2, duration = ?3, updated_at = ?4 WHERE id = ?1")
                .bind(id)
                .bind(mode.as_str())
                .bind(i64::from(duration))
                .bind(now())
                .execute(self.db.pool())
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<TaskRow>, StoreError> {
        let limit = limit.clamp(1, 200);
        let rows = sqlx::query("SELECT * FROM tasks ORDER BY created_at DESC LIMIT ?1")
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_to_task).collect()
    }
}
