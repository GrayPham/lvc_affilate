use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Tỉ lệ khung hình hợp lệ ở tầng cảnh — đúng 5 giá trị của `scenes.RATIOS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Ratio {
    #[default]
    #[serde(rename = "9:16")]
    R9x16,
    #[serde(rename = "16:9")]
    R16x9,
    #[serde(rename = "1:1")]
    R1x1,
    #[serde(rename = "4:3")]
    R4x3,
    #[serde(rename = "3:4")]
    R3x4,
}

impl Ratio {
    /// # Errors
    /// `CoreError::InvalidRatio` khi không thuộc 5 giá trị hợp lệ.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        match value.trim() {
            "9:16" => Ok(Self::R9x16),
            "16:9" => Ok(Self::R16x9),
            "1:1" => Ok(Self::R1x1),
            "4:3" => Ok(Self::R4x3),
            "3:4" => Ok(Self::R3x4),
            other => Err(CoreError::InvalidRatio(other.to_string())),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::R9x16 => "9:16",
            Self::R16x9 => "16:9",
            Self::R1x1 => "1:1",
            Self::R4x3 => "4:3",
            Self::R3x4 => "3:4",
        }
    }

    /// Cụm tiếng Nhật ghép vào prompt cho agent Pro.
    #[must_use]
    pub fn japanese(self) -> &'static str {
        match self {
            Self::R9x16 => "縦動画（9:16）",
            Self::R3x4 => "縦動画（3:4）",
            Self::R16x9 => "横動画（16:9）",
            Self::R4x3 => "横動画（4:3）",
            Self::R1x1 => "正方形動画（1:1）",
        }
    }
}

/// Model Seedance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Model {
    #[default]
    #[serde(rename = "seedance-2.0")]
    Seedance20,
    #[serde(rename = "seedance-2.5")]
    Seedance25,
    #[serde(rename = "seedance-1.0")]
    Seedance10,
}

impl Model {
    /// # Errors
    /// `CoreError::InvalidModel` khi alias không nhận ra.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let key = value.trim().to_lowercase();
        match key.as_str() {
            "2.0" | "2" | "seedance-2.0" | "seedance_2.0" | "seedance_v2.0" | "seedance-v2.0" => {
                Ok(Self::Seedance20)
            }
            "2.5" | "seedance-2.5" | "seedance_2.5" | "seedance_v2.5" | "seedance-v2.5" => {
                Ok(Self::Seedance25)
            }
            "1.0" | "1" | "seedance-1.0" | "seedance_1.0" | "seedance_v1.0" | "seedance-v1.0" => {
                Ok(Self::Seedance10)
            }
            _ => Err(CoreError::InvalidModel(value.trim().to_lowercase())),
        }
    }

    /// Tên công khai (API, TXT, DB).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seedance20 => "seedance-2.0",
            Self::Seedance25 => "seedance-2.5",
            Self::Seedance10 => "seedance-1.0",
        }
    }

    /// Tên Dola dùng trong `ability_param`.
    #[must_use]
    pub fn wire_str(self) -> &'static str {
        match self {
            Self::Seedance20 => "seedance_v2.0",
            Self::Seedance25 => "seedance_v2.5",
            Self::Seedance10 => "seedance_v1.0",
        }
    }
}

/// Cách xử lý logo/watermark sau khi tải video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogoMode {
    #[default]
    Off,
    Crop,
    Delogo,
    /// Dò & xoá watermark động (v0.55z).
    Auto,
}

impl LogoMode {
    /// Rỗng → `Off` (giữ hành vi `normalize_logo_mode`).
    ///
    /// # Errors
    /// `CoreError::InvalidLogo` khi không thuộc 3 giá trị.
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let key = value.trim().to_lowercase();
        if key.is_empty() {
            return Ok(Self::Off);
        }
        match key.as_str() {
            "off" => Ok(Self::Off),
            "crop" => Ok(Self::Crop),
            "delogo" => Ok(Self::Delogo),
            "auto" => Ok(Self::Auto),
            // Bản Python báo lỗi với giá trị GỐC (`value`), không phải bản đã
            // lowercase — giữ đúng để thông điệp khớp.
            _ => Err(CoreError::InvalidLogo(value.to_string())),
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Crop => "crop",
            Self::Delogo => "delogo",
            Self::Auto => "auto",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_dung_5_gia_tri() {
        for s in ["9:16", "16:9", "1:1", "4:3", "3:4"] {
            assert_eq!(Ratio::parse(s).unwrap().as_str(), s);
        }
        // 21:9 chi ton tai o _RATIO_JA cua ban Python, khong phai gia tri canh hop le
        let err = Ratio::parse("21:9").unwrap_err();
        assert_eq!(
            err.to_string(),
            "ratio must be one of 9:16, 16:9, 1:1, 4:3, 3:4 (got '21:9')"
        );
        assert_eq!(Ratio::default(), Ratio::R9x16);
    }

    #[test]
    fn ratio_tieng_nhat_cho_prompt_pro() {
        assert_eq!(Ratio::R9x16.japanese(), "縦動画（9:16）");
        assert_eq!(Ratio::R3x4.japanese(), "縦動画（3:4）");
        assert_eq!(Ratio::R16x9.japanese(), "横動画（16:9）");
        assert_eq!(Ratio::R4x3.japanese(), "横動画（4:3）");
        assert_eq!(Ratio::R1x1.japanese(), "正方形動画（1:1）");
    }

    #[test]
    fn model_alias_day_du() {
        for s in [
            "2.0",
            "2",
            "seedance-2.0",
            "seedance_2.0",
            "seedance_v2.0",
            "seedance-v2.0",
        ] {
            assert_eq!(Model::parse(s).unwrap(), Model::Seedance20, "{s}");
        }
        for s in [
            "2.5",
            "seedance-2.5",
            "seedance_2.5",
            "seedance_v2.5",
            "seedance-v2.5",
        ] {
            assert_eq!(Model::parse(s).unwrap(), Model::Seedance25, "{s}");
        }
        assert_eq!(Model::Seedance20.as_str(), "seedance-2.0");
        assert_eq!(Model::Seedance20.wire_str(), "seedance_v2.0");
        assert_eq!(Model::Seedance25.as_str(), "seedance-2.5");
        assert_eq!(Model::Seedance25.wire_str(), "seedance_v2.5");
        assert_eq!(
            Model::parse("3.0").unwrap_err().to_string(),
            "model must be 2.0 or 2.5 (got '3.0')"
        );
    }

    #[test]
    fn model_1_0_alias_va_wire() {
        for s in [
            "1.0",
            "1",
            "seedance-1.0",
            "seedance_1.0",
            "seedance_v1.0",
            "seedance-v1.0",
        ] {
            assert_eq!(Model::parse(s).unwrap(), Model::Seedance10, "{s}");
        }
        assert_eq!(Model::Seedance10.as_str(), "seedance-1.0");
        assert_eq!(Model::Seedance10.wire_str(), "seedance_v1.0");
    }

    #[test]
    fn thong_diep_model_van_ghi_2_0_hoac_2_5_nhu_python_quen_cap_nhat() {
        // Python 8ec1cfa them 1.0 nhung KHONG doi thong diep — giu nguyen de khop.
        assert_eq!(
            Model::parse("3.0").unwrap_err().to_string(),
            "model must be 2.0 or 2.5 (got '3.0')"
        );
    }

    #[test]
    fn logo_auto_va_thong_diep_van_ba_gia_tri() {
        assert_eq!(LogoMode::parse("auto").unwrap(), LogoMode::Auto);
        assert_eq!(LogoMode::Auto.as_str(), "auto");
        // Python 8ec1cfa them auto nhung thong diep van "off, crop or delogo".
        assert_eq!(
            LogoMode::parse("blur").unwrap_err().to_string(),
            "logo must be off, crop or delogo (got 'blur')"
        );
    }

    #[test]
    fn logo_mode_ba_gia_tri_rong_la_off() {
        assert_eq!(LogoMode::parse("").unwrap(), LogoMode::Off);
        assert_eq!(LogoMode::parse("  ").unwrap(), LogoMode::Off);
        assert_eq!(LogoMode::parse("CROP").unwrap(), LogoMode::Crop);
        assert_eq!(LogoMode::parse("delogo").unwrap(), LogoMode::Delogo);
        assert_eq!(
            LogoMode::parse("blur").unwrap_err().to_string(),
            "logo must be off, crop or delogo (got 'blur')"
        );
        // Ban Python bao loi voi gia tri GOC, khong lowercase
        assert_eq!(
            LogoMode::parse("BLUR").unwrap_err().to_string(),
            "logo must be off, crop or delogo (got 'BLUR')"
        );
    }
}
