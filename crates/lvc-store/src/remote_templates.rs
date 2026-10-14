//! Cache offline của kho template CỘNG ĐỒNG — bảng `remote_templates_cache` (migration `0007`).
//! Khác bảng `templates` (private, người dùng CRUD): bảng này do APP ghi lại kết quả fetch
//! `index.json` (read-through cache) — xem `crates/providers/lvc-template-registry`.

use sqlx::Row;

use crate::{Db, StoreError};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Một template cộng đồng đã cache (id GỐC trong `index.json`, chưa gắn tiền tố `"remote:"`).
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteTemplateRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub thumbnail: String,
    pub graph: String,
    pub updated_at: f64,
    pub fetched_at: f64,
    /// Phiên bản (`"1.0.1"`); rỗng với mục publish trước khi trường này tồn tại.
    pub version: String,
    /// Tác giả; rỗng khi `index.json` không ghi.
    pub author: String,
    /// JSON mảng `[{"kind":"Thêm","text":"…"}]`; `"[]"` khi tác giả không ghi gì.
    pub changelog: String,
}

fn row_from(r: &sqlx::sqlite::SqliteRow) -> Result<RemoteTemplateRow, StoreError> {
    Ok(RemoteTemplateRow {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        description: r.try_get("description")?,
        category: r.try_get("category")?,
        thumbnail: r.try_get("thumbnail")?,
        graph: r.try_get("graph")?,
        updated_at: r.try_get("updated_at")?,
        fetched_at: r.try_get("fetched_at")?,
        version: r.try_get("version")?,
        author: r.try_get("author")?,
        changelog: r.try_get("changelog")?,
    })
}

/// Một mục để ghi cache — tách khỏi [`RemoteTemplateRow`] vì `fetched_at` do [`RemoteTemplateRepo`]
/// tự gắn (caller không cần biết thời điểm ghi).
#[derive(Debug, Clone, PartialEq)]
pub struct NewRemoteTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub thumbnail: String,
    pub graph: String,
    pub updated_at: f64,
    pub version: String,
    pub author: String,
    pub changelog: String,
}

/// Repo bảng `remote_templates_cache`.
#[derive(Debug, Clone)]
pub struct RemoteTemplateRepo {
    db: Db,
}

impl RemoteTemplateRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Thay TOÀN BỘ cache bằng `items` trong một giao dịch (xoá sạch rồi chèn lại) — đúng ngữ
    /// nghĩa "ảnh chụp mới nhất của `index.json`": mục bị xoá khỏi nguồn cũng biến mất khỏi cache,
    /// không rác tồn đọng mãi.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi lỗi (toàn bộ giao dịch rollback — cache cũ còn nguyên).
    pub async fn replace_all(&self, items: &[NewRemoteTemplate]) -> Result<(), StoreError> {
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("DELETE FROM remote_templates_cache")
            .execute(&mut *tx)
            .await?;
        let fetched_at = now();
        for item in items {
            sqlx::query(
                "INSERT INTO remote_templates_cache \
                 (id, name, description, category, thumbnail, graph, updated_at, fetched_at, \
                  version, author, changelog) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&item.id)
            .bind(&item.name)
            .bind(&item.description)
            .bind(&item.category)
            .bind(&item.thumbnail)
            .bind(&item.graph)
            .bind(item.updated_at)
            .bind(fetched_at)
            .bind(&item.version)
            .bind(&item.author)
            .bind(&item.changelog)
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
    pub async fn list(&self) -> Result<Vec<RemoteTemplateRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM remote_templates_cache ORDER BY name")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_from).collect()
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn lỗi.
    pub async fn get(&self, id: &str) -> Result<Option<RemoteTemplateRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM remote_templates_cache WHERE id = ?")
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

    fn item(id: &str) -> NewRemoteTemplate {
        NewRemoteTemplate {
            id: id.to_string(),
            name: format!("ten {id}"),
            description: "mo ta".to_string(),
            category: "Cơ bản".to_string(),
            thumbnail: String::new(),
            graph: "{}".to_string(),
            updated_at: 123.0,
            version: "1.0.0".to_string(),
            author: "tac gia".to_string(),
            changelog: "[]".to_string(),
        }
    }

    #[tokio::test]
    async fn replace_all_roi_list_doc_lai_dung() {
        let db = memory_db().await;
        let repo = RemoteTemplateRepo::new(&db);
        repo.replace_all(&[item("a"), item("b")]).await.unwrap();
        let rows = repo.list().await.unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r.fetched_at > 0.0));

        let a = repo.get("a").await.unwrap().unwrap();
        assert_eq!(a.name, "ten a");
        assert_eq!(a.version, "1.0.0");
        assert_eq!(a.author, "tac gia");
        assert_eq!(a.changelog, "[]");
    }

    #[tokio::test]
    async fn replace_all_lan_hai_xoa_sach_cache_cu() {
        let db = memory_db().await;
        let repo = RemoteTemplateRepo::new(&db);
        repo.replace_all(&[item("a")]).await.unwrap();
        repo.replace_all(&[item("b")]).await.unwrap();
        let rows = repo.list().await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "b");
    }

    #[tokio::test]
    async fn get_khong_co_tra_none() {
        let db = memory_db().await;
        let repo = RemoteTemplateRepo::new(&db);
        assert!(repo.get("khong-co").await.unwrap().is_none());
    }
}
