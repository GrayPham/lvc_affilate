//! Định dạng thời gian UTC cho upload ảnh (không thêm dep `chrono`/`time`): `x-amz-date`
//! (`%Y%m%dT%H%M%SZ`) và header HTTP `Date` (`%a, %d %b %Y %H:%M:%S GMT`) — khớp
//! `datetime.datetime.now(utc).strftime(...)` của `dola_client.py:517-519`.
//!
//! HÀM THUẦN: nhận số giây epoch làm tham số để test tất định (thuật toán civil-from-days Hinnant).

const WD: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MON: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `(năm, tháng 1-12, ngày 1-31, giờ, phút, giây, thứ 0=CN..6=T7)` từ giây epoch UTC.
fn parts(secs: u64) -> (i64, u32, u32, u32, u32, u32, usize) {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let (hh, mm, ss) = (
        u32::try_from(rem / 3600).unwrap_or(0),
        u32::try_from(rem % 3600 / 60).unwrap_or(0),
        u32::try_from(rem % 60).unwrap_or(0),
    );
    // 1970-01-01 là thứ Năm → index (days + 4) mod 7 với 0 = Chủ Nhật.
    let wd = usize::try_from(days.rem_euclid(7) + 4).unwrap_or(4) % 7;
    // civil_from_days (Howard Hinnant): z = days kể từ 0000-03-01.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, hh, mm, ss, wd)
}

/// `x-amz-date` dạng `20231114T221320Z`.
#[must_use]
pub fn amz_date(secs: u64) -> String {
    let (y, m, d, hh, mm, ss, _) = parts(secs);
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

/// Header HTTP `Date` dạng `Tue, 14 Nov 2023 22:13:20 GMT`.
#[must_use]
pub fn http_date(secs: u64) -> String {
    let (y, m, d, hh, mm, ss, wd) = parts(secs);
    let mon = MON[(m as usize - 1).min(11)];
    let wday = WD[wd];
    format!("{wday}, {d:02} {mon} {y:04} {hh:02}:{mm:02}:{ss:02} GMT")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn khop_strftime_python() {
        // 1_700_000_000 = 2023-11-14 22:13:20 UTC (thứ Ba).
        assert_eq!(amz_date(1_700_000_000), "20231114T221320Z");
        assert_eq!(http_date(1_700_000_000), "Tue, 14 Nov 2023 22:13:20 GMT");
        // Mốc epoch 0 = 1970-01-01 00:00:00 UTC (thứ Năm).
        assert_eq!(amz_date(0), "19700101T000000Z");
        assert_eq!(http_date(0), "Thu, 01 Jan 1970 00:00:00 GMT");
    }
}
