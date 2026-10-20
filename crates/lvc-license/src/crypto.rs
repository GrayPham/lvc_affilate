//! Mã hoá payload gửi server + session: Fernet TỰ VIẾT (AES-128-CBC + HMAC-SHA256) để tránh crate
//! `fernet` (kéo OpenSSL — repo rustls-only), và `encrypt_data` (ECDH P-256 → HKDF → Fernet) khớp
//! `license_client._encrypt_data` của Veo3. KHÔNG log khoá/plaintext.

use aes::Aes128;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use hmac::{Hmac, Mac};
use p256::pkcs8::{DecodePublicKey, EncodePublicKey};
use rand::RngCore;
use rand::rngs::OsRng;
use serde_json::Value;
use sha2::Sha256;

type Enc = cbc::Encryptor<Aes128>;
type Dec = cbc::Decryptor<Aes128>;
type HmacSha256 = Hmac<Sha256>;

/// Lỗi mã hoá payload. KHÔNG kèm giá trị bí mật.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("khoa cong khai server khong hop le")]
    BadPubkey,
    #[error("loi dan khoa HKDF")]
    Hkdf,
    #[error("loi serialize payload")]
    Json,
    #[error("loi ma hoa DER khoa ephemeral")]
    Der,
}

/// Fernet-encrypt (chuẩn Fernet, cố định): `urlsafe_b64( 0x80 ++ ts(8B BE) ++ iv(16B) ++
/// AES128CBC(key32[16..], iv, pkcs7(plaintext)) ++ HMAC_SHA256(key32[..16], phần_trước) )`.
/// `key32[..16]` = signing key, `key32[16..]` = AES key. `now_unix`/`iv` tiêm vào để test tất định.
///
/// # Panics
/// Không bao giờ trong thực tế: độ dài khoá AES/IV cố định 16B, khoá HMAC nhận mọi độ dài.
#[must_use]
pub fn fernet_encrypt(key32: &[u8; 32], plaintext: &[u8], now_unix: i64, iv: [u8; 16]) -> String {
    let signing_key = &key32[..16];
    let enc_key = &key32[16..];
    let ct = Enc::new_from_slices(enc_key, &iv)
        .expect("16-byte key/iv")
        .encrypt_padded_vec_mut::<Pkcs7>(plaintext);
    let mut msg = Vec::with_capacity(1 + 8 + 16 + ct.len() + 32);
    msg.push(0x80);
    msg.extend_from_slice(&now_unix.to_be_bytes());
    msg.extend_from_slice(&iv);
    msg.extend_from_slice(&ct);
    let mut mac = <HmacSha256 as Mac>::new_from_slice(signing_key).expect("hmac key any len");
    mac.update(&msg);
    msg.extend_from_slice(&mac.finalize().into_bytes());
    URL_SAFE.encode(msg)
}

/// Giải Fernet (kiểm HMAC trước, bỏ qua TTL — client tự quản hạn qua session). `None` nếu token
/// hỏng/HMAC sai/padding sai.
#[must_use]
pub fn fernet_decrypt(key32: &[u8; 32], token: &str) -> Option<Vec<u8>> {
    let signing_key = &key32[..16];
    let enc_key = &key32[16..];
    let data = URL_SAFE.decode(token.trim()).ok()?;
    if data.len() < 1 + 8 + 16 + 32 || data[0] != 0x80 {
        return None;
    }
    let (msg, tag) = data.split_at(data.len() - 32);
    let mut mac = <HmacSha256 as Mac>::new_from_slice(signing_key).ok()?;
    mac.update(msg);
    mac.verify_slice(tag).ok()?;
    let iv = &msg[9..25];
    let ct = &msg[25..];
    Dec::new_from_slices(enc_key, iv)
        .ok()?
        .decrypt_padded_vec_mut::<Pkcs7>(ct)
        .ok()
}

/// Mã hoá `data` cho server (khớp `license_client._encrypt_data`): ephemeral P-256 → ECDH với
/// `server_pub_pem` → `HKDF-SHA256(salt=None, info=b"license verification", 32)` → Fernet →
/// `base64( ephemeral_pub_DER(SPKI) ++ token )`. `serde_json::to_vec` THƯỜNG (server tự `loads`, KHÔNG
/// cần canonical).
///
/// # Errors
/// [`CryptoError`] khi PEM server sai / HKDF / serialize / DER lỗi.
pub fn encrypt_data(server_pub_pem: &str, data: &Value) -> Result<String, CryptoError> {
    let server_pub =
        p256::PublicKey::from_public_key_pem(server_pub_pem).map_err(|_| CryptoError::BadPubkey)?;
    let eph = p256::ecdh::EphemeralSecret::random(&mut OsRng);
    let eph_pub_der = eph
        .public_key()
        .to_public_key_der()
        .map_err(|_| CryptoError::Der)?;
    let shared = eph.diffie_hellman(&server_pub);
    let mut key = [0u8; 32];
    hkdf::Hkdf::<Sha256>::new(None, shared.raw_secret_bytes().as_slice())
        .expand(b"license verification", &mut key)
        .map_err(|_| CryptoError::Hkdf)?;
    let plaintext = serde_json::to_vec(data).map_err(|_| CryptoError::Json)?;
    let mut iv = [0u8; 16];
    OsRng.fill_bytes(&mut iv);
    let token = fernet_encrypt(&key, &plaintext, chrono::Utc::now().timestamp(), iv);
    let mut combined = eph_pub_der.as_bytes().to_vec();
    combined.extend_from_slice(token.as_bytes());
    Ok(STANDARD.encode(combined))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::pkcs8::{DecodePublicKey, EncodePublicKey};
    use serde_json::json;

    #[test]
    fn fernet_round_trip() {
        let key = [7u8; 32];
        let token = fernet_encrypt(&key, b"hello muse", 0, [0u8; 16]);
        assert_eq!(fernet_decrypt(&key, &token).as_deref(), Some(&b"hello muse"[..]));
    }

    #[test]
    fn fernet_sai_key_tra_none() {
        let token = fernet_encrypt(&[1u8; 32], b"x", 0, [0u8; 16]);
        assert!(fernet_decrypt(&[2u8; 32], &token).is_none());
    }

    #[test]
    fn encrypt_data_server_doc_lai() {
        // Server keypair (test đóng vai server để giải mã payload client gửi).
        let server_sk = p256::SecretKey::random(&mut rand::rngs::OsRng);
        let server_pub_pem = server_sk
            .public_key()
            .to_public_key_pem(p256::pkcs8::LineEnding::LF)
            .unwrap();
        let data = json!({"license_key":"K","nonce":"abc","n":123});

        let payload_b64 = encrypt_data(&server_pub_pem, &data).unwrap();

        // --- vai SERVER: base64-decode -> tách DER(91B) ephemeral pub + token Fernet ---
        let combined = base64::engine::general_purpose::STANDARD
            .decode(&payload_b64)
            .unwrap();
        let (der, token) = combined.split_at(91);
        let eph_pub = p256::PublicKey::from_public_key_der(der).unwrap();
        // ECDH phía server + HKDF cùng info -> cùng khoá Fernet.
        let shared = p256::ecdh::diffie_hellman(
            server_sk.to_nonzero_scalar(),
            eph_pub.as_affine(),
        );
        let mut key = [0u8; 32];
        hkdf::Hkdf::<sha2::Sha256>::new(None, shared.raw_secret_bytes())
            .expand(b"license verification", &mut key)
            .unwrap();
        let plain = fernet_decrypt(&key, std::str::from_utf8(token).unwrap()).unwrap();
        let got: serde_json::Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(got, data);
    }
}
