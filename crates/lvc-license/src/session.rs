//! Session cache mã hoá (Fernet, khoá suy từ `device_id`) để auto-login OFFLINE tới khi hết hạn.
//! `load_session` trả `None` nếu thiếu/hỏng/hết hạn → caller về màn đăng nhập. KHÔNG log plaintext.

use std::path::Path;

use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::crypto::{fernet_decrypt, fernet_encrypt};
use crate::verify::{LicenseInfo, parse_iso_to_unix};

/// Số giây tối đa một session được tin MÀ KHÔNG gặp máy chủ lần nào (tính từ `saved_at`, mốc kích
/// hoạt online gần nhất). Hết hạn thì `load_session` trả `None` → người dùng đăng nhập + kích hoạt
/// lại (online) và mốc được làm mới. Chặn kiểu "kích hoạt một lần rồi tách mạng dùng mãi" và giới
/// hạn thời gian một license đã bị thu hồi ở server vẫn chạy được trên máy này.
pub const MAX_OFFLINE_SECS: i64 = 7 * 24 * 3600;

/// Độ lệch đồng hồ cho phép khi so với mốc đã thấy (`last_seen`). Vượt quá = nghi lùi đồng hồ để
/// kéo dài hạn. Rộng 1 giờ để không phạt người chỉnh đồng hồ lệch nhẹ / NTP đồng bộ lại.
pub const CLOCK_ROLLBACK_TOLERANCE_SECS: i64 = 3600;

/// Phiên đã kích hoạt, cache cục bộ để auto-login offline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub username: String,
    pub license: LicenseInfo,
    pub device_id: String,
    /// Mốc kích hoạt ONLINE gần nhất (epoch giây UTC).
    pub saved_at: i64,
    /// Mốc đồng hồ lớn nhất từng thấy khi mở app (epoch giây UTC). `serde(default)` để file session
    /// cũ (chưa có trường này) vẫn đọc được: `0` = chưa có mốc, không coi là lùi đồng hồ.
    #[serde(default)]
    pub last_seen: i64,
}

/// Session còn đáng tin về MẶT THỜI GIAN không: không bị ghi từ "tương lai", không offline quá
/// [`MAX_OFFLINE_SECS`], và đồng hồ không lùi quá [`CLOCK_ROLLBACK_TOLERANCE_SECS`] so với mốc đã thấy.
/// THUẦN (không I/O) để test tất định.
#[must_use]
pub fn time_trustworthy(s: &Session, now_unix: i64) -> bool {
    if s.saved_at > now_unix + CLOCK_ROLLBACK_TOLERANCE_SECS {
        return false;
    }
    if now_unix - s.saved_at > MAX_OFFLINE_SECS {
        return false;
    }
    s.last_seen == 0 || now_unix + CLOCK_ROLLBACK_TOLERANCE_SECS >= s.last_seen
}

/// Khoá Fernet cho session = `sha256( device_id[..32] ++ salt )`. Khoá suy từ `device_id` của MÁY
/// HIỆN TẠI nên file session chép sang máy khác sẽ KHÔNG giải được.
#[must_use]
pub fn session_key(device_id: &str, salt: &str) -> [u8; 32] {
    let n = device_id.len().min(32);
    let mut h = Sha256::new();
    h.update(&device_id.as_bytes()[..n]);
    h.update(salt.as_bytes());
    h.finalize().into()
}

/// Ghi session (mã hoá Fernet bằng khoá suy từ `device_id` máy hiện tại).
///
/// # Errors
/// [`std::io::Error`] khi serialize hoặc ghi file thất bại.
pub fn save_session(path: &Path, s: &Session, salt: &str) -> std::io::Result<()> {
    let key = session_key(&crate::device::device_id(), salt);
    let plaintext = serde_json::to_vec(s).map_err(std::io::Error::other)?;
    let mut iv = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut iv);
    let token = fernet_encrypt(&key, &plaintext, chrono::Utc::now().timestamp(), iv);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, token)
}

/// Giải mã + parse session, CHƯA kiểm hạn/thời gian. Nội bộ cho `load_session`/`touch_session`.
fn read_session_raw(path: &Path, salt: &str) -> Option<Session> {
    let key = session_key(&crate::device::device_id(), salt);
    let token = std::fs::read_to_string(path).ok()?;
    let plain = fernet_decrypt(&key, &token)?;
    serde_json::from_slice(&plain).ok()
}

/// Đọc session: giải mã bằng khoá máy hiện tại; `None` nếu thiếu/hỏng/sai máy/hết hạn
/// (`now_unix >= expires_at`) hoặc KHÔNG đáng tin về thời gian (xem [`time_trustworthy`]: offline quá
/// lâu, ghi từ tương lai, đồng hồ bị lùi).
#[must_use]
pub fn load_session(path: &Path, salt: &str, now_unix: i64) -> Option<Session> {
    let s = read_session_raw(path, salt)?;
    let expires = parse_iso_to_unix(&s.license.expires_at)?;
    if now_unix >= expires || !time_trustworthy(&s, now_unix) {
        return None;
    }
    Some(s)
}

/// Ghi nhận mốc đồng hồ đã thấy (`last_seen = max(cũ, now)`) để lần sau phát hiện lùi đồng hồ. Chỉ
/// tiến, không bao giờ lùi. Không làm gì nếu session thiếu/hỏng. Best-effort: lỗi ghi bị bỏ qua
/// (mất một lần ghi mốc chỉ làm kiểm tra lần sau lỏng hơn, không chặn người dùng hợp lệ).
pub fn touch_session(path: &Path, salt: &str, now_unix: i64) {
    let Some(mut s) = read_session_raw(path, salt) else {
        return;
    };
    if now_unix <= s.last_seen {
        return;
    }
    s.last_seen = now_unix;
    let _ = save_session(path, &s, salt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::LicenseInfo;

    fn tmp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("lvc_sess_{}.enc", rand::random::<u64>()))
    }

    fn sample(expires_at: &str) -> Session {
        Session {
            username: "u1".into(),
            license: LicenseInfo {
                key: "K".into(),
                expires_at: expires_at.into(),
                tool_id: None,
                max_devices: Some(1),
                device_id: "D".into(),
            },
            device_id: "D".into(),
            // Gần mốc `now` dùng ở các test (1_700_000_000), vì session offline quá lâu bị từ chối.
            saved_at: 1_699_999_900,
            last_seen: 0,
        }
    }

    #[test]
    fn session_round_trip() {
        let p = tmp();
        let s = sample("2999-01-01T00:00:00Z");
        save_session(&p, &s, "salt").unwrap();
        let got = load_session(&p, "salt", 1_700_000_000);
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, Some(s));
    }

    #[test]
    fn session_het_han_tra_none() {
        let p = tmp();
        save_session(&p, &sample("2000-01-01T00:00:00Z"), "salt").unwrap();
        let got = load_session(&p, "salt", 1_700_000_000);
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, None);
    }

    #[test]
    fn session_file_hong_tra_none() {
        let p = tmp();
        std::fs::write(&p, b"rac khong giai duoc").unwrap();
        let got = load_session(&p, "salt", 1_700_000_000);
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, None);
    }

    #[test]
    fn session_sai_salt_tra_none() {
        let p = tmp();
        save_session(&p, &sample("2999-01-01T00:00:00Z"), "salt").unwrap();
        let got = load_session(&p, "salt-khac", 1_700_000_000);
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, None);
    }

    /// Session mẫu đã kích hoạt ở `saved_at`, còn hạn tới 2999.
    fn at(saved_at: i64, last_seen: i64) -> Session {
        let mut s = sample("2999-01-01T00:00:00Z");
        s.saved_at = saved_at;
        s.last_seen = last_seen;
        s
    }

    #[test]
    fn thoi_gian_binh_thuong_duoc_tin() {
        assert!(time_trustworthy(&at(1_000, 1_000), 1_000 + 3_600));
    }

    #[test]
    fn offline_qua_han_toi_da_thi_khong_tin() {
        let saved = 1_000;
        assert!(time_trustworthy(&at(saved, 0), saved + MAX_OFFLINE_SECS));
        assert!(!time_trustworthy(
            &at(saved, 0),
            saved + MAX_OFFLINE_SECS + 1
        ));
    }

    #[test]
    fn lui_dong_ho_qua_dung_sai_thi_khong_tin() {
        let seen = 10_000_000;
        // Lùi đúng bằng dung sai: còn chấp nhận. Lùi thêm 1 giây: từ chối.
        assert!(time_trustworthy(
            &at(9_900_000, seen),
            seen - CLOCK_ROLLBACK_TOLERANCE_SECS
        ));
        assert!(!time_trustworthy(
            &at(9_900_000, seen),
            seen - CLOCK_ROLLBACK_TOLERANCE_SECS - 1
        ));
    }

    #[test]
    fn session_ghi_tu_tuong_lai_thi_khong_tin() {
        let now = 5_000_000;
        assert!(!time_trustworthy(
            &at(now + CLOCK_ROLLBACK_TOLERANCE_SECS + 1, 0),
            now
        ));
    }

    #[test]
    fn session_cu_khong_co_last_seen_van_doc_duoc() {
        // File session cũ không có trường `last_seen` → serde(default) = 0, không bị coi là lùi giờ.
        let mut v = serde_json::to_value(sample("2999-01-01T00:00:00Z")).unwrap();
        v.as_object_mut().unwrap().remove("last_seen");
        let s: Session = serde_json::from_value(v).unwrap();
        assert_eq!(s.last_seen, 0);
        assert!(time_trustworthy(&s, s.saved_at + 10));
    }

    #[test]
    fn load_session_tu_choi_khi_lui_dong_ho() {
        let p = tmp();
        let now = chrono::Utc::now().timestamp();
        save_session(&p, &at(now - 100, now + 86_400), "salt").unwrap();
        let got = load_session(&p, "salt", now);
        let _ = std::fs::remove_file(&p);
        assert_eq!(
            got, None,
            "đồng hồ lùi 1 ngày so với mốc đã thấy phải bị từ chối"
        );
    }

    #[test]
    fn touch_chi_tien_khong_lui() {
        let p = tmp();
        let now = chrono::Utc::now().timestamp();
        save_session(&p, &at(now - 100, now), "salt").unwrap();
        touch_session(&p, "salt", now - 5_000); // cố lùi → bị bỏ qua
        assert_eq!(read_session_raw(&p, "salt").unwrap().last_seen, now);
        touch_session(&p, "salt", now + 50); // tiến → ghi
        let seen = read_session_raw(&p, "salt").unwrap().last_seen;
        let _ = std::fs::remove_file(&p);
        assert_eq!(seen, now + 50);
    }
}
