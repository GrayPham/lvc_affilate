//! Quy tắc lập lịch thuần: nắn thời lượng theo mode và thứ tự đổi mode khi hết lượt.

use crate::{Limits, Mode, Model};

/// Nắn thời lượng cho vừa chế độ/model (modes.py:234-240 `fit_duration_to_mode`): lấy mức lớn
/// nhất còn `<= want`; nếu `want` nhỏ hơn mọi mức thì lấy mức nhỏ nhất. `0` được coi như `10`
/// (Python: `int(duration or 10)`).
#[must_use]
pub fn fit_duration_to_mode(duration: u16, mode: Mode, model: Model) -> u16 {
    let allowed = mode.allowed_durations(model);
    let want = if duration == 0 { 10 } else { duration };
    allowed
        .iter()
        .copied()
        .filter(|&d| d <= want)
        .max()
        .unwrap_or_else(|| allowed.iter().copied().min().unwrap_or(10))
}

/// Thứ tự đổi chế độ khi HẾT LƯỢT: chế độ đang yêu cầu trước (kể cả khi bị tắt), rồi
/// các chế độ khác CÒN BẬT theo ưu tiên giữ chất lượng: agent → pro → `pro_max` → fast.
/// Người dùng: "hết agent thì gọi model khác chứ này phí thế".
#[must_use]
pub fn mode_fallback_chain(mode: Mode, limits: &Limits) -> Vec<Mode> {
    let mut out = vec![mode];
    for m in [Mode::Agent, Mode::Pro, Mode::ProMax, Mode::Fast] {
        if m == mode {
            continue;
        }
        let enabled = match m {
            Mode::Pro => limits.pro_enabled,
            Mode::ProMax => limits.pro_max_enabled,
            Mode::Agent => limits.agent_enabled,
            Mode::Fast => true,
        };
        if enabled {
            out.push(m);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_lay_muc_lon_nhat_con_nho_hon_hoac_bang() {
        // Gia tri doi chieu oracle modes.fit_duration_to_mode (ADR-044, 2026-09-17)
        assert_eq!(fit_duration_to_mode(30, Mode::Fast, Model::Seedance20), 15);
        assert_eq!(fit_duration_to_mode(30, Mode::Fast, Model::Seedance25), 30);
        assert_eq!(fit_duration_to_mode(30, Mode::Pro, Model::Seedance25), 15);
        assert_eq!(fit_duration_to_mode(15, Mode::Fast, Model::Seedance20), 15);
        assert_eq!(fit_duration_to_mode(10, Mode::Agent, Model::Seedance20), 10);
        assert_eq!(fit_duration_to_mode(30, Mode::Agent, Model::Seedance25), 15);
        assert_eq!(
            fit_duration_to_mode(30, Mode::ProMax, Model::Seedance25),
            15
        );
    }

    #[test]
    fn fit_nho_hon_moi_muc_thi_lay_muc_nho_nhat() {
        assert_eq!(fit_duration_to_mode(5, Mode::Agent, Model::default()), 10);
        assert_eq!(fit_duration_to_mode(0, Mode::Fast, Model::Seedance20), 10);
        assert_eq!(fit_duration_to_mode(0, Mode::Fast, Model::default()), 10);
    }

    #[test]
    fn chain_uu_tien_chat_luong_roi_moi_xuong_fast() {
        let l = Limits::default();
        assert_eq!(
            mode_fallback_chain(Mode::Fast, &l),
            vec![Mode::Fast, Mode::Agent, Mode::Pro, Mode::ProMax]
        );
        assert_eq!(
            mode_fallback_chain(Mode::Pro, &l),
            vec![Mode::Pro, Mode::Agent, Mode::ProMax, Mode::Fast]
        );
        assert_eq!(
            mode_fallback_chain(Mode::ProMax, &l),
            vec![Mode::ProMax, Mode::Agent, Mode::Pro, Mode::Fast]
        );
        assert_eq!(
            mode_fallback_chain(Mode::Agent, &l),
            vec![Mode::Agent, Mode::Pro, Mode::ProMax, Mode::Fast]
        );
    }

    #[test]
    fn chain_bo_mode_bi_tat_nhung_giu_mode_dang_yeu_cau() {
        let l = Limits {
            agent_enabled: false,
            pro_max_enabled: false,
            ..Limits::default()
        };
        assert_eq!(
            mode_fallback_chain(Mode::Fast, &l),
            vec![Mode::Fast, Mode::Pro]
        );
        // Mode dang yeu cau LUON dung dau ke ca khi bi tat (Python: out = [mode] truoc vong lap)
        assert_eq!(
            mode_fallback_chain(Mode::Agent, &l),
            vec![Mode::Agent, Mode::Pro, Mode::Fast]
        );
    }
}
