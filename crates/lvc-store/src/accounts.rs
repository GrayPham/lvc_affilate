use lvc_core::Mode;
use sqlx::Row;

use crate::{Db, StoreError};

/// Một dòng `accounts_meta` (đủ mọi cột của schema `0001_init.sql`, khớp bảng Python
/// `browser_pool.py:196-244`). Mốc thời gian là unix giây (`REAL`), `0` = chưa đặt.
#[derive(Debug, Clone)]
pub struct AccountRow {
    pub name: String,
    pub scheduling: bool,
    pub email: String,
    pub note: String,
    pub proxy: String,
    pub browser: String,
    pub created_at: f64,
    /// `login_ok != 0` (Python: `None` = chưa verify vẫn cho lịch → schema mặc định 1).
    pub login_ok: bool,
    pub login_checked_at: f64,
    pub last_used_at: f64,
    pub cooldown_until: f64,
    pub rate_limited_until: f64,
    pub limit_reason: String,
    pub quota_blocked_until: f64,
    pub quota_reason: String,
    /// `NULL` = chưa đọc số dư credit lần nào (`_credit_blocks` không chặn).
    pub credit_balance: Option<i64>,
    pub credit_checked_at: f64,
    pub logged_out_at: f64,
    pub cookie_expires_at: f64,
    /// `''` | `checking` | `ok` | `manual` | `revoked` (`browser_pool.py:600`, `:663`).
    pub auto_verify_state: String,
    pub auto_verify_tries: i64,
}

/// Ánh xạ một dòng `SELECT * FROM accounts_meta` sang [`AccountRow`] (dùng chung cho `list`/`get`).
fn row_from(r: &sqlx::sqlite::SqliteRow) -> Result<AccountRow, StoreError> {
    Ok(AccountRow {
        name: r.try_get("name")?,
        scheduling: r.try_get::<i64, _>("scheduling")? != 0,
        email: r.try_get("email")?,
        note: r.try_get("note")?,
        proxy: r.try_get("proxy")?,
        browser: r.try_get("browser")?,
        created_at: r.try_get("created_at")?,
        login_ok: r.try_get::<i64, _>("login_ok")? != 0,
        login_checked_at: r.try_get("login_checked_at")?,
        last_used_at: r.try_get("last_used_at")?,
        cooldown_until: r.try_get("cooldown_until")?,
        rate_limited_until: r.try_get("rate_limited_until")?,
        limit_reason: r.try_get("limit_reason")?,
        quota_blocked_until: r.try_get("quota_blocked_until")?,
        quota_reason: r.try_get("quota_reason")?,
        credit_balance: r.try_get("credit_balance")?,
        credit_checked_at: r.try_get("credit_checked_at")?,
        logged_out_at: r.try_get("logged_out_at")?,
        cookie_expires_at: r.try_get("cookie_expires_at")?,
        auto_verify_state: r.try_get("auto_verify_state")?,
        auto_verify_tries: r.try_get("auto_verify_tries")?,
    })
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
        rows.iter().map(row_from).collect()
    }

    /// Một tài khoản theo tên (browser_pool.py:413-415 `_meta`); `None` khi chưa có dòng.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy vấn thất bại.
    pub async fn get(&self, name: &str) -> Result<Option<AccountRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM accounts_meta WHERE name = ?1")
            .bind(name)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_from).transpose()
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

    /// Cột `browser` (Task 4/HOP-DONG v1.5: `chromium`/`chrome`/`opera`, `''` = chromium) — engine
    /// đọc qua `BrowserKind::parse`; giá trị KHÔNG được kiểm ở đây (UI P3 kiểm trước khi ghi).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_browser(&self, name: &str, browser: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE accounts_meta SET browser = ?2 WHERE name = ?1")
            .bind(name)
            .bind(browser)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// Trạng thái phiên (Task 16 `AccountPool::mark_logged_out` + Verify/cookie ở P3):
    /// - `ok = false` (browser_pool.py:651-654 `mark_logged_out`): `login_ok=0`,
    ///   `login_checked_at=at`, `logged_out_at=at`; KHÔNG đụng `auto_verify_*` (`:663` ghi
    ///   `revoked` riêng qua [`AccountRepo::set_auto_verify`]). Lưu ý: đường verify-*thất bại*
    ///   (`:864-865`) không ghi `logged_out_at` — P3 không được dùng hàm này cho đường đó.
    /// - `ok = true` (browser_pool.py:864-866 verify thành công / 590-593 `clear_logged_out`):
    ///   `login_ok=1`, `login_checked_at=at`, `logged_out_at=0`, `auto_verify_state=''`,
    ///   `auto_verify_tries=0` (review T16 I1).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_login_ok(&self, name: &str, ok: bool, at: f64) -> Result<(), StoreError> {
        let logged_out_at = if ok { 0.0 } else { at };
        sqlx::query(
            "UPDATE accounts_meta SET login_ok = ?2, login_checked_at = ?3, logged_out_at = ?4,
               auto_verify_state = CASE WHEN ?2 = 1 THEN '' ELSE auto_verify_state END,
               auto_verify_tries = CASE WHEN ?2 = 1 THEN 0  ELSE auto_verify_tries END
             WHERE name = ?1",
        )
        .bind(name)
        .bind(i64::from(ok))
        .bind(at)
        .bind(logged_out_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// browser_pool.py:599-601 `_set_auto_verify`: `auto_verify_state` + `auto_verify_tries`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_auto_verify(
        &self,
        name: &str,
        state: &str,
        tries: i64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE accounts_meta SET auto_verify_state = ?2, auto_verify_tries = ?3 WHERE name = ?1",
        )
        .bind(name)
        .bind(state)
        .bind(tries)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// browser_pool.py:1225-1227 (`RiskControl`) / 1257-1259 (profile bận): `cooldown_until`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_cooldown_until(&self, name: &str, until: f64) -> Result<(), StoreError> {
        sqlx::query("UPDATE accounts_meta SET cooldown_until = ?2 WHERE name = ?1")
            .bind(name)
            .bind(until)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// browser_pool.py:484-487 `_mark_daily_limit`: `rate_limited_until` + `limit_reason`
    /// (`last_used_at` ghi riêng bằng [`AccountRepo::touch_last_used`]; cắt 300 ký tự là việc của
    /// tầng gọi).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_rate_limited_until(
        &self,
        name: &str,
        until: f64,
        reason: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE accounts_meta SET rate_limited_until = ?2, limit_reason = ?3 WHERE name = ?1",
        )
        .bind(name)
        .bind(until)
        .bind(reason)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// browser_pool.py:469-474 `_mark_quota_blocked`: `quota_blocked_until` + `quota_reason`
    /// (`last_used_at` ghi riêng bằng [`AccountRepo::touch_last_used`]).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_quota_blocked_until(
        &self,
        name: &str,
        until: f64,
        reason: &str,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE accounts_meta SET quota_blocked_until = ?2, quota_reason = ?3 WHERE name = ?1",
        )
        .bind(name)
        .bind(until)
        .bind(reason)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// browser_pool.py:1187-1189 (sau `_claim`) / 1234-1236 (timeout): `last_used_at = now`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn touch_last_used(&self, name: &str, now: f64) -> Result<(), StoreError> {
        sqlx::query("UPDATE accounts_meta SET last_used_at = ?2 WHERE name = ?1")
            .bind(name)
            .bind(now)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    /// browser_pool.py:881-885 `_set_credit_balance` (chỉ 2 cột số dư; phần `quota_blocked` khi
    /// `balance < 2` là việc của tầng gọi) và 893-897 `_unblock_quota` (`None`, `0.0`).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_credit_balance(
        &self,
        name: &str,
        balance: Option<i64>,
        checked_at: f64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "UPDATE accounts_meta SET credit_balance = ?2, credit_checked_at = ?3 WHERE name = ?1",
        )
        .bind(name)
        .bind(balance)
        .bind(checked_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// browser_pool.py:459-467 `_clear_expired_rate_limits`: mốc `rate_limited_until` /
    /// `quota_blocked_until` đã qua (`> 0 AND <= now`) → 0 và xoá lý do; ruling Task 16 gộp thêm
    /// `cooldown_until` (Python để nguyên mốc cũ, chỉ so `> now` khi đọc — kết quả như nhau).
    /// Mỗi cột xét độc lập — KHÁC Python: một `UPDATE` xoá cả 4 cột khi *một trong hai* mốc hết
    /// hạn (mốc quota chưa hết cũng bị xoá theo); ở đây chỉ xoá đúng cột đã hết hạn (review T16
    /// M3, chấp nhận — thực tế hai mốc luôn cùng bằng lần reset kế tiếp). Trả về số dòng bị chạm.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn clear_expired_limits(&self, now: f64) -> Result<u64, StoreError> {
        let res = sqlx::query(
            "UPDATE accounts_meta SET
               rate_limited_until  = CASE WHEN rate_limited_until  > 0 AND rate_limited_until  <= ?1 THEN 0  ELSE rate_limited_until  END,
               limit_reason        = CASE WHEN rate_limited_until  > 0 AND rate_limited_until  <= ?1 THEN '' ELSE limit_reason        END,
               quota_blocked_until = CASE WHEN quota_blocked_until > 0 AND quota_blocked_until <= ?1 THEN 0  ELSE quota_blocked_until END,
               quota_reason        = CASE WHEN quota_blocked_until > 0 AND quota_blocked_until <= ?1 THEN '' ELSE quota_reason        END,
               cooldown_until      = CASE WHEN cooldown_until      > 0 AND cooldown_until      <= ?1 THEN 0  ELSE cooldown_until      END
             WHERE (rate_limited_until  > 0 AND rate_limited_until  <= ?1)
                OR (quota_blocked_until > 0 AND quota_blocked_until <= ?1)
                OR (cooldown_until      > 0 AND cooldown_until      <= ?1)",
        )
        .bind(now)
        .execute(self.db.pool())
        .await?;
        Ok(res.rows_affected())
    }

    /// browser_pool.py:480-483 `_mark_daily_limit`: đặt `used` của (tài khoản, ngày, mode) lên
    /// **ít nhất** `used` — `ON CONFLICT DO UPDATE SET used = MAX(used, excluded.used)`, KHÔNG hạ
    /// số đã dùng (hạn mức bị hạ sau khi đã dùng nhiều hơn thì giữ số lớn).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi thất bại.
    pub async fn set_usage(
        &self,
        account: &str,
        day: &str,
        mode: Mode,
        used: u32,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO usage (account, day, mode, used) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(account, day, mode) DO UPDATE SET used = MAX(used, excluded.used)",
        )
        .bind(account)
        .bind(day)
        .bind(mode.as_str())
        .bind(i64::from(used))
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
