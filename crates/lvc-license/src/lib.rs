//! `lvc-license`: cổng bản quyền LVC Shop (port 1:1 từ LVC Veo3). Đăng nhập tài khoản → kích hoạt
//! license; verify phản hồi server bằng ECDSA P-256 (4 lớp guard); ECDH/HKDF/Fernet cho payload gửi
//! lên; session cache mã hoá để auto-login offline tới khi hết hạn.
//!
//! Chia module theo trách nhiệm (luật CLAUDE.md): [`verify`] (THUẦN: canonical JSON + chữ ký + 4 lớp
//! guard), `crypto` (ECDH/HKDF/Fernet), `device` (`device_id`/info), `client` (HTTP), `session` (cache).
//!
//! LUẬT: KHÔNG log bí mật (password, license key, JWT, session plaintext, Fernet key).

pub mod client;
pub mod crypto;
pub mod device;
pub mod session;
pub mod verify;

/// Server cấp phép LVC Shop (cùng server + API như Veo3, khác `tool_code` + public key).
pub const API_URL: &str = "https://tool.lvcmedia.vn";
/// Mã sản phẩm LVC Shop.
pub const TOOL_CODE: &str = "lvc-shop";
/// Salt khoá Fernet cho file session (local-only; server không quan tâm).
pub const SESSION_SALT: &str = "LvcShopSessionKey2026";
/// Tên file session trong `app_data_dir`.
pub const SESSION_FILE: &str = "license_session.enc";

/// Public key verify chữ ký server (ECDSA P-256 / SPKI) — người dùng cung cấp.
pub const PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEiKkmCNoUo1pI/fILINFp/U8wF5MZ
U6ZyQpmwf5qnDUe44e5lRiKRZIxc6PQVnCmpC+nkGXsHLYc0W++euqS//A==
-----END PUBLIC KEY-----
";

/// Client cấp phép mặc định (hằng số LVC Shop nhúng sẵn).
#[must_use]
pub fn default_client() -> client::LicenseClient {
    client::LicenseClient::new(API_URL, PUBLIC_KEY_PEM, TOOL_CODE)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn public_key_pem_hop_le_encrypt_duoc() {
        // PEM nhúng phải parse được — nếu sai thì không ai kích hoạt được.
        assert!(crate::crypto::encrypt_data(super::PUBLIC_KEY_PEM, &json!({"x":1})).is_ok());
    }
}

