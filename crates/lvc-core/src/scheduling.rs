//! Quy tắc lập lịch thuần: nắn thời lượng theo mode, thứ tự đổi mode khi hết lượt, và mốc ngày
//! reset hạn mức Dola (giờ Tokyo — Task 16 `AccountPool`).

use crate::{Limits, Mode, Model};

/// Múi giờ reset hạn mức Dola: `config.py:157 LIMIT_RESET_TZ = "Asia/Tokyo"` — UTC+9 cố định,
/// không DST (`browser_pool.py:433-441 _reset_tz`; nhánh fallback :436-440 cũng là offset cố
/// định 9 giờ). Không nhận cấu hình múi giờ khác (ruling Task 16).
pub const LIMIT_RESET_TZ_OFFSET_S: f64 = 9.0 * 3600.0;

const DAY_S: f64 = 86_400.0;

/// Số ngày (theo giờ Tokyo) kể từ epoch của `now_unix` — làm tròn xuống, âm được (trước 1970).
fn tokyo_days_since_epoch(now_unix: f64) -> f64 {
    ((now_unix + LIMIT_RESET_TZ_OFFSET_S) / DAY_S).floor()
}

/// browser_pool.py:448-457 `_next_limit_reset` / `next_limit_reset`: mốc unix giây của 00:00
/// Tokyo KẾ TIẾP sau `now_unix` (đúng nửa đêm thì trả nửa đêm hôm sau — `date() + 1 day`).
/// Thuần, không đọc đồng hồ: engine truyền `now`.
#[must_use]
pub fn next_limit_reset(now_unix: f64) -> f64 {
    (tokyo_days_since_epoch(now_unix) + 1.0) * DAY_S - LIMIT_RESET_TZ_OFFSET_S
}

/// browser_pool.py:443-446 `_usage_day` (v0.55v): khoá `day` của bảng `usage` = ngày theo giờ
/// Tokyo dạng `YYYY-MM-DD` (`date.isoformat()`), để "lượt hôm nay" khớp với Dola.
#[must_use]
pub fn tokyo_day(now_unix: f64) -> String {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "so ngay ke tu epoch da floor(), luon nam trong i64 voi moi moc thoi gian thuc te"
    )]
    let days = tokyo_days_since_epoch(now_unix) as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Số ngày kể từ 1970-01-01 → (năm, tháng, ngày) lịch Gregorian đón trước (thuật toán
/// `civil_from_days` của Howard Hinnant — không cần crate thời gian, giữ `lvc-core` thuần).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

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

    // Oracle (2026-09-18): than ham browser_pool.py:448-457 `_next_limit_reset` + :443-446
    // `_usage_day` chay voi `timezone(timedelta(hours=9))` — dung nhanh fallback :436-440 cua
    // `_reset_tz` (may kiem khong co tzdata) = UTC+9 co dinh, khong DST.
    #[test]
    fn next_limit_reset_tokyo() {
        // 2026-09-17T14:59:59Z = 23:59:59 Tokyo -> reset ngay 1 giay sau (00:00 Tokyo 18/9)
        assert!((next_limit_reset(1_789_657_199.0) - 1_789_657_200.0).abs() < f64::EPSILON);
        // 2026-09-17T15:00:00Z = dung 00:00 Tokyo 18/9 -> reset la 00:00 Tokyo 19/9 (mot ngay sau)
        assert!((next_limit_reset(1_789_657_200.0) - 1_789_743_600.0).abs() < f64::EPSILON);
        // 0.5 giay sau nua dem Tokyo van thuoc ngay 18/9
        assert!((next_limit_reset(1_789_657_200.5) - 1_789_743_600.0).abs() < f64::EPSILON);
        // giua ngay: 2026-09-17T03:21:07Z = 12:21:07 Tokyo 17/9 -> 00:00 Tokyo 18/9
        assert!((next_limit_reset(1_789_615_267.0) - 1_789_657_200.0).abs() < f64::EPSILON);
        // nam nhuan: 2024-02-28T15:00:00Z = 00:00 Tokyo 29/2 -> 00:00 Tokyo 1/3
        assert!((next_limit_reset(1_709_132_400.0) - 1_709_218_800.0).abs() < f64::EPSILON);
        // epoch: 1970-01-01T00:00Z = 09:00 Tokyo 1/1 -> 00:00 Tokyo 2/1 = 54000
        assert!((next_limit_reset(0.0) - 54_000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tokyo_day_khoa_ngay_theo_gio_nhat() {
        assert_eq!(tokyo_day(1_789_657_199.0), "2026-09-17"); // 23:59:59 Tokyo
        assert_eq!(tokyo_day(1_789_657_200.0), "2026-09-18"); // 00:00:00 Tokyo
        assert_eq!(tokyo_day(1_789_657_200.5), "2026-09-18");
        assert_eq!(tokyo_day(1_789_615_267.0), "2026-09-17");
        assert_eq!(tokyo_day(1_709_132_400.0), "2024-02-29"); // nam nhuan
        assert_eq!(tokyo_day(1_709_218_799.0), "2024-02-29");
        assert_eq!(tokyo_day(978_274_799.9), "2000-12-31");
        assert_eq!(tokyo_day(0.0), "1970-01-01");
        assert_eq!(tokyo_day(1_798_729_200.0), "2027-01-01"); // 2026-12-31T15:00Z
    }

    #[test]
    fn next_limit_reset_tru_mot_ngay_la_moc_reset_gan_nhat() {
        // browser_pool.py:931 `last_reset = self._next_limit_reset() - 86400` — moc reset gan nhat
        // phai <= now va reset ke tiep > now.
        for now in [1_789_657_199.0, 1_789_657_200.0, 1_789_615_267.0, 0.0] {
            let next = next_limit_reset(now);
            assert!(next > now, "{now}");
            assert!(next - 86_400.0 <= now, "{now}");
        }
    }
}
