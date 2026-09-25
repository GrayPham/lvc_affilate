//! Kho template RIÊNG của người dùng — bảng `templates` (P4d, migration `0005`). Mẫu Public nhúng
//! sẵn trong binary (không ở đây). `graph` là workflow JSON v2 (chuỗi thô — repo không parse).

use sqlx::Row;

use crate::{Db, StoreError};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Một template riêng.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub graph: String,
    pub thumbnail: String,
    pub updated_at: i64,
}

fn row_from(r: &sqlx::sqlite::SqliteRow) -> Result<TemplateRow, StoreError> {
    Ok(TemplateRow {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        description: r.try_get("description")?,
        graph: r.try_get("graph")?,
        thumbnail: r.try_get("thumbnail")?,
        #[allow(clippy::cast_possible_truncation)]
        updated_at: r.try_get::<f64, _>("updated_at")? as i64,
    })
}

/// Repo bảng `templates`.
#[derive(Debug, Clone)]
pub struct TemplateRepo {
    db: Db,
}

impl TemplateRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Thêm/ghi đè một template riêng.
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn upsert(
        &self,
        id: &str,
        name: &str,
        description: &str,
        graph: &str,
        thumbnail: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO templates (id, name, description, graph, thumbnail, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, description=excluded.description, \
             graph=excluded.graph, thumbnail=excluded.thumbnail, updated_at=excluded.updated_at",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(graph)
        .bind(thumbnail)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Danh sách template riêng, mới nhất trước.
    ///
    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn list(&self) -> Result<Vec<TemplateRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM templates ORDER BY updated_at DESC, name")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_from).collect()
    }

    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn get(&self, id: &str) -> Result<Option<TemplateRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM templates WHERE id = ?")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_from).transpose()
    }

    /// # Errors
    /// `StoreError` khi xoá lỗi.
    pub async fn delete(&self, id: &str) -> Result<bool, StoreError> {
        let r = sqlx::query("DELETE FROM templates WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected() > 0)
    }
}
