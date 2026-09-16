use serde::{Deserialize, Serialize};

use crate::{CoreError, Limits};

/// Chế độ tạo video của Dola.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Fast,
    Pro,
    ProMax,
}

impl Mode {
    /// Chuỗi rỗng / chỉ khoảng trắng → `Fast` (giữ hành vi `normalize_mode`).
    ///
    /// # Errors
    /// `CoreError::InvalidMode` khi giá trị không thuộc danh sách alias.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let key = value.trim().to_lowercase();
        if key.is_empty() {
            return Ok(Self::Fast);
        }
        match key.as_str() {
            "fast" | "nhanh" | "高速" | "quick" | "default" => Ok(Self::Fast),
            "pro" | "プロ" | "professional" => Ok(Self::Pro),
            "pro_max" | "promax" | "pro-max" | "skill" => Ok(Self::ProMax),
            _ => Err(CoreError::InvalidMode(value.to_string())),
        }
    }

    /// `pro` và `pro_max` cùng dùng nút プロ, cùng nhận link mp4 trong text_block.
    // text_block là văn xuôi trong doc comment do brief quy định, không phải định danh code.
    #[allow(clippy::doc_markdown)]
    #[must_use]
    pub fn is_pro_like(self) -> bool {
        matches!(self, Self::Pro | Self::ProMax)
    }

    /// fast chỉ có 10s (30s nhờ extension); pro nhận 15s qua prompt.
    #[must_use]
    pub fn allowed_durations(self) -> &'static [u16] {
        if self.is_pro_like() {
            &[10, 15, 30]
        } else {
            &[10, 30]
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Pro => "pro",
            Self::ProMax => "pro_max",
        }
    }

    /// Nhãn nút chế độ trên trang Dola (ja-JP). `pro_max` bấm cùng nút với `pro`.
    #[must_use]
    pub fn button_label(self) -> &'static str {
        match self {
            Self::Fast => "高速",
            Self::Pro | Self::ProMax => "プロ",
        }
    }
}

impl Limits {
    /// Hạn mức video/ngày/tài khoản theo chế độ, kẹp sàn 1 như bản Python.
    #[must_use]
    pub fn daily_limit(&self, mode: Mode) -> u32 {
        match mode {
            Mode::Fast => self.daily_limit_fast.max(1),
            Mode::Pro => self.daily_limit_pro.max(1),
            Mode::ProMax => self.daily_limit_pro_max.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Limits;

    #[test]
    fn alias_dung_nhu_ban_python() {
        for v in [
            "fast", "nhanh", "高速", "quick", "default", "FAST", "  fast  ",
        ] {
            assert_eq!(Mode::parse(v).unwrap(), Mode::Fast, "{v}");
        }
        for v in ["pro", "プロ", "professional"] {
            assert_eq!(Mode::parse(v).unwrap(), Mode::Pro, "{v}");
        }
        for v in ["pro_max", "promax", "pro-max", "skill"] {
            assert_eq!(Mode::parse(v).unwrap(), Mode::ProMax, "{v}");
        }
    }

    #[test]
    fn rong_thi_ve_fast() {
        assert_eq!(Mode::parse("").unwrap(), Mode::Fast);
        assert_eq!(Mode::parse("   ").unwrap(), Mode::Fast);
        assert_eq!(Mode::default(), Mode::Fast);
    }

    #[test]
    fn la_thi_loi_co_thong_diep_dung() {
        let err = Mode::parse("bogus").unwrap_err();
        assert_eq!(
            err.to_string(),
            "mode must be fast, pro or pro_max (got 'bogus')"
        );
    }

    #[test]
    fn thoi_luong_cho_phep_theo_che_do() {
        assert_eq!(Mode::Fast.allowed_durations(), &[10, 30]);
        assert_eq!(Mode::Pro.allowed_durations(), &[10, 15, 30]);
        assert_eq!(Mode::ProMax.allowed_durations(), &[10, 15, 30]);
    }

    #[test]
    fn pro_max_dung_nut_pro() {
        assert_eq!(Mode::Fast.button_label(), "高速");
        assert_eq!(Mode::Pro.button_label(), "プロ");
        assert_eq!(Mode::ProMax.button_label(), "プロ");
        assert!(!Mode::Fast.is_pro_like());
        assert!(Mode::Pro.is_pro_like());
        assert!(Mode::ProMax.is_pro_like());
    }

    #[test]
    fn han_muc_ngay_lay_tu_limits() {
        let l = Limits::default();
        assert_eq!(l.daily_limit(Mode::Fast), 2);
        assert_eq!(l.daily_limit(Mode::Pro), 10);
        assert_eq!(l.daily_limit(Mode::ProMax), 10);
        let zero = Limits {
            daily_limit_fast: 0,
            ..Limits::default()
        };
        assert_eq!(zero.daily_limit(Mode::Fast), 1, "ban Python kep san >= 1");
    }
}
