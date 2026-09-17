use serde::{Deserialize, Serialize};

use crate::{CoreError, Limits, Model};

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

    /// Đo thật 2026-09-17 (ADR-044, hook đổi `ability_param.duration` trước khi SDK ký,
    /// modes.py:206-213): Nhanh — server NHẬN mọi duration nhưng KẸP theo model: Seedance 2.0
    /// → tối đa 15s (gửi 30 vẫn ra 15.1s), Seedance 2.5 → 30s thật (30.08s, 2 credit, ~5 phút).
    /// Pro/Agent: agent gọi công cụ nội bộ, kẹp 15s kể cả model Seedance 2.5 (gửi 30s ra 15.1s).
    /// => fast: (10, 15) + 30 khi model 2.5; pro-like: (10, 15) bất kể model.
    ///
    /// Thay thế ADR-033/034 (2026-09-16, "30s không mode nào dùng được"): route mở khoá dola30
    /// đã tắt hẳn (`DOLA_UNLOCK_ROUTES=0`), 30s thật giờ đi bằng hook `ability_param.duration`.
    #[must_use]
    pub fn allowed_durations(self, model: Model) -> &'static [u16] {
        if self.is_pro_like() {
            &[10, 15]
        } else if matches!(model, Model::Seedance25) {
            &[10, 15, 30]
        } else {
            &[10, 15]
        }
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

/// modes.py:216-231 — 30s chỉ có MỘT đường: Nhanh (`fast`) + Seedance 2.5; mọi input khác đều
/// tự chuyển. Trả `(mode, model, ghi_chú)`: ghi chú `None` khi không phải đổi gì (đã đúng, hoặc
/// `duration != 30`).
#[must_use]
pub fn route_30s(mode: Mode, duration: u16, model: Model) -> (Mode, Model, Option<String>) {
    if duration != 30 {
        return (mode, model, None);
    }
    let mut changes = Vec::new();
    let new_mode = if mode == Mode::Fast {
        mode
    } else {
        changes.push(format!("{}→fast", mode.as_str()));
        Mode::Fast
    };
    let new_model = if matches!(model, Model::Seedance25) {
        model
    } else {
        changes.push(format!("{}→seedance-2.5", model.as_str()));
        Model::Seedance25
    };
    let note = if changes.is_empty() {
        None
    } else {
        Some(format!(
            "30s chỉ tạo được bằng Nhanh + Seedance 2.5 (agent/pro và 2.0 bị Dola kẹp 15s) \
             → tự chuyển: {}",
            changes.join(", ")
        ))
    };
    (new_mode, new_model, note)
}

/// modes.py:278-285 `video_timeout` — cộng dồn `max()` theo pro-like, thời lượng dài (≥30s), có
/// ảnh tham chiếu (mirror `video_worker_ui.py:2404-2405`: `if reference_image_paths: timeout =
/// max(timeout, config.REFERENCE_VIDEO_TIMEOUT)`, gọi SAU `video_timeout` gốc nên gộp chung ở đây).
#[must_use]
pub fn video_timeout(mode: Mode, duration: u16, has_refs: bool, limits: &Limits) -> u64 {
    let mut t = u64::from(limits.video_timeout_s);
    if mode.is_pro_like() {
        t = t.max(u64::from(limits.pro_video_timeout_s));
    }
    if duration >= 30 {
        t = t.max(u64::from(limits.long_video_timeout_s));
    }
    if has_refs {
        t = t.max(u64::from(limits.reference_video_timeout_s));
    }
    t
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
    fn thoi_luong_cho_phep_theo_model_adr_044() {
        // ADR-044 (2026-09-17), thay ADR-033/034: hook doi ability_param.duration truoc khi ky
        // -> fast + Seedance 2.5 ra duoc 30s that; fast + 2.0 va moi pro-like van kep 15s.
        assert_eq!(
            Mode::Fast.allowed_durations(Model::Seedance25),
            &[10, 15, 30]
        );
        assert_eq!(Mode::Fast.allowed_durations(Model::Seedance20), &[10, 15]);
        assert_eq!(Mode::Fast.allowed_durations(Model::Seedance10), &[10, 15]);
        assert_eq!(Mode::Pro.allowed_durations(Model::Seedance25), &[10, 15]);
        assert_eq!(Mode::ProMax.allowed_durations(Model::Seedance25), &[10, 15]);
        assert_eq!(Mode::Agent.allowed_durations(Model::Seedance25), &[10, 15]);
    }

    #[test]
    fn route_30s_giu_nguyen_khi_khong_phai_30s() {
        let (m, model, note) = route_30s(Mode::Fast, 10, Model::Seedance20);
        assert_eq!((m, model, note), (Mode::Fast, Model::Seedance20, None));
        let (m, model, note) = route_30s(Mode::Agent, 15, Model::Seedance25);
        assert_eq!((m, model, note), (Mode::Agent, Model::Seedance25, None));
    }

    #[test]
    fn route_30s_da_dung_fast_2_5_thi_khong_ghi_chu() {
        assert_eq!(
            route_30s(Mode::Fast, 30, Model::Seedance25),
            (Mode::Fast, Model::Seedance25, None)
        );
    }

    #[test]
    fn route_30s_doi_model_2_0_sang_2_5() {
        let (m, model, note) = route_30s(Mode::Fast, 30, Model::Seedance20);
        assert_eq!(m, Mode::Fast);
        assert_eq!(model, Model::Seedance25);
        assert_eq!(
            note.unwrap(),
            "30s chỉ tạo được bằng Nhanh + Seedance 2.5 (agent/pro và 2.0 bị Dola kẹp 15s) \
             → tự chuyển: seedance-2.0→seedance-2.5"
        );
    }

    #[test]
    fn route_30s_doi_mode_sang_fast_giu_model_2_5() {
        let (m, model, note) = route_30s(Mode::Agent, 30, Model::Seedance25);
        assert_eq!(m, Mode::Fast);
        assert_eq!(model, Model::Seedance25);
        assert_eq!(
            note.unwrap(),
            "30s chỉ tạo được bằng Nhanh + Seedance 2.5 (agent/pro và 2.0 bị Dola kẹp 15s) \
             → tự chuyển: agent→fast"
        );
    }

    #[test]
    fn route_30s_doi_ca_mode_lan_model() {
        let (m, model, note) = route_30s(Mode::Agent, 30, Model::Seedance20);
        assert_eq!(m, Mode::Fast);
        assert_eq!(model, Model::Seedance25);
        assert_eq!(
            note.unwrap(),
            "30s chỉ tạo được bằng Nhanh + Seedance 2.5 (agent/pro và 2.0 bị Dola kẹp 15s) \
             → tự chuyển: agent→fast, seedance-2.0→seedance-2.5"
        );
        // pro_max + seedance-1.0 -> oracle modes.route_30s: doi ca hai, dung dung ten model cu
        let (m2, model2, note2) = route_30s(Mode::ProMax, 30, Model::Seedance10);
        assert_eq!(m2, Mode::Fast);
        assert_eq!(model2, Model::Seedance25);
        assert_eq!(
            note2.unwrap(),
            "30s chỉ tạo được bằng Nhanh + Seedance 2.5 (agent/pro và 2.0 bị Dola kẹp 15s) \
             → tự chuyển: pro_max→fast, seedance-1.0→seedance-2.5"
        );
    }

    #[test]
    fn video_timeout_cong_don_max_theo_python() {
        let l = Limits::default();
        assert_eq!(video_timeout(Mode::Fast, 10, false, &l), 300);
        assert_eq!(video_timeout(Mode::Pro, 10, false, &l), 1800);
        assert_eq!(video_timeout(Mode::Fast, 30, false, &l), 1500);
        assert_eq!(video_timeout(Mode::Fast, 10, true, &l), 900);
        // pro-like (1800) da lon hon reference (900) nen khong doi.
        assert_eq!(video_timeout(Mode::Pro, 10, true, &l), 1800);
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
