//! `lvc-vault` — mã hoá bí mật tài khoản AI trước khi ghi DB (P4c, ADR-R017).
//!
//! LUẬT (xem `CLAUDE.md`): đây là nơi DUY NHẤT chạm plaintext bí mật (cookie, API key, mật khẩu,
//! TOTP). Không log plaintext; không để plaintext ra IPC — UI chỉ nhận bản đã che ([`mask`]).
//!
//! [`SecretBox`] là trait để test không cần Windows ([`PlainBox`]); bản thật trên Windows dùng
//! [`DpapiBox`] (Windows DPAPI, phạm vi user). Chọn hộp mặc định qua [`default_box`].

pub mod mask;
pub mod vault;
pub mod verify;

pub use vault::{AddCredential, CredentialView, Vault, VaultRepoError};
pub use verify::{VerifyOutcome, verify_api_key};

use std::sync::Arc;

/// Nhãn ứng dụng dùng làm "optional entropy" cho DPAPI — buộc ciphertext gắn với app này.
pub const ENTROPY: &[u8] = b"lvc-seedance-vault-v1";

/// Lỗi mã hoá/giải mã bí mật. Thông điệp KHÔNG chứa plaintext.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VaultError {
    #[error("giai ma bi mat that bai (sai may/sai user Windows, hoac du lieu hong)")]
    Decrypt,
    #[error("ma hoa bi mat that bai")]
    Encrypt,
    #[error("kho bi mat: {0}")]
    Other(String),
}

/// Hộp mã hoá bí mật: `encrypt` khi ghi, `decrypt` khi đọc. `Send + Sync` để dùng chung qua `Arc`.
pub trait SecretBox: Send + Sync {
    /// Tên hộp (chẩn đoán/log — KHÔNG bí mật): `"dpapi"` | `"plain"`.
    fn kind(&self) -> &'static str;

    /// # Errors
    /// [`VaultError::Encrypt`] khi hệ điều hành từ chối mã hoá.
    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, VaultError>;

    /// # Errors
    /// [`VaultError::Decrypt`] khi ciphertext hỏng / khác máy / khác user Windows.
    fn decrypt(&self, cipher: &[u8]) -> Result<Vec<u8>, VaultError>;
}

/// Hộp mặc định cho bản chạy thật: DPAPI trên Windows.
///
/// # Panics
/// Trên hệ KHÔNG phải Windows (chỉ xảy ra khi build sai mục tiêu) — bản release luôn Windows.
#[must_use]
pub fn default_box() -> Arc<dyn SecretBox> {
    #[cfg(windows)]
    {
        Arc::new(dpapi::DpapiBox)
    }
    #[cfg(not(windows))]
    {
        panic!("lvc-vault: ban that chi ho tro Windows (DPAPI); test dung PlainBox")
    }
}

/// Hộp KHÔNG mã hoá — CHỈ cho test/dev trên hệ không phải Windows. Không bao giờ dùng ở bản release.
#[derive(Debug, Default, Clone, Copy)]
pub struct PlainBox;

impl SecretBox for PlainBox {
    fn kind(&self) -> &'static str {
        "plain"
    }

    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, VaultError> {
        // Bọc nhẹ để không nhầm là DPAPI blob; KHÔNG phải bảo mật.
        let mut out = Vec::with_capacity(plain.len() + 4);
        out.extend_from_slice(b"PLN\0");
        out.extend_from_slice(plain);
        Ok(out)
    }

    fn decrypt(&self, cipher: &[u8]) -> Result<Vec<u8>, VaultError> {
        cipher
            .strip_prefix(b"PLN\0")
            .map(<[u8]>::to_vec)
            .ok_or(VaultError::Decrypt)
    }
}

#[cfg(windows)]
pub mod dpapi;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plainbox_round_trip() {
        let b = PlainBox;
        let c = b.encrypt(b"sessionid=abc").unwrap();
        assert_ne!(c, b"sessionid=abc");
        assert_eq!(b.decrypt(&c).unwrap(), b"sessionid=abc");
        assert!(b.decrypt(b"khong-phai-blob").is_err());
        assert_eq!(b.kind(), "plain");
    }
}
