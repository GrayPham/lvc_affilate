//! Engine `FirefoxPortable` (xem `lvc_core::BrowserEngine`): tiêm cookie vào profile Firefox để
//! người dùng MỞ TAY đã đăng nhập sẵn. KHÔNG có driver automation Firefox — CDP là của Chromium;
//! tài khoản engine này không verify/chạy workflow tự động
//! (`docs/superpowers/plans/2026-10-18-firefox-portable-cookie-profiles.md`).
//!
//! Chia nhỏ: một file một trách nhiệm.
//! - `locate`: dò `firefox.exe`.
//! - `cookies_db`: ghi cookie vào `cookies.sqlite` của profile (GĐ2).

pub mod locate;

pub use locate::{FirefoxRoots, resolve_firefox, resolve_firefox_in};
