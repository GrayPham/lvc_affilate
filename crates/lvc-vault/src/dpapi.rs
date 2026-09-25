//! Windows DPAPI (phạm vi user) — `CryptProtectData` / `CryptUnprotectData`.
//!
//! Ciphertext gắn với tài khoản Windows hiện tại (chuyển máy/đổi user → giải mã thất bại, đúng ý:
//! bí mật không theo file DB sang máy khác). `ENTROPY` (optional entropy) buộc thêm vào app này.
//!
//! Workspace đặt `unsafe_code = "warn"` (gate `-D warnings`); DPAPI là FFI Win32 nên bắt buộc
//! `unsafe` — cho phép trong CHÍNH module này, mỗi khối `unsafe` có ghi chú SAFETY.
#![allow(unsafe_code)]

use windows_sys::Win32::Foundation::{HLOCAL, LocalFree};
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

use crate::{ENTROPY, SecretBox, VaultError};

/// Hộp DPAPI phạm vi user.
#[derive(Debug, Default, Clone, Copy)]
pub struct DpapiBox;

/// `CRYPT_INTEGER_BLOB` trỏ vào một lát byte (chỉ đọc trong lúc gọi API).
fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).unwrap_or(u32::MAX),
        // API chi doc; con tro khong bi ghi. Cast bo const vi chu ky Win32 nhan *mut.
        pbData: data.as_ptr().cast_mut(),
    }
}

/// Sao chép `out` (do DPAPI cấp phát) ra `Vec` rồi `LocalFree` — an toàn kể cả khi rỗng.
///
/// # Safety
/// Gọi ngay sau khi `CryptProtectData`/`CryptUnprotectData` trả TRUE với `out` hợp lệ.
unsafe fn take_blob(out: &CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let v = if out.pbData.is_null() || out.cbData == 0 {
        Vec::new()
    } else {
        // SAFETY: DPAPI dam bao pbData tro vung cbData byte hop le khi tra TRUE.
        unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec()
    };
    if !out.pbData.is_null() {
        // SAFETY: pbData do LocalAlloc cua DPAPI cap phat -> phai LocalFree.
        unsafe {
            LocalFree(out.pbData as HLOCAL);
        }
    }
    v
}

impl SecretBox for DpapiBox {
    fn kind(&self) -> &'static str {
        "dpapi"
    }

    fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, VaultError> {
        let in_blob = blob(plain);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // SAFETY: cac con tro hop le trong suot loi goi; out duoc take_blob giai phong.
        let ok = unsafe {
            CryptProtectData(
                std::ptr::from_ref(&in_blob),
                std::ptr::null(),
                std::ptr::from_ref(&entropy),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                std::ptr::from_mut(&mut out),
            )
        };
        if ok == 0 {
            return Err(VaultError::Encrypt);
        }
        // SAFETY: goi ngay sau khi API tra TRUE.
        Ok(unsafe { take_blob(&out) })
    }

    fn decrypt(&self, cipher: &[u8]) -> Result<Vec<u8>, VaultError> {
        let in_blob = blob(cipher);
        let entropy = blob(ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // SAFETY: nhu encrypt.
        let ok = unsafe {
            CryptUnprotectData(
                std::ptr::from_ref(&in_blob),
                std::ptr::null_mut(),
                std::ptr::from_ref(&entropy),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                std::ptr::from_mut(&mut out),
            )
        };
        if ok == 0 {
            return Err(VaultError::Decrypt);
        }
        // SAFETY: goi ngay sau khi API tra TRUE.
        Ok(unsafe { take_blob(&out) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpapi_round_trip_va_ciphertext_khac_plaintext() {
        let b = DpapiBox;
        let secret = b"sessionid=deadbeef; sid_guard=x";
        let c = b.encrypt(secret).unwrap();
        assert_ne!(c.as_slice(), secret.as_slice());
        assert!(c.len() > secret.len(), "DPAPI blob co header");
        assert_eq!(b.decrypt(&c).unwrap(), secret);
    }

    #[test]
    fn ciphertext_hong_giai_ma_that_bai() {
        let b = DpapiBox;
        let mut c = b.encrypt(b"api-key-123").unwrap();
        let n = c.len();
        c[n / 2] ^= 0xFF;
        assert_eq!(b.decrypt(&c), Err(VaultError::Decrypt));
    }

    #[test]
    fn rong_round_trip() {
        let b = DpapiBox;
        let c = b.encrypt(b"").unwrap();
        assert_eq!(b.decrypt(&c).unwrap(), Vec::<u8>::new());
    }
}
