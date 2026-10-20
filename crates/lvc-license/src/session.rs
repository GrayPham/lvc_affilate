//! Session cache mã hoá (Fernet, khoá suy từ `device_id`) để auto-login OFFLINE tới khi hết hạn.
//! `load_session` trả `None` nếu thiếu/hỏng/hết hạn → caller về màn đăng nhập. KHÔNG log plaintext.

use std::path::Path;

use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::crypto::{fernet_decrypt, fernet_encrypt};
use crate::verify::{LicenseInfo, parse_iso_to_unix};

/// Phiên đã kích hoạt, cache cục bộ để auto-login offline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub username: String,
    pub license: LicenseInfo,
    pub device_id: String,
    pub saved_at: i64,
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

/// Đọc session: giải mã bằng khoá máy hiện tại; `None` nếu thiếu/hỏng/sai máy/hết hạn
/// (`now_unix >= expires_at`).
#[must_use]
pub fn load_session(path: &Path, salt: &str, now_unix: i64) -> Option<Session> {
    let key = session_key(&crate::device::device_id(), salt);
    let token = std::fs::read_to_string(path).ok()?;
    let plain = fernet_decrypt(&key, &token)?;
    let s: Session = serde_json::from_slice(&plain).ok()?;
    let expires = parse_iso_to_unix(&s.license.expires_at)?;
    if now_unix >= expires {
        return None;
    }
    Some(s)
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
            saved_at: 100,
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
}
