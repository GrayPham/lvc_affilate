//! Vân tay máy: `device_id` = SHA-256 của `machine_uuid|hostname|processor|os` (khớp
//! `license_client._generate_device_id`). Chỉ cần ỔN ĐỊNH qua reboot (license bind lần kích hoạt
//! đầu). Windows lấy `MachineGuid` từ registry. KHÔNG log.

use sha2::{Digest, Sha256};

/// `MachineGuid` (HKLM\\SOFTWARE\\Microsoft\\Cryptography) — nguồn ổn định nhất trên Windows.
#[cfg(windows)]
fn machine_uuid() -> String {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Cryptography")
        .and_then(|k| k.get_value::<String, _>("MachineGuid"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(not(windows))]
fn machine_uuid() -> String {
    "unknown".to_string()
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Tên máy (hostname).
#[must_use]
pub fn device_name() -> String {
    if cfg!(windows) {
        env_or("COMPUTERNAME", "unknown")
    } else {
        env_or("HOSTNAME", "unknown")
    }
}

fn processor() -> String {
    env_or("PROCESSOR_IDENTIFIER", "unknown")
}

fn os_name() -> String {
    if cfg!(windows) {
        "Windows".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}

/// Vân tay máy ổn định: `sha256_hex( machine_uuid | hostname | processor | os )`.
#[must_use]
pub fn device_id() -> String {
    let joined = format!(
        "{}|{}|{}|{}",
        machine_uuid(),
        device_name(),
        processor(),
        os_name()
    );
    let digest = Sha256::digest(joined.as_bytes());
    let mut hex = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// Thông tin máy (best-effort) gửi kèm trong payload mã hoá — KHÔNG nằm trong lớp guard nào.
#[must_use]
pub fn device_info() -> serde_json::Value {
    serde_json::json!({
        "os": os_name(),
        "os_version": "",
        "machine": env_or("PROCESSOR_ARCHITECTURE", "unknown"),
        "processor": processor(),
        "hostname": device_name(),
        "tool_version": env!("CARGO_PKG_VERSION"),
        "cpu_id": processor(),
        "machine_id": machine_uuid(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_hex_64_on_dinh() {
        let a = device_id();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a, device_id(), "device_id phai on dinh trong cung may");
    }

    #[test]
    fn device_info_co_field_bat_buoc() {
        let info = device_info();
        for k in ["os", "hostname", "processor"] {
            assert!(info.get(k).is_some(), "thieu field {k}");
        }
    }
}
