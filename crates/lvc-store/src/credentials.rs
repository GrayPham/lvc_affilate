//! Kho tài khoản AI — bảng `credentials` (P4c, migration `0004`). `secret` là ciphertext (lvc-vault
//! mã hoá TRƯỚC khi tới đây); repo này KHÔNG mã hoá/giải mã, chỉ lưu/đọc blob thô. KHÔNG log `secret`.

use sqlx::Row;

use crate::{Db, StoreError};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Một dòng `credentials`. `secret` là ciphertext — Debug KHÔNG in nội dung.
#[derive(Clone)]
pub struct CredentialRow {
    pub id: String,
    pub provider: String,
    pub kind: String,
    pub label: String,
    pub secret: Vec<u8>,
    /// JSON tự do (email, `profile_dir`, `base_url`, model mặc định...).
    pub meta: String,
    pub proxy: String,
    pub status: String,
    pub enabled: bool,
    pub daily_limit: Option<i64>,
    pub cooldown_until: f64,
    pub last_used_at: f64,
    pub last_verified_at: f64,
    pub created_at: f64,
}

impl std::fmt::Debug for CredentialRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialRow")
            .field("id", &self.id)
            .field("provider", &self.provider)
            .field("kind", &self.kind)
            .field("label", &self.label)
            .field("secret", &format_args!("<{} bytes>", self.secret.len()))
            .field("status", &self.status)
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

/// Bản ghi mới cần lưu (secret đã mã hoá).
#[derive(Clone)]
pub struct NewCredential {
    pub id: String,
    pub provider: String,
    pub kind: String,
    pub label: String,
    pub secret: Vec<u8>,
    pub meta: String,
    pub proxy: String,
}

fn row_from(r: &sqlx::sqlite::SqliteRow) -> Result<CredentialRow, StoreError> {
    Ok(CredentialRow {
        id: r.try_get("id")?,
        provider: r.try_get("provider")?,
        kind: r.try_get("kind")?,
        label: r.try_get("label")?,
        secret: r.try_get("secret")?,
        meta: r.try_get("meta")?,
        proxy: r.try_get("proxy")?,
        status: r.try_get("status")?,
        enabled: r.try_get::<i64, _>("enabled")? != 0,
        daily_limit: r.try_get("daily_limit")?,
        cooldown_until: r.try_get("cooldown_until")?,
        last_used_at: r.try_get("last_used_at")?,
        last_verified_at: r.try_get("last_verified_at")?,
        created_at: r.try_get("created_at")?,
    })
}

/// Repo bảng `credentials`.
#[derive(Debug, Clone)]
pub struct CredentialRepo {
    db: Db,
}

impl CredentialRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Thêm một credential (id do caller sinh — uuid).
    ///
    /// # Errors
    /// `StoreError` khi id trùng hoặc ghi lỗi.
    pub async fn insert(&self, c: &NewCredential) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO credentials (id, provider, kind, label, secret, meta, proxy, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&c.id)
        .bind(&c.provider)
        .bind(&c.kind)
        .bind(&c.label)
        .bind(&c.secret)
        .bind(&c.meta)
        .bind(&c.proxy)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Liệt kê (lọc theo provider nếu có), sắp theo `provider, label`.
    ///
    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn list(&self, provider: Option<&str>) -> Result<Vec<CredentialRow>, StoreError> {
        let rows = if let Some(p) = provider {
            sqlx::query("SELECT * FROM credentials WHERE provider = ? ORDER BY label")
                .bind(p)
                .fetch_all(self.db.pool())
                .await?
        } else {
            sqlx::query("SELECT * FROM credentials ORDER BY provider, label")
                .fetch_all(self.db.pool())
                .await?
        };
        rows.iter().map(row_from).collect()
    }

    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn get(&self, id: &str) -> Result<Option<CredentialRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM credentials WHERE id = ?")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_from).transpose()
    }

    /// Đổi bí mật (đã mã hoá) + `meta` cho một credential.
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn update_secret(
        &self,
        id: &str,
        secret: &[u8],
        meta: &str,
    ) -> Result<bool, StoreError> {
        let r = sqlx::query("UPDATE credentials SET secret = ?, meta = ? WHERE id = ?")
            .bind(secret)
            .bind(meta)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected() > 0)
    }

    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn set_label(&self, id: &str, label: &str) -> Result<bool, StoreError> {
        self.set_text(id, "label", label).await
    }

    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn set_meta(&self, id: &str, meta: &str) -> Result<bool, StoreError> {
        self.set_text(id, "meta", meta).await
    }

    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn set_proxy(&self, id: &str, proxy: &str) -> Result<bool, StoreError> {
        self.set_text(id, "proxy", proxy).await
    }

    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn set_enabled(&self, id: &str, on: bool) -> Result<bool, StoreError> {
        let r = sqlx::query("UPDATE credentials SET enabled = ? WHERE id = ?")
            .bind(i64::from(on))
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected() > 0)
    }

    /// Ghi trạng thái kiểm tra (`ok`/`expired`/`banned`/`quota`/`unknown`) + `last_verified_at`.
    ///
    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn set_status(&self, id: &str, status: &str) -> Result<bool, StoreError> {
        let r = sqlx::query("UPDATE credentials SET status = ?, last_verified_at = ? WHERE id = ?")
            .bind(status)
            .bind(now())
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected() > 0)
    }

    /// # Errors
    /// `StoreError` khi ghi lỗi.
    pub async fn touch_used(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE credentials SET last_used_at = ? WHERE id = ?")
            .bind(now())
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// # Errors
    /// `StoreError` khi xoá lỗi.
    pub async fn delete(&self, id: &str) -> Result<bool, StoreError> {
        let r = sqlx::query("DELETE FROM credentials WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected() > 0)
    }

    /// Đã có credential Dola cho `label` này chưa (di trú tránh trùng).
    ///
    /// # Errors
    /// `StoreError` khi truy vấn lỗi.
    pub async fn exists_for(&self, provider: &str, label: &str) -> Result<bool, StoreError> {
        let row = sqlx::query("SELECT 1 FROM credentials WHERE provider = ? AND label = ? LIMIT 1")
            .bind(provider)
            .bind(label)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row.is_some())
    }

    async fn set_text(&self, id: &str, col: &str, val: &str) -> Result<bool, StoreError> {
        // `col` la hang noi bo (khong tu input) — an toan noi vao cau lenh.
        let sql = format!("UPDATE credentials SET {col} = ? WHERE id = ?");
        let r = sqlx::query(&sql)
            .bind(val)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(r.rows_affected() > 0)
    }
}
