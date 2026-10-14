//! Nhóm cài đặt **Chạy tác vụ**: hàng chờ xử sự thế nào khi một workflow lỗi, nghỉ bao lâu giữa
//! hai workflow, và khi tới lượt mà không có tài khoản dùng được thì làm gì.
//!
//! Đây là cài đặt THẬT, có người đọc: `lvc-app::commands::queue` lấy từng giá trị ở đây thay cho
//! hằng số cứng trước đó.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::parse::{parse_enum, parse_one_of_u16};

/// Các lựa chọn "nghỉ giữa hai workflow" (giây) — UI hiện đúng 4 nút này nên giá trị ngoài danh
/// sách bị coi là sai (xem [`parse_one_of_u16`]).
pub const REST_CHOICES: [u16; 4] = [0, 30, 60, 180];

/// Mặc định 30 giây — chạy liên tục không nghỉ dễ bị nhà cung cấp gắn cờ.
pub const REST_DEFAULT: u16 = 30;

/// Khi một workflow trong hàng chờ lỗi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    /// Đánh dấu lỗi rồi chạy workflow kế tiếp — mặc định (một lỗi không chặn cả đêm chạy).
    #[default]
    Skip,
    /// Dừng cả hàng chờ để người dùng xem lại.
    Stop,
}

impl OnError {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Stop => "stop",
        }
    }

    /// # Errors
    /// Chuỗi không phải `"skip"`/`"stop"`.
    #[allow(
        clippy::result_unit_err,
        reason = "cho goi duy nhat (parse_enum) chi can biet dung/sai"
    )]
    pub fn parse(s: &str) -> Result<Self, ()> {
        match s.trim().to_lowercase().as_str() {
            "skip" => Ok(Self::Skip),
            "stop" => Ok(Self::Stop),
            _ => Err(()),
        }
    }
}

/// Khi tới lượt chạy mà node AI không có tài khoản dùng được.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MissingAccount {
    /// Bỏ qua workflow đó, chạy tiếp — mặc định.
    #[default]
    Skip,
    /// Chờ tối đa [`MISSING_ACCOUNT_WAIT_SECS`] rồi mới bỏ qua (tài khoản có thể hồi hạn mức).
    Wait,
}

/// Thời gian chờ tối đa của [`MissingAccount::Wait`] — 30 phút, khớp nhãn trên UI.
pub const MISSING_ACCOUNT_WAIT_SECS: u64 = 30 * 60;

impl MissingAccount {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Wait => "wait",
        }
    }

    /// # Errors
    /// Chuỗi không phải `"skip"`/`"wait"`.
    #[allow(
        clippy::result_unit_err,
        reason = "cho goi duy nhat (parse_enum) chi can biet dung/sai"
    )]
    pub fn parse(s: &str) -> Result<Self, ()> {
        match s.trim().to_lowercase().as_str() {
            "skip" => Ok(Self::Skip),
            "wait" => Ok(Self::Wait),
            _ => Err(()),
        }
    }
}

pub(super) fn parse_on_error(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> OnError {
    parse_enum(
        map,
        "on_error",
        OnError::default(),
        OnError::default().as_str(),
        OnError::parse,
        warnings,
    )
}

pub(super) fn parse_missing_account(
    map: &HashMap<String, String>,
    warnings: &mut Vec<String>,
) -> MissingAccount {
    parse_enum(
        map,
        "missing_account",
        MissingAccount::default(),
        MissingAccount::default().as_str(),
        MissingAccount::parse,
        warnings,
    )
}

pub(super) fn parse_rest_between(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> u16 {
    parse_one_of_u16(map, "rest_between", &REST_CHOICES, REST_DEFAULT, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_dinh_khong_dung_ca_hang_cho() {
        assert_eq!(
            OnError::default(),
            OnError::Skip,
            "mot workflow loi khong duoc chan ca dem chay"
        );
        assert_eq!(MissingAccount::default(), MissingAccount::Skip);
        assert_eq!(REST_DEFAULT, 30);
    }

    #[test]
    fn as_str_parse_khop_nhau() {
        for v in [OnError::Skip, OnError::Stop] {
            assert_eq!(OnError::parse(v.as_str()).unwrap(), v);
        }
        for v in [MissingAccount::Skip, MissingAccount::Wait] {
            assert_eq!(MissingAccount::parse(v.as_str()).unwrap(), v);
        }
    }

    #[test]
    fn skip_la_chuoi_chung_nhung_hai_enum_khac_nhau() {
        // Ca hai enum dung chuoi "skip" — de lan khi doc bang settings, nen ghim lai: moi enum
        // doc tu KHOA RIENG (`on_error` / `missing_account`), khong bao gio dung chung mot khoa.
        assert_eq!(OnError::parse("skip").unwrap(), OnError::Skip);
        assert_eq!(MissingAccount::parse("skip").unwrap(), MissingAccount::Skip);
        assert!(OnError::parse("wait").is_err(), "on_error khong co 'wait'");
        assert!(
            MissingAccount::parse("stop").is_err(),
            "missing_account khong co 'stop'"
        );
    }

    #[test]
    fn rest_choices_co_du_0_va_mac_dinh() {
        assert!(REST_CHOICES.contains(&0), "phai co lua chon 'khong nghi'");
        assert!(REST_CHOICES.contains(&REST_DEFAULT));
    }
}
