//! Tien ich nho dung chung trong `dola` (Task 10 tach tu `compose.rs` — ruling #4): dau thoi
//! gian dat ten file theo `time.strftime('%Y%m%d_%H%M%S')` cua Python (`video_worker_ui.py:1958`
//! `mode_fail_<ts>.png`, `:2591` `no_conv_<acc>_<ts>.png`, `video_worker.py:176`
//! `<account>_<ts>.mp4`).
//!
//! Python dung GIO MAY (`time.strftime` = local time); std Rust khong co mui gio va khong them
//! dependency (`chrono`/`time`) chi de dat ten file -> dung UTC (ruling Task 7/10). Ten file chi
//! de phan biet, khong tham gia logic.

use std::time::{SystemTime, UNIX_EPOCH};

/// `time.strftime('%Y%m%d_%H%M%S')` tai thoi diem hien tai (UTC — xem doc module).
pub(crate) fn timestamp_utc() -> String {
    timestamp_utc_at(SystemTime::now())
}

/// Nhu [`timestamp_utc`] nhung tai thoi diem `t` cho truoc (test dinh dang voi gia tri co dinh).
/// Truoc 1970 -> coi nhu 1970-01-01 00:00:00 (khong panic).
pub(crate) fn timestamp_utc_at(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, mo, d) = civil_from_days(secs / 86_400);
    let rem = secs % 86_400;
    format!(
        "{y:04}{mo:02}{d:02}_{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Ngay Gregorian tu so ngay ke tu 1970-01-01 (thuat toan Howard Hinnant, chi cho ngay >= 1970).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Lich Gregorian cho ten file `mode_fail_<ts>.png` (oracle `datetime`) — chuyen tu
    /// `compose::tests::civil_from_days_va_timestamp` (Task 7).
    #[test]
    fn civil_from_days_va_timestamp() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19723), (2024, 1, 1));
        assert_eq!(civil_from_days(20714), (2026, 9, 18));
        assert_eq!(civil_from_days(11016), (2000, 2, 29));
        let ts = timestamp_utc();
        assert_eq!(ts.len(), 15, "{ts}");
        assert_eq!(ts.as_bytes()[8], b'_', "{ts}");
        assert!(ts.starts_with("20"), "{ts}");
    }

    /// Gia tri co dinh: 2026-09-18 01:02:03 UTC = 1789693323 s (oracle Python
    /// `datetime(2026,9,18,1,2,3,tzinfo=timezone.utc).timestamp()`); truoc epoch -> 1970-01-01.
    #[test]
    fn timestamp_utc_at_gia_tri_co_dinh() {
        let t = UNIX_EPOCH + Duration::from_secs(1_789_693_323);
        assert_eq!(timestamp_utc_at(t), "20260918_010203");
        assert_eq!(timestamp_utc_at(UNIX_EPOCH), "19700101_000000");
        let before = UNIX_EPOCH - Duration::from_secs(5);
        assert_eq!(timestamp_utc_at(before), "19700101_000000");
    }
}
