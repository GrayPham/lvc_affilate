use lvc_core::Mode;
use sqlx::Row;

use crate::{Db, StoreError};

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub name: String,
    pub scheduling: bool,
    pub email: String,
    pub note: String,
    pub proxy: String,
    pub browser: String,
    pub login_ok: bool,
    pub last_used_at: f64,
    pub cooldown_until: f64,
    pub rate_limited_until: f64,
    pub quota_blocked_until: f64,
    pub cookie_expires_at: f64,
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[derive(Debug, Clone)]
pub struct AccountRepo {
    db: Db,
}

impl AccountRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// Tạo nếu chưa có; đã có thì không đụng gì.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn upsert(&self, name: &str) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO accounts_meta (name, created_at) VALUES (?1, ?2)
             ON CONFLICT(name) DO NOTHING",
        )
        .bind(name)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn list(&self) -> Result<Vec<AccountRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM accounts_meta ORDER BY name")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter()
            .map(|r| {
                Ok(AccountRow {
                    name: r.try_get("name")?,
                    scheduling: r.try_get::<i64, _>("scheduling")? != 0,
                    email: r.try_get("email")?,
                    note: r.try_get("note")?,
                    proxy: r.try_get("proxy")?,
                    browser: r.try_get("browser")?,
                    login_ok: r.try_get::<i64, _>("login_ok")? != 0,
                    last_used_at: r.try_get("last_used_at")?,
                    cooldown_until: r.try_get("cooldown_until")?,
                    rate_limited_until: r.try_get("rate_limited_until")?,
                    quota_blocked_until: r.try_get("quota_blocked_until")?,
                    cookie_expires_at: r.try_get("cookie_expires_at")?,
                })
            })
            .collect()
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_scheduling(&self, name: &str, on: bool) -> Result<(), StoreError> {
        sqlx::query("UPDATE accounts_meta SET scheduling = ?2 WHERE name = ?1")
            .bind(name)
            .bind(i64::from(on))
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_proxy(&self, name: &str, proxy: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE accounts_meta SET proxy = ?2 WHERE name = ?1")
            .bind(name)
            .bind(proxy)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Tăng và trả về số lượt đã dùng trong ngày của chế độ đó.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn bump_usage(
        &self,
        account: &str,
        day: &str,
        mode: Mode,
    ) -> Result<i64, StoreError> {
        let row = sqlx::query(
            "INSERT INTO usage (account, day, mode, used) VALUES (?1, ?2, ?3, 1)
             ON CONFLICT(account, day, mode) DO UPDATE SET used = used + 1
             RETURNING used",
        )
        .bind(account)
        .bind(day)
        .bind(mode.as_str())
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.try_get("used")?)
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn used_today(
        &self,
        account: &str,
        day: &str,
        mode: Mode,
    ) -> Result<i64, StoreError> {
        let row =
            sqlx::query("SELECT used FROM usage WHERE account = ?1 AND day = ?2 AND mode = ?3")
                .bind(account)
                .bind(day)
                .bind(mode.as_str())
                .fetch_optional(self.db.pool())
                .await?;
        match row {
            Some(r) => Ok(r.try_get("used")?),
            None => Ok(0),
        }
    }
}
