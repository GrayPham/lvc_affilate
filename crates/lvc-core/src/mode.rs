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
    /// Dola Agent: chat tự nhiên, không skill, không `/Create Video` (TUT Bỏ skill).
    Agent,
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
            "agent" | "dola_agent" | "chat" | "bot" => Ok(Self::Agent),
            _ => Err(CoreError::InvalidMode(value.to_string())),
        }
    }

    /// `pro` và `pro_max` cùng dùng nút プロ, cùng nhận link mp4 trong text_block.
    // text_block là văn xuôi trong doc comment do brief quy định, không phải định danh code.
    #[allow(clippy::doc_markdown)]
    #[must_use]
    pub fn is_pro_like(self) -> bool {
        matches!(self, Self::Pro | Self::ProMax | Self::Agent)
    }

    /// 10s mọi mode. 15s chỉ pro-like (qua prompt). **30s không mode nào dùng được nữa**:
    /// cơ chế 30s dựa vào extension dola30 đã tắt (ADR-031); đo thật 2026-09-16
    /// (ADR-033/034): agent 30s → "`conversation_id` not acquired", fast 30s → 8/10 cảnh
    /// fail ngay, lịch sử fast 30s chưa từng completed.
    #[must_use]
    pub fn allowed_durations(self) -> &'static [u16] {
        if self.is_pro_like() { &[10, 15] } else { &[10] }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Pro => "pro",
            Self::ProMax => "pro_max",
            Self::Agent => "agent",
        }
    }

    /// Nhãn nút chế độ trên trang Dola (ja-JP). `pro_max` và `agent` bấm cùng nút với `pro`.
    #[must_use]
    pub fn button_label(self) -> &'static str {
        match self {
            Self::Fast => "高速",
            Self::Pro | Self::ProMax | Self::Agent => "プロ",
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
            Mode::Agent => self.daily_limit_agent.max(1),
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
        for v in ["agent", "dola_agent", "chat", "bot", "AGENT"] {
            assert_eq!(Mode::parse(v).unwrap(), Mode::Agent, "{v}");
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
            "mode must be fast, pro, pro_max or agent (got 'bogus')"
        );
    }

    #[test]
    fn thoi_luong_cho_phep_khong_con_30s() {
        // ADR-033/034 Python: 30s dua vao extension dola30 da tat; do that
        // "lich su toan bo task fast 30s CHUA tung completed lan nao".
        assert_eq!(Mode::Fast.allowed_durations(), &[10]);
        assert_eq!(Mode::Pro.allowed_durations(), &[10, 15]);
        assert_eq!(Mode::ProMax.allowed_durations(), &[10, 15]);
        assert_eq!(Mode::Agent.allowed_durations(), &[10, 15]);
    }

    #[test]
    fn pro_max_va_agent_dung_nut_pro() {
        assert_eq!(Mode::Fast.button_label(), "高速");
        assert_eq!(Mode::Pro.button_label(), "プロ");
        assert_eq!(Mode::ProMax.button_label(), "プロ");
        assert_eq!(Mode::Agent.button_label(), "プロ");
        assert!(!Mode::Fast.is_pro_like());
        assert!(Mode::Pro.is_pro_like());
        assert!(Mode::ProMax.is_pro_like());
        assert!(Mode::Agent.is_pro_like());
    }

    #[test]
    fn as_str_bon_gia_tri() {
        assert_eq!(Mode::Agent.as_str(), "agent");
        assert_eq!(Mode::parse(Mode::Agent.as_str()).unwrap(), Mode::Agent);
    }

    #[test]
    fn han_muc_ngay_lay_tu_limits() {
        let l = Limits::default();
        assert_eq!(
            l.daily_limit(Mode::Fast),
            30,
            "Python HEAD: DAILY_LIMIT_FAST=30"
        );
        assert_eq!(l.daily_limit(Mode::Pro), 10);
        assert_eq!(l.daily_limit(Mode::ProMax), 10);
        assert_eq!(l.daily_limit(Mode::Agent), 30);
        let zero = Limits {
            daily_limit_agent: 0,
            ..Limits::default()
        };
        assert_eq!(zero.daily_limit(Mode::Agent), 1, "kep san >= 1");
    }
}
