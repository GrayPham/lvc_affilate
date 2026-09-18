use std::collections::HashMap;

use sqlx::Row;

use crate::{Db, StoreError};

#[derive(Debug, Clone)]
pub struct SettingsRepo {
    db: Db,
}

impl SettingsRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(self.db.pool())
            .await?;
        match row {
            Some(r) => Ok(Some(r.try_get("value")?)),
            None => Ok(None),
        }
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get_or(&self, key: &str, default: &str) -> Result<String, StoreError> {
        Ok(self.get(key).await?.unwrap_or_else(|| default.to_string()))
    }

    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// Task 3 (P3): toàn bộ bảng `settings` thành map — `AppSettings::from_map`
    /// (Task 5, lvc-core) đọc từ đây rồi tự áp mặc định/parse; repo này chỉ biết
    /// key/value thô.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get_all(&self) -> Result<HashMap<String, String>, StoreError> {
        let rows = sqlx::query("SELECT key, value FROM settings")
            .fetch_all(self.db.pool())
            .await?;
        rows.iter()
            .map(|r| Ok((r.try_get("key")?, r.try_get("value")?)))
            .collect()
    }

    /// Task 3 (P3): ghi nhiều cặp trong một giao dịch (Tauri command lưu cả form
    /// cài đặt một lần thay vì gọi [`SettingsRepo::set`] lặp — tránh trạng thái nửa
    /// vời nếu giữa chừng lỗi).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại (toàn bộ giao dịch bị rollback).
    pub async fn set_many(&self, pairs: &[(&str, &str)]) -> Result<(), StoreError> {
        let mut tx = self.db.pool().begin().await?;
        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            )
            .bind(*key)
            .bind(*value)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}
