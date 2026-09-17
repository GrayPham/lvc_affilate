//! Tien ich nho dung chung trong `dola` (Task 10 tach tu `compose.rs` — ruling #4): dau thoi
//! gian dat ten file theo `time.strftime('%Y%m%d_%H%M%S')` cua Python (`video_worker_ui.py:1958`
//! `mode_fail_<ts>.png`, `:2591` `no_conv_<acc>_<ts>.png`, `video_worker.py:176`
//! `<account>_<ts>.mp4`).
//!
//! Python dung GIO MAY (`time.strftime` = local time); std Rust khong co mui gio va khong them
//! dependency (`chrono`/`time`) chi de dat ten file -> dung UTC (ruling Task 7/10). Ten file chi
//! de phan biet, khong tham gia logic.
//!
//! Fix vong 1 (I-2): them [`cookie_date_unix`] — parser ngay `Expires` cua cookie theo RFC 6265
//! §5.1.1 (cach TRINH DUYET doc: RFC 1123 / RFC 850 / asctime va cac bien the), chi `std`
//! (`httpdate` 1.0.3 co trong `Cargo.lock` nhung CHI qua dev-dep `wiremock` -> `hyper/server`,
//! khong nam trong do thi build thuong -> khong them). Oracle test: `aiohttp.CookieJar._parse_date`
//! (cung thuat toan RFC 6265, khac 2 diem nho ghi o doc ham).

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

/// So ngay ke tu 1970-01-01 cua ngay Gregorian `y-m-d` (Howard Hinnant `days_from_civil`, nghich
/// dao cua [`civil_from_days`]; am cho ngay truoc 1970, hop le tu nam -32767).
pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Giay Unix (i64, am truoc epoch) cua `t`.
pub(crate) fn unix_secs(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
    }
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

/// RFC 6265 §5.1.1 "delimiter": `%x09 / %x20-2F / %x3B-40 / %x5B-60 / %x7B-7E`. Moi ky tu khac
/// (ke ca ngoai ASCII, `%x00-08`, `%x0A-1F`) la non-delimiter — dinh vao token.
fn is_cookie_date_delim(c: char) -> bool {
    matches!(c, '\t' | ' '..='/' | ';'..='@' | '['..='`' | '{'..='~')
}

/// `1*2DIGIT`/`2*4DIGIT` ... `( non-digit *OCTET )`: chuoi chu so DAU token co do dai trong
/// `[min, max]` (dai hon = "theo sau la chu so" -> khong khop). Tra (gia tri, phan con lai).
fn leading_digits(tok: &str, min: usize, max: usize) -> Option<(u32, &str)> {
    let n = tok.bytes().take_while(u8::is_ascii_digit).count();
    if n < min || n > max {
        return None;
    }
    Some((tok[..n].parse().ok()?, &tok[n..]))
}

/// `hms-time = time-field ":" time-field ":" time-field` (moi truong `1*2DIGIT`), theo sau la
/// non-digit hoac het.
fn parse_hms(tok: &str) -> Option<(u32, u32, u32)> {
    let (h, rest) = leading_digits(tok, 1, 2)?;
    let (m, rest) = leading_digits(rest.strip_prefix(':')?, 1, 2)?;
    let (s, _) = leading_digits(rest.strip_prefix(':')?, 1, 2)?;
    Some((h, m, s))
}

/// `month = ( "jan" / ... / "dec" ) *OCTET` — 3 byte dau, khong phan biet hoa/thuong.
fn parse_month(tok: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let head = tok.as_bytes().get(..3)?;
    MONTHS
        .iter()
        .position(|m| m.as_bytes().eq_ignore_ascii_case(head))
        .and_then(|i| u32::try_from(i + 1).ok())
}

/// Ngay `Expires` cua cookie -> giay Unix (UTC), theo thuat toan RFC 6265 §5.1.1 nhu trinh duyet:
/// tach token bang delimiter, moi token thu (chi MOT nhanh, `else if`) time `hh:mm:ss` -> day-of-month
/// `1*2DIGIT` -> month (3 chu cai dau) -> year `2*4DIGIT`; nam 70..=99 -> +1900, 0..=69 -> +2000;
/// thieu bat ky phan nao, ngay ngoai 1..=31, nam < 1601, gio > 23, phut/giay > 59 -> `None`; ngay
/// khong ton tai (30/02, 29/02 nam thuong) -> `None` (buoc 6 "no such date"). Doc duoc ca 3 dinh
/// dang HTTP (RFC 1123 `Sun, 06 Nov 1994 08:49:37 GMT`, RFC 850 `Sunday, 06-Nov-94 08:49:37 GMT`,
/// asctime `Sun Nov  6 08:49:37 1994`) lan bien the Netscape `21-Oct-2015`; token thua bo qua.
///
/// So voi oracle `aiohttp.CookieJar._parse_date` (cung RFC 6265) lech 2 diem, deu theo RFC:
/// (1) aiohttp dung regex `.match` KHONG neo cuoi nen token `1994` khop day-of-month = 19 (RFC:
/// phai theo sau la non-digit) -> `1994 Nov 6 8:49:37` aiohttp `None`, o day hop le; (2) aiohttp
/// `calendar.timegm` cuon ngay khong ton tai (30/02 -> 02/03), o day `None`.
pub(crate) fn cookie_date_unix(text: &str) -> Option<i64> {
    let mut time: Option<(u32, u32, u32)> = None;
    let mut dom: Option<u32> = None;
    let mut month: Option<u32> = None;
    let mut year: Option<u32> = None;
    for tok in text.split(is_cookie_date_delim).filter(|t| !t.is_empty()) {
        if time.is_none() {
            if let Some(t) = parse_hms(tok) {
                time = Some(t);
                continue;
            }
        }
        if dom.is_none() {
            if let Some((d, _)) = leading_digits(tok, 1, 2) {
                dom = Some(d);
                continue;
            }
        }
        if month.is_none() {
            if let Some(m) = parse_month(tok) {
                month = Some(m);
                continue;
            }
        }
        if year.is_none() {
            if let Some((y, _)) = leading_digits(tok, 2, 4) {
                year = Some(y);
            }
        }
    }
    let ((hour, minute, second), day, month, year) = (time?, dom?, month?, year?);
    let year = i64::from(match year {
        70..=99 => year + 1900,
        0..=69 => year + 2000,
        other => other,
    });
    if !(1..=31).contains(&day) || year < 1601 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    if day > days_in_month(year, month) {
        return None;
    }
    let secs = days_from_civil(year, month, day) * 86_400
        + i64::from(hour) * 3600
        + i64::from(minute) * 60
        + i64::from(second);
    Some(secs)
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

    /// `days_from_civil` nghich dao `civil_from_days` (Hinnant) — ke ca truoc 1970 (am).
    #[test]
    fn days_from_civil_nghich_dao_va_am() {
        for days in [0u64, 1, 59, 11016, 19723, 20714, 30000] {
            let (y, m, d) = civil_from_days(days);
            let y = i64::try_from(y).unwrap();
            let m = u32::try_from(m).unwrap();
            let d = u32::try_from(d).unwrap();
            assert_eq!(
                days_from_civil(y, m, d),
                i64::try_from(days).unwrap(),
                "{y}-{m}-{d}"
            );
        }
        assert_eq!(days_from_civil(1969, 12, 31), -1);
        assert_eq!(days_from_civil(1601, 1, 1), -134_774);
    }

    /// Oracle `aiohttp.CookieJar._parse_date` 3.14.3 (`.venv` Python, chi doc) cho cac vector hop
    /// le: RFC 1123, RFC 850 (nam 2 chu so), asctime, dang Netscape `dd-Mon-yyyy`, epoch 0, am.
    #[test]
    fn cookie_date_unix_ba_dinh_dang_theo_oracle_aiohttp() {
        let cases: [(&str, Option<i64>); 20] = [
            ("Sun, 06 Nov 1994 08:49:37 GMT", Some(784_111_777)),
            ("Sunday, 06-Nov-94 08:49:37 GMT", Some(784_111_777)),
            ("Sun Nov  6 08:49:37 1994", Some(784_111_777)),
            ("06 Nov 1994 08:49:37", Some(784_111_777)),
            ("Sun, 06 Nov 1994 08:49:37 GMT extra 12", Some(784_111_777)),
            ("Sun, 06 Nov 1994 08:49:37.123 GMT", Some(784_111_777)),
            ("Wed, 21 Oct 2015 07:28:00 GMT", Some(1_445_412_480)),
            ("Wed, 21-Oct-2015 07:28:00 GMT", Some(1_445_412_480)),
            ("06-Nov-24 08:49:37 GMT", Some(1_730_882_977)),
            ("Thu, 01 Jan 1970 00:00:00 GMT", Some(0)),
            ("Thu, 01-Jan-70 00:00:00 GMT", Some(0)),
            ("Thu, 31 Dec 1969 23:59:59 GMT", Some(-1)),
            ("Thu, 01-Jan-69 00:00:00 GMT", Some(3_124_224_000)),
            ("Mon, 01 Jan 0069 00:00:00 GMT", Some(3_124_224_000)),
            ("Fri, 18 Sep 2026 01:02:03 GMT", Some(1_789_693_323)),
            ("Mon, 01 Jan 2035 00:00:00 GMT", Some(2_051_222_400)),
            ("Sat, 01 Jan 2000 00:00:00 GMT", Some(946_684_800)),
            ("garbage", None),
            ("", None),
            ("06-Nov-1994 08:49", None),
        ];
        for (s, want) in cases {
            assert_eq!(cookie_date_unix(s), want, "{s:?}");
        }
    }

    /// Bien: RFC 6265 §5.1.1 buoc 5 (gio > 23, nam < 1601, ngay ngoai 1..=31) -> None (oracle
    /// aiohttp cung None). Hai diem LECH aiohttp (co chu y — theo RFC/trinh duyet, xem doc ham):
    /// token "1994" KHONG phai day-of-month (aiohttp regex khong neo -> lay "19" -> None);
    /// 30/02 la ngay khong ton tai -> None (aiohttp `timegm` cuon sang 02/03).
    #[test]
    fn cookie_date_unix_bien_va_lech_aiohttp() {
        assert_eq!(cookie_date_unix("Sun, 06 Nov 1994 24:00:00 GMT"), None);
        assert_eq!(cookie_date_unix("Sun, 06 Nov 1600 08:49:37 GMT"), None);
        assert_eq!(cookie_date_unix("Sun, 32 Nov 1994 08:49:37 GMT"), None);
        assert_eq!(cookie_date_unix("Sun, 00 Nov 1994 08:49:37 GMT"), None);
        assert_eq!(cookie_date_unix("1994 Nov 6 8:49:37"), Some(784_111_777));
        assert_eq!(cookie_date_unix("Tue, 30 Feb 2021 00:00:00 GMT"), None);
        assert_eq!(cookie_date_unix("Mon, 29 Feb 2021 00:00:00 GMT"), None);
        assert_eq!(
            cookie_date_unix("Sat, 29 Feb 2020 00:00:00 GMT"),
            Some(1_582_934_400)
        );
        // Ky tu ngoai ASCII la NON-delimiter (dinh vao token) — khong panic, khong khop.
        assert_eq!(cookie_date_unix("Thứ, 06 Tháng 1994 08:49:37"), None);
    }

    #[test]
    fn unix_secs_truoc_va_sau_epoch() {
        assert_eq!(unix_secs(UNIX_EPOCH), 0);
        assert_eq!(
            unix_secs(UNIX_EPOCH + Duration::from_secs(1_789_693_323)),
            1_789_693_323
        );
        assert_eq!(unix_secs(UNIX_EPOCH - Duration::from_secs(5)), -5);
    }
}
