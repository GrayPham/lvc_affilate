//! Cache offline của kho kịch bản CỘNG ĐỒNG — bảng `remote_scripts_cache` (migration `0008`).
//! Khác bảng `scripts` (private, người dùng CRUD scenes/task): bảng này do APP ghi lại kết quả
//! fetch `index.json` (read-through cache, CÙNG nguồn với `remote_templates_cache` — xem
//! `crates/providers/lvc-template-registry`), chỉ giữ BẢN NHÁP (tên + danh sách dòng prompt thô).

use sqlx::Row;

use crate::{Db, StoreError};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Một kịch bản cộng đồng đã cache (id GỐC trong `index.json`, chưa gắn tiền tố `"remote:"`).
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteScriptRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// JSON (`Vec<String>`) — mỗi phần tử một cảnh.
    pub scenes: String,
    pub updated_at: f64,
    pub fetched_at: f64,
}

fn row_from(r: &sqlx::sqlite::SqliteRow) -> Result<RemoteScriptRow, StoreError> {
    Ok(RemoteScriptRow {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        description: r.try_get("description")?,
        category: r.try_get("category")?,
        scenes: r.try_get("scenes")?,
        updated_at: r.try_get("updated_at")?,
        fetched_at: r.try_get("fetched_at")?,
    })
}

/// Một mục để ghi cache — tách khỏi [`RemoteScriptRow`] vì `fetched_at` do [`RemoteScriptRepo`] tự
/// gắn (caller không cần biết thời điểm ghi).
#[derive(Debug, Clone, PartialEq)]
pub struct NewRemoteScript {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub scenes: String,
    pub updated_at: f64,
}

/// Repo bảng `remote_scripts_cache`.
#[derive(Debug, Clone)]
pub struct RemoteScriptRepo {
    db: Db,
}

impl RemoteScriptRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Thay TOÀN BỘ cache bằng `items` trong một giao dịch — cùng ngữ nghĩa
    /// [`crate::RemoteTemplateRepo::replace_all`].
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi lỗi (toàn bộ giao dịch rollback — cache cũ còn nguyên).
    pub async fn replace_all(&self, items: &[NewRemoteScript]) -> Result<(), StoreError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DELETE FROM remote_scripts_cache")
            .execute(&mut *tx)
            .await?;
        let fetched_at = now();
        for item in items {
            sqlx::query(
                "INSERT INTO remote_scripts_cache \
                 (id, name, description, category, scenes, updated_at, fetched_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&item.id)
            .bind(&item.name)
            .bind(&item.description)
            .bind(&item.category)
            .bind(&item.scenes)
            .bind(item.updated_at)
            .bind(fetched_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Danh sách cache hiện có, theo tên.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn lỗi.
    pub async fn list(&self) -> Result<Vec<RemoteScriptRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM remote_scripts_cache ORDER BY name")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_from).collect()
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn lỗi.
    pub async fn get(&self, id: &str) -> Result<Option<RemoteScriptRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM remote_scripts_cache WHERE id = ?")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_from).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    async fn memory_db() -> Db {
        Db::open_memory().await.unwrap()
    }

    fn item(id: &str) -> NewRemoteScript {
        NewRemoteScript {
            id: id.to_string(),
            name: format!("ten {id}"),
            description: "mo ta".to_string(),
            category: "Affiliate".to_string(),
            scenes: "[\"canh 1\"]".to_string(),
            updated_at: 123.0,
        }
    }

    #[tokio::test]
    async fn replace_all_roi_list_doc_lai_dung() {
        let db = memory_db().await;
        let repo = RemoteScriptRepo::new(&db);
        repo.replace_all(&[item("a"), item("b")]).await.unwrap();
        let rows = repo.list().await.unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.fetched_at > 0.0));

        let a = repo.get("a").await.unwrap().unwrap();
        assert_eq!(a.name, "ten a");
    }

    #[tokio::test]
    async fn replace_all_lan_hai_xoa_sach_cache_cu() {
        let db = memory_db().await;
        let repo = RemoteScriptRepo::new(&db);
        repo.replace_all(&[item("a")]).await.unwrap();
        repo.replace_all(&[item("b")]).await.unwrap();
        let rows = repo.list().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "b");
    }

    #[tokio::test]
    async fn get_khong_co_tra_none() {
        let db = memory_db().await;
        let repo = RemoteScriptRepo::new(&db);
        assert!(repo.get("khong-co").await.unwrap().is_none());
    }
}
