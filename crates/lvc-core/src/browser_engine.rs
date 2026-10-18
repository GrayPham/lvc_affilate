//! Engine trình duyệt theo tài khoản — TRỰC GIAO với [`crate::BrowserKind`].
//!
//! `BrowserKind` (chromium/chrome/opera) chỉ chọn *họ Chromium* nào cho `ChromiumDriver` (CDP).
//! `BrowserEngine` chọn CÁCH chạy hẳn một tài khoản:
//! - `Chromium`: đường cũ — `ChromiumDriver` + CDP (verify/workflow tự động được).
//! - `FirefoxPortable`: tiêm cookie thẳng vào `cookies.sqlite` của profile Firefox rồi mở Firefox
//!   cho người dùng DÙNG TAY. KHÔNG có driver automation Firefox (CDP là của Chromium), nên tài
//!   khoản engine này KHÔNG verify/chạy workflow tự động — xem
//!   `docs/superpowers/plans/2026-10-18-firefox-portable-cookie-profiles.md`.
//!
//! Thuần: chỉ chuẩn hoá tên; không dò `.exe`, không I/O.

use crate::CoreError;

/// Engine chạy một tài khoản. Khoá bền (lưu trong `meta.engine`); đổi tên phải migrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserEngine {
    /// Mặc định: Chromium + CDP (tự động hoá đầy đủ).
    #[default]
    Chromium,
    /// Firefox Portable: tiêm cookie, mở dùng tay. Không tự động hoá.
    FirefoxPortable,
}

impl BrowserEngine {
    /// Chuẩn hoá tên engine. Rỗng/thiếu → `Chromium` (giữ hành vi cũ cho tài khoản chưa gắn engine).
    ///
    /// # Errors
    /// [`CoreError::InvalidEngine`] khi `value` không khớp tên nào.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        match value.trim().to_lowercase().as_str() {
            "" | "chromium" | "chrome" | "default" => Ok(Self::Chromium),
            "firefox_portable" | "firefox-portable" | "firefox" | "firefoxportable" => {
                Ok(Self::FirefoxPortable)
            }
            _ => Err(CoreError::InvalidEngine(value.to_string())),
        }
    }

    /// Tên bền để lưu/so sánh (khoá trong `meta.engine`).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chromium => "chromium",
            Self::FirefoxPortable => "firefox_portable",
        }
    }

    /// Engine này có driver tự động hoá (verify/chạy workflow) không? Firefox: không.
    #[must_use]
    pub fn supports_automation(self) -> bool {
        matches!(self, Self::Chromium)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_dinh_la_chromium() {
        assert_eq!(BrowserEngine::default(), BrowserEngine::Chromium);
    }

    #[test]
    fn parse_chromium_gom_rong_va_chrome() {
        for v in ["", "chromium", "chrome", "DEFAULT", "  Chromium "] {
            assert_eq!(
                BrowserEngine::parse(v).unwrap(),
                BrowserEngine::Chromium,
                "{v}"
            );
        }
    }

    #[test]
    fn parse_firefox_cac_bien_the() {
        for v in [
            "firefox_portable",
            "firefox-portable",
            "firefox",
            "FirefoxPortable",
            "  FIREFOX_PORTABLE ",
        ] {
            assert_eq!(
                BrowserEngine::parse(v).unwrap(),
                BrowserEngine::FirefoxPortable,
                "{v}"
            );
        }
    }

    #[test]
    fn parse_la_thi_loi_liet_ke() {
        let e = BrowserEngine::parse("edge").unwrap_err();
        assert_eq!(
            e.to_string(),
            "engine must be one of chromium, firefox_portable (got 'edge')"
        );
    }

    #[test]
    fn as_str_va_round_trip() {
        for e in [BrowserEngine::Chromium, BrowserEngine::FirefoxPortable] {
            assert_eq!(BrowserEngine::parse(e.as_str()).unwrap(), e);
        }
    }

    #[test]
    fn chi_chromium_tu_dong_hoa() {
        assert!(BrowserEngine::Chromium.supports_automation());
        assert!(!BrowserEngine::FirefoxPortable.supports_automation());
    }
}
