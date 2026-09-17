//! Lựa chọn trình duyệt theo tài khoản (`HOP-DONG v1.5`, `ADR-007`) — thuần, không dò file `.exe`.
//! Nguồn: `src/browser/browsers.py` (`BROWSERS`, `_ALIASES`, `normalize_browser`).
//!
//! Chỉ chuẩn hoá TÊN; dò đường dẫn `.exe` (I/O) thuộc tầng khác, không thuộc `lvc-core`.

use crate::CoreError;

/// browsers.py:13 `BROWSERS = ("chromium", "chrome", "opera")` — CHỈ 3 loại, không có `edge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserKind {
    Chromium,
    Chrome,
    Opera,
}

impl BrowserKind {
    /// browsers.py:41-46 `normalize_browser` — copy nguyên bảng `_ALIASES` (browsers.py:15-19):
    /// rỗng/"chromium"/"playwright"/"bundled"/"default" → `Chromium`;
    /// "chrome"/"google chrome"/"googlechrome"/"google-chrome" → `Chrome`;
    /// "opera"/"opera one"/"operaone" → `Opera`; khác → lỗi liệt kê đúng 3 loại.
    ///
    /// # Errors
    /// `CoreError::InvalidBrowser` khi `value` không khớp alias nào (thông điệp copy nguyên văn
    /// bản Python — `browsers.py` KHÔNG có alias `edge`/`msedge`, khác gợi ý trong đặc tả brief).
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let key = value.trim().to_lowercase();
        match key.as_str() {
            "" | "chromium" | "playwright" | "bundled" | "default" => Ok(Self::Chromium),
            "chrome" | "google chrome" | "googlechrome" | "google-chrome" => Ok(Self::Chrome),
            "opera" | "opera one" | "operaone" => Ok(Self::Opera),
            _ => Err(CoreError::InvalidBrowser(value.to_string())),
        }
    }

    /// browsers.py:13 `BROWSERS` — tên công khai (đúng thứ tự liệt kê trong thông điệp lỗi).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chromium => "chromium",
            Self::Chrome => "chrome",
            Self::Opera => "opera",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_chromium() {
        for v in [
            "",
            "chromium",
            "playwright",
            "bundled",
            "default",
            "  DEFAULT  ",
        ] {
            assert_eq!(BrowserKind::parse(v).unwrap(), BrowserKind::Chromium, "{v}");
        }
    }

    #[test]
    fn alias_chrome() {
        for v in [
            "chrome",
            "google chrome",
            "googlechrome",
            "google-chrome",
            "CHROME",
            "  chrome  ",
        ] {
            assert_eq!(BrowserKind::parse(v).unwrap(), BrowserKind::Chrome, "{v}");
        }
    }

    #[test]
    fn alias_opera() {
        for v in ["opera", "opera one", "operaone", "OPERA"] {
            assert_eq!(BrowserKind::parse(v).unwrap(), BrowserKind::Opera, "{v}");
        }
    }

    #[test]
    fn khong_co_alias_edge_khac_dac_ta_brief_nhung_dung_ban_python() {
        // browsers.py khong co "edge"/"msedge" trong _ALIASES — giu dung nguon that.
        let err = BrowserKind::parse("edge").unwrap_err();
        assert_eq!(
            err.to_string(),
            "browser must be one of chromium, chrome, opera (got 'edge')"
        );
        assert!(BrowserKind::parse("msedge").is_err());
    }

    #[test]
    fn gia_tri_la_thi_loi_dung_thong_diep() {
        let err = BrowserKind::parse("bogus").unwrap_err();
        assert_eq!(
            err.to_string(),
            "browser must be one of chromium, chrome, opera (got 'bogus')"
        );
    }

    #[test]
    fn as_str_dung_ba_gia_tri() {
        assert_eq!(BrowserKind::Chromium.as_str(), "chromium");
        assert_eq!(BrowserKind::Chrome.as_str(), "chrome");
        assert_eq!(BrowserKind::Opera.as_str(), "opera");
    }
}
