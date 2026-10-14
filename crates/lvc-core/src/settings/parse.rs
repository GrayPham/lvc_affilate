//! Hàm đọc giá trị thô từ bảng `settings` — dùng chung cho mọi khoá, KHÔNG biết khoá nào tồn tại.
//!
//! Quy ước của cả module `settings` (xem `mod.rs`): khoá THIẾU → mặc định, **không** cảnh báo;
//! khoá CÓ nhưng giá trị sai → mặc định + MỘT dòng cảnh báo. Một khoá sai không bao giờ làm hỏng
//! các khoá khác.

use std::collections::HashMap;

/// Ghi một dòng cảnh báo đúng định dạng chung: `"<khoá>: '<raw>' khong hop le, dung <mặc định>"`.
pub(super) fn invalid(
    warnings: &mut Vec<String>,
    key: &str,
    raw: &str,
    default: impl std::fmt::Display,
) {
    warnings.push(format!("{key}: '{raw}' khong hop le, dung {default}"));
}

/// `1/0/true/false/yes/no`, không phân biệt hoa/thường (Python lưu "1"/"0", form UI có thể gửi
/// "true"/"false"/"yes"/"no").
pub(super) fn parse_bool(
    map: &HashMap<String, String>,
    key: &str,
    default: bool,
    warnings: &mut Vec<String>,
) -> bool {
    match map.get(key) {
        None => default,
        Some(raw) => match raw.trim().to_lowercase().as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => {
                invalid(warnings, key, raw, default);
                default
            }
        },
    }
}

pub(super) fn get_string(map: &HashMap<String, String>, key: &str, default: &str) -> String {
    map.get(key).cloned().unwrap_or_else(|| default.to_string())
}

/// Đọc một khoá enum dạng chuỗi: `parse` trả `Err` → mặc định + cảnh báo.
///
/// Gom về một chỗ để mỗi enum trong các file con chỉ cần khai `as_str`/`parse`, không lặp lại
/// đoạn `match map.get(..) { None => .., Some(raw) => .. }` cho từng khoá.
///
/// Generic cả kiểu LỖI vì hai họ enum cùng dùng hàm này: `Mode`/`Model`/`Ratio` trả
/// `Result<_, CoreError>` (parse được gọi cả từ tầng TXT/CLI nên cần chi tiết lỗi), còn các enum
/// chỉ sống trong cài đặt trả `Result<_, ()>`. Ở đây nội dung lỗi không dùng tới — cảnh báo luôn
/// ghi theo [`invalid`] để mọi khoá có cùng một định dạng thông điệp.
pub(super) fn parse_enum<T: Copy, E>(
    map: &HashMap<String, String>,
    key: &str,
    default: T,
    default_str: &str,
    parse: impl Fn(&str) -> Result<T, E>,
    warnings: &mut Vec<String>,
) -> T {
    match map.get(key) {
        None => default,
        Some(raw) => parse(raw).unwrap_or_else(|_| {
            invalid(warnings, key, raw, default_str);
            default
        }),
    }
}

/// Đọc một khoá số nguyên chỉ nhận MỘT TẬP giá trị cho trước (vd nghỉ giữa workflow: 0/30/60/180).
/// Ngoài tập đó → mặc định + cảnh báo.
pub(super) fn parse_one_of_u16(
    map: &HashMap<String, String>,
    key: &str,
    allowed: &[u16],
    default: u16,
    warnings: &mut Vec<String>,
) -> u16 {
    match map.get(key) {
        None => default,
        Some(raw) => {
            let parsed = raw
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|v| allowed.contains(v));
            if let Some(v) = parsed {
                v
            } else {
                invalid(warnings, key, raw, default);
                default
            }
        }
    }
}

pub(super) fn bool_str(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn parse_one_of_u16_ngoai_tap_ve_mac_dinh() {
        let mut w = Vec::new();
        let m = map(&[("rest", "45")]);
        assert_eq!(parse_one_of_u16(&m, "rest", &[0, 30, 60], 30, &mut w), 30);
        assert_eq!(w.len(), 1);
        assert!(w[0].contains("rest"), "{w:?}");
    }

    #[test]
    fn parse_one_of_u16_trong_tap_khong_canh_bao() {
        let mut w = Vec::new();
        let m = map(&[("rest", "60")]);
        assert_eq!(parse_one_of_u16(&m, "rest", &[0, 30, 60], 30, &mut w), 60);
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn parse_one_of_u16_thieu_khoa_khong_canh_bao() {
        let mut w = Vec::new();
        let m = map(&[]);
        assert_eq!(parse_one_of_u16(&m, "rest", &[0, 30], 30, &mut w), 30);
        assert!(w.is_empty(), "khoa thieu KHONG duoc canh bao: {w:?}");
    }

    #[test]
    fn parse_enum_sai_ve_mac_dinh_co_canh_bao() {
        let mut w = Vec::new();
        let m = map(&[("k", "xyz")]);
        let got = parse_enum(
            &m,
            "k",
            1_u8,
            "mot",
            |s| if s == "mot" { Ok(1) } else { Err(()) },
            &mut w,
        );
        assert_eq!(got, 1);
        assert_eq!(w.len(), 1);
    }
}
