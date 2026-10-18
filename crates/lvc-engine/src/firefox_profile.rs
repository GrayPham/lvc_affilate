//! Engine `FirefoxPortable` (`lvc_core::BrowserEngine`): tiêm cookie vào profile Firefox để người
//! dùng MỞ TAY đã đăng nhập sẵn. KHÔNG CDP, KHÔNG automation — tách hẳn khỏi `profiles.rs` (đường
//! Chromium/CDP) theo luật chia module: đây là "kịch bản" độc lập.
//!
//! Luồng khác `provision_profile` (Chromium): KHÔNG `launch`/`add_cookies` qua trình duyệt mà ghi
//! thẳng `cookies.sqlite` (`lvc_store::firefox_cookies`) khi Firefox CHƯA chạy. "Mở trình duyệt" =
//! spawn `firefox.exe -no-remote -profile <dir>` (tiến trình thường, không theo dõi).
//!
//! Giới hạn: tài khoản engine Firefox KHÔNG verify/chạy workflow tự động (không có driver CDP cho
//! Firefox) — xem `docs/superpowers/plans/2026-10-18-firefox-portable-cookie-profiles.md`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use lvc_browser::BrowserDriver;
use lvc_browser::firefox::resolve_firefox;

use crate::accounts::AccountError;
use crate::generate::Engine;
use crate::login::safe_component;

/// Mốc hiện tại tính bằng micro-giây epoch (Firefox lưu `creationTime`/`lastAccessed` micro-giây).
fn now_micros() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX))
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Tiêm cookie (Netscape) vào `cookies.sqlite` của MỘT profile Firefox cho `provider`/`label`.
    /// Thư mục: `accounts_dir/.logins/firefox/<provider>/<label an toàn>` — tách khỏi profile
    /// Chromium cùng tên (`.logins/<provider>/...`) để hai engine không giẫm lên nhau. Trả
    /// `profile_dir` để caller lưu `meta.profile_dir`.
    ///
    /// Firefox PHẢI chưa chạy trên profile này (nếu không `cookies.sqlite` bị khoá).
    ///
    /// # Errors
    /// - [`AccountError::CookieInvalid`]: `cookie_text` không đọc được cookie nào.
    /// - [`AccountError::Store`]: không ghi được `cookies.sqlite` (vd Firefox đang mở giữ khoá).
    pub async fn provision_firefox_profile(
        &self,
        provider: &str,
        label: &str,
        cookie_text: &str,
    ) -> Result<PathBuf, AccountError> {
        let cookies = lvc_core::cookie::parse_netscape(cookie_text);
        if cookies.is_empty() {
            return Err(AccountError::CookieInvalid(
                "khong doc duoc cookie nao tu noi dung da dan (kiem tra dinh dang Netscape)"
                    .to_string(),
            ));
        }
        let profile_dir = self
            .cfg
            .accounts_dir
            .join(".logins")
            .join("firefox")
            .join(safe_component(provider))
            .join(safe_component(label));
        lvc_store::firefox_cookies::write_cookies(&profile_dir, &cookies, now_micros()).await?;
        Ok(profile_dir)
    }

    /// Mở Firefox Portable trên `profile_dir` cho người dùng dùng tay (đã đăng nhập nhờ cookie đã
    /// tiêm). `-no-remote` để mở instance riêng với đúng profile này kể cả khi đã có Firefox khác
    /// đang chạy. Trả ngay sau khi spawn — KHÔNG đợi người dùng đóng.
    ///
    /// # Errors
    /// - [`AccountError::Browser`]: không tìm thấy `firefox.exe` (đặt env `LVC_FIREFOX`).
    /// - [`AccountError::Io`]: không spawn được tiến trình.
    pub fn open_firefox_viewer(&self, profile_dir: &Path) -> Result<(), AccountError> {
        let exe = resolve_firefox()?;
        Command::new(exe)
            .arg("-no-remote")
            .arg("-profile")
            .arg(profile_dir)
            .spawn()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::generate::tests::fx;

    const COOKIE_TEXT: &str = ".example.com\tTRUE\t/\tTRUE\t1900000000\tsessionid\tabc123\n";

    #[tokio::test]
    async fn provision_firefox_tao_profile_va_ghi_cookies_sqlite() {
        let f = fx(&[], vec![]).await;
        let dir = f
            .engine
            .provision_firefox_profile("museai", "acc 1", COOKIE_TEXT)
            .await
            .unwrap();
        let expected = f
            .accounts_dir()
            .join(".logins")
            .join("firefox")
            .join("museai")
            .join("acc_1");
        assert_eq!(dir, expected);
        assert!(
            dir.join("cookies.sqlite").is_file(),
            "phai ghi cookies.sqlite vao profile Firefox"
        );
    }

    #[tokio::test]
    async fn provision_firefox_cookie_rong_bao_loi_khong_tao_thu_muc() {
        let f = fx(&[], vec![]).await;
        let err = f
            .engine
            .provision_firefox_profile("museai", "acc1", "khong-phai-cookie")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "COOKIE_INVALID");
        let dir = f
            .accounts_dir()
            .join(".logins")
            .join("firefox")
            .join("museai")
            .join("acc1");
        assert!(!dir.exists(), "cookie rong thi khong tao rac");
    }
}
