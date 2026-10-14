//! Kênh chạy Dola (`dola_channel`) — tách riêng file vì chỉ liên quan tới Dola, thêm/bỏ nền tảng
//! khác không phải sửa file này.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::parse::parse_enum;

/// Kênh chạy Dola (P4b): `Api` = gọi API app trực tiếp (ký X-Gorgon/Ladon/Argus, KHÔNG trình
/// duyệt); `Browser` = backend trình duyệt headless (P2/P3). Mặc định `Api`; lỗi ký → engine tự
/// chuyển sang `Browser`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DolaChannel {
    /// Gọi API app Android trực tiếp (nhanh, không Chromium).
    #[default]
    Api,
    /// Trình duyệt headless dùng phiên đăng nhập thật.
    Browser,
}

impl DolaChannel {
    /// Chuỗi wire (`"api"`/`"browser"`) — dùng cho bảng `settings` + hợp đồng UI.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Browser => "browser",
        }
    }

    /// Parse từ chuỗi (không phân biệt hoa/thường). Sai → `Err`.
    ///
    /// # Errors
    /// Chuỗi không phải `"api"`/`"browser"`.
    #[allow(
        clippy::result_unit_err,
        reason = "cho goi duy nhat (parse_dola_channel) chi can biet dung/sai, khong can chi tiet loi"
    )]
    pub fn parse(s: &str) -> Result<Self, ()> {
        match s.trim().to_lowercase().as_str() {
            "api" => Ok(Self::Api),
            "browser" => Ok(Self::Browser),
            _ => Err(()),
        }
    }
}

pub(super) fn parse_dola_channel(
    map: &HashMap<String, String>,
    warnings: &mut Vec<String>,
) -> DolaChannel {
    parse_enum(
        map,
        "dola_channel",
        DolaChannel::default(),
        DolaChannel::default().as_str(),
        DolaChannel::parse,
        warnings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_khong_phan_biet_hoa_thuong() {
        assert_eq!(DolaChannel::parse("API").unwrap(), DolaChannel::Api);
        assert_eq!(
            DolaChannel::parse(" Browser ").unwrap(),
            DolaChannel::Browser
        );
        assert!(DolaChannel::parse("xyz").is_err());
    }

    #[test]
    fn as_str_khop_chuoi_wire() {
        assert_eq!(DolaChannel::Api.as_str(), "api");
        assert_eq!(DolaChannel::Browser.as_str(), "browser");
    }
}
