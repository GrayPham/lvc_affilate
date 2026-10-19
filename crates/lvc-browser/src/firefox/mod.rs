//! Engine `FirefoxPortable` (xem `lvc_core::BrowserEngine`): tiêm cookie vào profile Firefox để
//! người dùng MỞ TAY đã đăng nhập sẵn. KHÔNG có driver automation Firefox — CDP là của Chromium;
//! tài khoản engine này không verify/chạy workflow tự động
//! (`docs/superpowers/plans/2026-10-18-firefox-portable-cookie-profiles.md`).
//!
//! Chia nhỏ: một file một trách nhiệm.
//! - `locate`: dò `firefox.exe`.
//! - `prefs`: dựng nội dung `user.js` đặt proxy cho profile (hàm thuần).
//! (Ghi cookie vào `cookies.sqlite` nằm ở `lvc_store::firefox_cookies` — xem doc crate đó.)

pub mod locate;
pub mod prefs;

pub use locate::{FirefoxRoots, resolve_firefox, resolve_firefox_in};
pub use prefs::user_js_for_proxy;
