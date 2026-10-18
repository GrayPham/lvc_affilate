//! Tiêm cookie vào `cookies.sqlite` của một profile Firefox — backend cho engine `FirefoxPortable`
//! (`lvc_core::BrowserEngine`). Firefox PHẢI đóng khi ghi (nếu không DB bị khoá).
//!
//! Vì sao ở `lvc-store` (không ở `lvc-browser`): đây là ghi `SQLite`, mà `sqlx` chỉ nằm ở crate này;
//! thêm `rusqlite` cho `lvc-browser` sẽ ĐỤNG native lib `sqlite3` với `sqlx` (Cargo cấm hai gói
//! cùng `links="sqlite3"`). `lvc-browser::firefox::locate` lo phần dò `firefox.exe`; `lvc-engine`
//! ghép hai mảnh.
//!
//! ⚠️ SCHEMA DỄ VỠ: bảng `moz_cookies` theo Firefox hiện hành (schema v13). Nếu bản Firefox của
//! người dùng dùng schema khác, Firefox có thể DỰNG LẠI `cookies.sqlite` lúc khởi động và bỏ cookie
//! ta tiêm. KHÔNG kiểm được trong CI (không có Firefox) — phải nghiệm thu tay: mở Firefox Portable
//! thấy đã đăng nhập. Test ở đây chỉ bảo đảm GHI/ĐỌC-LẠI đúng (round-trip qua sqlx).
//!
//! An toàn bí mật (luật #3): KHÔNG log giá trị cookie — chỉ log SỐ LƯỢNG.

use std::path::{Path, PathBuf};

use lvc_core::Cookie;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::StoreError;

/// Cookie phiên (không hạn) được tiêm với hạn xa để Firefox GIỮ LẠI qua lần mở sau. Firefox chặn
/// hạn cookie quá ~400 ngày; dùng đúng mốc đó.
const SESSION_TTL_SECS: i64 = 400 * 86_400;

/// Một hàng `moz_cookies` đã ánh xạ sẵn — tách khỏi I/O để test thuần.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MozCookie {
    pub origin_attributes: String,
    pub name: String,
    pub value: String,
    pub host: String,
    pub path: String,
    /// Giây (epoch).
    pub expiry: i64,
    /// Micro-giây (epoch) — Firefox lưu mốc truy cập/tạo ở micro-giây.
    pub last_accessed: i64,
    pub creation_time: i64,
    pub is_secure: i64,
    pub is_http_only: i64,
    pub same_site: i64,
    /// Bitmask: 2 = https; 3 = http|https (cookie không-secure dùng được cả hai).
    pub scheme_map: i64,
}

/// Ánh xạ cookie (Netscape đã parse) → hàng `moz_cookies`. THUẦN — `now_micros` truyền vào để test.
/// Cookie phiên (`expires = None`) nhận hạn `now + 400 ngày` để Firefox giữ lại.
#[must_use]
pub fn to_moz_rows(cookies: &[Cookie], now_micros: i64) -> Vec<MozCookie> {
    let now_secs = now_micros / 1_000_000;
    cookies
        .iter()
        .map(|c| {
            let path = if c.path.is_empty() { "/" } else { &c.path };
            MozCookie {
                origin_attributes: String::new(),
                name: c.name.clone(),
                value: c.value.clone(),
                host: c.domain.clone(),
                path: path.to_string(),
                expiry: c.expires.unwrap_or(now_secs + SESSION_TTL_SECS),
                last_accessed: now_micros,
                creation_time: now_micros,
                is_secure: i64::from(c.secure),
                is_http_only: i64::from(c.http_only),
                same_site: 0,
                // Secure → chỉ https (2); không-secure → cả http lẫn https (3) để chắc chắn được
                // gửi trên trang https (đa số cookie đăng nhập chạy qua https).
                scheme_map: if c.secure { 2 } else { 3 },
            }
        })
        .collect()
}

/// `cookies.sqlite` trong một profile Firefox.
#[must_use]
pub fn cookies_db_path(profile_dir: &Path) -> PathBuf {
    profile_dir.join("cookies.sqlite")
}

const CREATE_MOZ_COOKIES: &str = "CREATE TABLE IF NOT EXISTS moz_cookies (\
    id INTEGER PRIMARY KEY, \
    originAttributes TEXT NOT NULL DEFAULT '', \
    name TEXT, value TEXT, host TEXT, path TEXT, \
    expiry INTEGER, lastAccessed INTEGER, creationTime INTEGER, \
    isSecure INTEGER, isHttpOnly INTEGER, \
    inBrowserElement INTEGER DEFAULT 0, \
    sameSite INTEGER DEFAULT 0, rawSameSite INTEGER DEFAULT 0, \
    schemeMap INTEGER DEFAULT 0, \
    CONSTRAINT moz_uniqueid UNIQUE (name, host, path, originAttributes))";

/// Tiêm `cookies` vào `cookies.sqlite` của `profile_dir` (tạo file + bảng nếu chưa có). Trả về SỐ
/// cookie đã ghi. Firefox PHẢI đóng.
///
/// # Errors
/// [`StoreError::Sqlx`] khi không mở/ghi được DB (vd Firefox đang mở giữ khoá).
pub async fn write_cookies(
    profile_dir: &Path,
    cookies: &[Cookie],
    now_micros: i64,
) -> Result<usize, StoreError> {
    std::fs::create_dir_all(profile_dir).map_err(|e| {
        StoreError::Sqlx(sqlx::Error::Io(std::io::Error::new(
            e.kind(),
            format!("tao thu muc profile Firefox: {e}"),
        )))
    })?;
    let path = cookies_db_path(profile_dir);
    let opts = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;
    let n = write_into(&pool, cookies, now_micros).await;
    pool.close().await; // checkpoint + dong sach de Firefox doc duoc ngay
    n
}

/// Phần ghi thuần-SQL (tách để test dùng chung pool). Dựng bảng nếu thiếu; nếu DB mới tinh
/// (`user_version = 0`) thì đặt `user_version = 13` để Firefox không coi là DB chưa khởi tạo.
async fn write_into(
    pool: &SqlitePool,
    cookies: &[Cookie],
    now_micros: i64,
) -> Result<usize, StoreError> {
    sqlx::query(CREATE_MOZ_COOKIES).execute(pool).await?;
    let ver: i64 = sqlx::query("PRAGMA user_version")
        .fetch_one(pool)
        .await?
        .get(0);
    if ver == 0 {
        sqlx::query("PRAGMA user_version = 13")
            .execute(pool)
            .await?;
    }
    let rows = to_moz_rows(cookies, now_micros);
    for r in &rows {
        sqlx::query(
            "INSERT OR REPLACE INTO moz_cookies \
             (originAttributes, name, value, host, path, expiry, lastAccessed, creationTime, \
              isSecure, isHttpOnly, inBrowserElement, sameSite, rawSameSite, schemeMap) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, 0, ?12)",
        )
        .bind(&r.origin_attributes)
        .bind(&r.name)
        .bind(&r.value)
        .bind(&r.host)
        .bind(&r.path)
        .bind(r.expiry)
        .bind(r.last_accessed)
        .bind(r.creation_time)
        .bind(r.is_secure)
        .bind(r.is_http_only)
        .bind(r.same_site)
        .bind(r.scheme_map)
        .execute(pool)
        .await?;
    }
    Ok(rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(name: &str, secure: bool, expires: Option<i64>) -> Cookie {
        Cookie {
            name: name.to_string(),
            value: format!("val-{name}"),
            domain: ".example.com".to_string(),
            path: String::new(),
            secure,
            http_only: true,
            expires,
        }
    }

    #[test]
    fn to_moz_rows_anh_xa_dung_don_vi_va_mac_dinh() {
        let now_micros = 1_700_000_000_000_000; // 1_700_000_000 giây
        let rows = to_moz_rows(&[cookie("sess", true, None)], now_micros);
        let r = &rows[0];
        assert_eq!(r.path, "/", "path rong -> '/'");
        assert_eq!(r.host, ".example.com");
        assert_eq!(r.creation_time, now_micros);
        assert_eq!(r.last_accessed, now_micros);
        // Cookie phien -> han = now_secs + 400 ngay.
        assert_eq!(r.expiry, 1_700_000_000 + SESSION_TTL_SECS);
        assert_eq!(r.is_secure, 1);
        assert_eq!(r.scheme_map, 2, "secure -> https");
    }

    #[test]
    fn to_moz_rows_khong_secure_scheme_map_3_va_giu_han() {
        let rows = to_moz_rows(&[cookie("a", false, Some(123))], 2_000_000);
        assert_eq!(rows[0].expiry, 123, "co han thi giu nguyen");
        assert_eq!(rows[0].scheme_map, 3, "khong secure -> http|https");
        assert_eq!(rows[0].is_secure, 0);
    }

    #[tokio::test]
    async fn write_roi_doc_lai_dung_so_va_gia_tri() {
        let dir = tempfile::tempdir().unwrap();
        let cookies = vec![
            cookie("sessionid", true, Some(1_900_000_000)),
            cookie("csrf", false, None),
        ];
        let n = write_cookies(dir.path(), &cookies, 1_700_000_000_000_000)
            .await
            .unwrap();
        assert_eq!(n, 2);

        // Doc lai bang sqlx (khong can Firefox).
        let path = cookies_db_path(dir.path());
        let opts = SqliteConnectOptions::new().filename(&path);
        let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
        let ver: i64 = sqlx::query("PRAGMA user_version")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(ver, 13, "DB moi -> user_version=13");
        let count: i64 = sqlx::query("SELECT COUNT(*) FROM moz_cookies")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 2);
        let host: String = sqlx::query("SELECT host FROM moz_cookies WHERE name='sessionid'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(host, ".example.com");
        pool.close().await;
    }

    #[tokio::test]
    async fn ghi_lai_cung_cookie_thi_thay_the_khong_nhan_doi() {
        let dir = tempfile::tempdir().unwrap();
        let c = vec![cookie("sessionid", true, Some(1))];
        write_cookies(dir.path(), &c, 1_000_000).await.unwrap();
        // Lan 2: cung (name, host, path, originAttributes) -> REPLACE, van 1 hang.
        let c2 = vec![cookie("sessionid", true, Some(2))];
        write_cookies(dir.path(), &c2, 2_000_000).await.unwrap();

        let path = cookies_db_path(dir.path());
        let pool = SqlitePoolOptions::new()
            .connect_with(SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
        let count: i64 = sqlx::query("SELECT COUNT(*) FROM moz_cookies")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 1, "REPLACE theo khoa duy nhat, khong nhan doi");
        let expiry: i64 = sqlx::query("SELECT expiry FROM moz_cookies WHERE name='sessionid'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(expiry, 2, "ban ghi moi de ban cu");
        pool.close().await;
    }
}
