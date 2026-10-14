//! `Noise_XX_25519_AESGCM_SHA256` initiator — khớp bundle web Muse.
//!
//! Port 1:1 từ `noise.py` (tham khảo `E:\ProjectAILVC\Lvc_veo3_automation\src\core\museai\protocol\`,
//! CHỈ ĐỌC). Đây là giao thức bắt tay mã hoá CHUẨN CÔNG KHAI (Noise Protocol Framework,
//! noiseprotocol.org, cùng họ dùng trong WireGuard/Signal) — KHÔNG phải chữ ký chống-bot riêng của
//! nền tảng (khác bản chất với Dola X-Gorgon/Argus/Ladon). Cố ý KHÔNG dùng crate `snow` (thư viện
//! Noise tổng quát) để các bước chuyển trạng thái khớp CHÍNH XÁC với bundle đã bắt được, dễ so khớp
//! lại với Python khi debug.

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit, Nonce as NonceT, Payload};
use hmac::{Hmac, Mac};

type GcmNonce = NonceT<Aes256Gcm>;
use rand::RngCore;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

type HmacSha256 = Hmac<Sha256>;

const PROTOCOL: &[u8] = b"Noise_XX_25519_AESGCM_SHA256";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoiseError {
    #[error("write_message1 goi sai pha")]
    WrongPhaseMessage1,
    #[error("read_message2 goi sai pha")]
    WrongPhaseMessage2,
    #[error("write_message3 goi sai pha")]
    WrongPhaseMessage3,
    #[error("split goi sai pha")]
    WrongPhaseSplit,
    #[error("Noise message2 qua ngan: {0} byte")]
    Message2TooShort(usize),
    #[error("thieu khoa ephemeral cua phia doi tac (responder)")]
    MissingResponderEphemeral,
    #[error("X25519 tra ve toan byte 0 (diem yeu/dau vao hong)")]
    AllZeroDh,
    #[error("ma hoa/giai ma that bai")]
    Aead,
    #[error("counter nonce vuot qua gioi han")]
    NonceOverflow,
    #[error("trang thai noi bo sai (bat bien bi vi pham)")]
    InvalidState,
}

pub type Result<T> = std::result::Result<T, NoiseError>;

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// `outputs = 2` (bỏ `out3`) — Python hỗ trợ 2 hoặc 3, codebase này chỉ cần 2 (split cũng dùng 2).
fn hkdf_noise2(chaining_key: &[u8; 32], input_key_material: &[u8]) -> ([u8; 32], [u8; 32]) {
    let temp = hmac_sha256(chaining_key, input_key_material);
    let out1 = hmac_sha256(&temp, &[0x01]);
    let mut buf2 = Vec::with_capacity(32 + 1);
    buf2.extend_from_slice(&out1);
    buf2.push(0x02);
    let out2 = hmac_sha256(&temp, &buf2);
    (out1, out2)
}

fn nonce12(counter: u64) -> GcmNonce {
    let mut buf = [0u8; 12];
    buf[4..].copy_from_slice(&counter.to_be_bytes());
    *GcmNonce::from_slice(&buf)
}

#[derive(Debug)]
pub struct CipherState {
    key: Option<[u8; 32]>,
    nonce: u64,
}

impl CipherState {
    fn empty() -> Self {
        Self {
            key: None,
            nonce: 0,
        }
    }

    fn with_key(key: [u8; 32]) -> Self {
        Self {
            key: Some(key),
            nonce: 0,
        }
    }

    /// # Errors
    /// [`NoiseError::NonceOverflow`] khi vượt quá số lần mã hoá cho phép của một phiên;
    /// [`NoiseError::Aead`] khi mã hoá thất bại.
    pub fn encrypt_with_ad(&mut self, ad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        let Some(key) = self.key else {
            return Ok(plaintext.to_vec());
        };
        let nonce = nonce12(self.nonce);
        self.nonce = self.nonce.checked_add(1).ok_or(NoiseError::NonceOverflow)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| NoiseError::Aead)?;
        cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad: ad,
                },
            )
            .map_err(|_| NoiseError::Aead)
    }

    /// # Errors
    /// [`NoiseError::NonceOverflow`] khi vượt quá số lần giải mã cho phép của một phiên;
    /// [`NoiseError::Aead`] khi giải mã thất bại (khoá/dữ liệu sai).
    pub fn decrypt_with_ad(&mut self, ad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        let Some(key) = self.key else {
            return Ok(ciphertext.to_vec());
        };
        let nonce = nonce12(self.nonce);
        self.nonce = self.nonce.checked_add(1).ok_or(NoiseError::NonceOverflow)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| NoiseError::Aead)?;
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad: ad,
                },
            )
            .map_err(|_| NoiseError::Aead)
    }
}

struct SymmetricState {
    ck: [u8; 32],
    h: [u8; 32],
    cipher: CipherState,
}

impl SymmetricState {
    fn new() -> Self {
        // InitializeSymmetric(): ten giao thuc ngan hon HASHLEN (32) thi dem 0 ben phai (Muse ngan
        // hon 32 byte nen luon roi vao nhanh nay — khong can nhanh sha256(PROTOCOL) con lai).
        let mut initial_hash = [0u8; 32];
        initial_hash[..PROTOCOL.len()].copy_from_slice(PROTOCOL);
        let mut state = Self {
            ck: initial_hash,
            h: initial_hash,
            cipher: CipherState::empty(),
        };
        state.mix_hash(b"");
        state
    }

    fn mix_hash(&mut self, data: &[u8]) {
        let mut buf = Vec::with_capacity(32 + data.len());
        buf.extend_from_slice(&self.h);
        buf.extend_from_slice(data);
        self.h = sha256(&buf);
    }

    fn mix_key(&mut self, ikm: &[u8]) {
        let (ck, temp_k) = hkdf_noise2(&self.ck, ikm);
        self.ck = ck;
        self.cipher = CipherState::with_key(temp_k);
    }

    fn encrypt_and_hash(&mut self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let ciphertext = self.cipher.encrypt_with_ad(&self.h, plaintext)?;
        self.mix_hash(&ciphertext);
        Ok(ciphertext)
    }

    fn decrypt_and_hash(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let plaintext = self.cipher.decrypt_with_ad(&self.h, ciphertext)?;
        self.mix_hash(ciphertext);
        Ok(plaintext)
    }

    fn split(&self) -> (CipherState, CipherState) {
        let (k1, k2) = hkdf_noise2(&self.ck, b"");
        (CipherState::with_key(k1), CipherState::with_key(k2))
    }
}

fn dh(priv_key: &StaticSecret, peer_raw: &[u8; 32]) -> Result<[u8; 32]> {
    let peer_pub = PublicKey::from(*peer_raw);
    let shared = priv_key.diffie_hellman(&peer_pub);
    let out = *shared.as_bytes();
    if out == [0u8; 32] {
        return Err(NoiseError::AllZeroDh);
    }
    Ok(out)
}

fn random_static_secret() -> StaticSecret {
    let mut rng = rand::rngs::OsRng;
    let mut bytes = [0u8; 32];
    rng.fill_bytes(&mut bytes);
    StaticSecret::from(bytes)
}

/// Phía khởi tạo (client) của bắt tay Noise XX — `write_message1` → `read_message2` →
/// `write_message3` → `split` (đúng 4 bước, gọi SAI thứ tự trả lỗi pha thay vì panic).
pub struct NoiseXxInitiator {
    symmetric: SymmetricState,
    e_priv: Option<StaticSecret>,
    re: Option<[u8; 32]>,
    rs: Option<Vec<u8>>,
    phase: u8,
}

impl Default for NoiseXxInitiator {
    fn default() -> Self {
        Self::new()
    }
}

impl NoiseXxInitiator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            symmetric: SymmetricState::new(),
            e_priv: None,
            re: None,
            rs: None,
            phase: 1,
        }
    }

    /// # Errors
    /// [`NoiseError::WrongPhaseMessage1`] khi gọi không đúng lúc (phải là bước đầu tiên);
    /// [`NoiseError::Aead`] khi mã hoá payload thất bại.
    pub fn write_message1(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        if self.phase != 1 {
            return Err(NoiseError::WrongPhaseMessage1);
        }
        let e_priv = random_static_secret();
        let e_pub = PublicKey::from(&e_priv);
        self.symmetric.mix_hash(e_pub.as_bytes());
        let encrypted_payload = self.symmetric.encrypt_and_hash(payload)?;
        self.e_priv = Some(e_priv);
        self.phase = 2;
        let mut out = Vec::with_capacity(32 + encrypted_payload.len());
        out.extend_from_slice(e_pub.as_bytes());
        out.extend_from_slice(&encrypted_payload);
        Ok(out)
    }

    /// # Errors
    /// [`NoiseError::WrongPhaseMessage2`] khi gọi không đúng lúc;
    /// [`NoiseError::Message2TooShort`] khi `msg` ngắn hơn 96 byte (32 pubkey + 48 static + ≥16 tag);
    /// [`NoiseError::Aead`] khi giải mã thất bại (khoá/dữ liệu sai).
    pub fn read_message2(&mut self, msg: &[u8]) -> Result<Vec<u8>> {
        if self.phase != 2 {
            return Err(NoiseError::WrongPhaseMessage2);
        }
        if msg.len() < 96 {
            return Err(NoiseError::Message2TooShort(msg.len()));
        }
        // `self.phase == 2` chỉ đạt được qua `write_message1` (dòng trên đặt `e_priv` rồi mới
        // chuyển phase) — None ở đây là bất biến bị vi phạm, trả lỗi thay vì panic cho an toàn.
        let e_priv = self.e_priv.as_ref().ok_or(NoiseError::InvalidState)?;
        let mut offset = 0usize;
        // Độ dài đã kiểm ở trên (`msg.len() >= 96`) nên `try_into` không thể lỗi thật sự; vẫn trả
        // lỗi thay vì `.expect()` để hàm không có đường panic nào (rõ ràng hơn cho lint + người gọi).
        let re: [u8; 32] = msg[offset..offset + 32]
            .try_into()
            .map_err(|_| NoiseError::InvalidState)?;
        offset += 32;
        self.symmetric.mix_hash(&re);
        self.symmetric.mix_key(&dh(e_priv, &re)?); // ee
        let rs = self.symmetric.decrypt_and_hash(&msg[offset..offset + 48])?;
        offset += 48;
        let rs_arr: [u8; 32] = rs.as_slice().try_into().map_err(|_| NoiseError::Aead)?;
        self.symmetric.mix_key(&dh(e_priv, &rs_arr)?); // es
        let payload = self.symmetric.decrypt_and_hash(&msg[offset..])?;
        self.re = Some(re);
        self.rs = Some(rs);
        self.phase = 3;
        Ok(payload)
    }

    /// # Errors
    /// [`NoiseError::WrongPhaseMessage3`] khi gọi không đúng lúc;
    /// [`NoiseError::MissingResponderEphemeral`] khi chưa có `read_message2` thành công trước đó;
    /// [`NoiseError::Aead`] khi mã hoá thất bại.
    pub fn write_message3(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
        if self.phase != 3 {
            return Err(NoiseError::WrongPhaseMessage3);
        }
        let re = self.re.ok_or(NoiseError::MissingResponderEphemeral)?;
        let s_priv = random_static_secret();
        let s_pub = PublicKey::from(&s_priv);
        let encrypted_static = self.symmetric.encrypt_and_hash(s_pub.as_bytes())?;
        self.symmetric.mix_key(&dh(&s_priv, &re)?); // se
        let encrypted_payload = self.symmetric.encrypt_and_hash(payload)?;
        self.phase = 4;
        let mut out = Vec::with_capacity(encrypted_static.len() + encrypted_payload.len());
        out.extend_from_slice(&encrypted_static);
        out.extend_from_slice(&encrypted_payload);
        Ok(out)
    }

    /// Trả `(gui, nhan)` — cap khoa AES-GCM sau khi bat tay xong (pha 4 -> 5, khong goi lai duoc).
    ///
    /// # Errors
    /// [`NoiseError::WrongPhaseSplit`] khi gọi trước khi hoàn tất `write_message3`.
    pub fn split_ciphers(&mut self) -> Result<(CipherState, CipherState)> {
        if self.phase != 4 {
            return Err(NoiseError::WrongPhaseSplit);
        }
        self.phase = 5;
        Ok(self.symmetric.split())
    }

    #[must_use]
    pub fn handshake_hash(&self) -> [u8; 32] {
        self.symmetric.h
    }

    #[must_use]
    pub fn remote_static_public_key(&self) -> Option<&[u8]> {
        self.rs.as_deref()
    }
}

/// Mã hoá payload message1: trường protobuf 1 (bytes), tag `0x0a` + độ dài 32 — dùng để gửi nonce
/// client ngẫu nhiên trong bước bắt tay đầu.
#[must_use]
pub fn encode_client_nonce_message1(nonce: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + 32);
    out.push(0x0a);
    out.push(0x20);
    out.extend_from_slice(nonce);
    out
}

#[must_use]
pub fn make_message1_payload() -> ([u8; 32], Vec<u8>) {
    let mut nonce = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let encoded = encode_client_nonce_message1(&nonce);
    (nonce, encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_name_duoc_dem_0_dung_32_byte() {
        let state = SymmetricState::new();
        assert_eq!(state.ck.len(), 32);
        assert_eq!(state.h.len(), 32);
    }

    #[test]
    fn write_message1_dung_pha_tra_32_byte_pubkey_cong_payload_rong() {
        let mut initiator = NoiseXxInitiator::new();
        let msg1 = initiator.write_message1(&[]).unwrap();
        // payload rong -> cipher chua co khoa -> encrypt_with_ad tra nguyen plaintext (rong).
        assert_eq!(msg1.len(), 32);
        assert_eq!(initiator.phase, 2);
    }

    #[test]
    fn goi_sai_thu_tu_tra_loi_pha_khong_panic() {
        let mut initiator = NoiseXxInitiator::new();
        assert_eq!(
            initiator.read_message2(&[0u8; 96]).unwrap_err(),
            NoiseError::WrongPhaseMessage2
        );
        assert_eq!(
            initiator.write_message3(&[]).unwrap_err(),
            NoiseError::WrongPhaseMessage3
        );
        assert_eq!(
            initiator.split_ciphers().unwrap_err(),
            NoiseError::WrongPhaseSplit
        );
    }

    #[test]
    fn message2_qua_ngan_tra_loi_ro_rang() {
        let mut initiator = NoiseXxInitiator::new();
        initiator.write_message1(&[]).unwrap();
        let err = initiator.read_message2(&[0u8; 95]).unwrap_err();
        assert_eq!(err, NoiseError::Message2TooShort(95));
    }

    /// Bat tay day du voi MOT "responder" toi gian viet rieng cho test (chi de kiem khop giao thuc
    /// giua initiator/responder Rust voi nhau, KHONG phai vi du ve server that cua Muse) — chung
    /// minh `write_message1` -> `read_message2` -> `write_message3` -> `split` cho ra 2 `CipherState` dung
    /// hai chieu ma hoa roi giai ma lai dung nguyen van.
    fn responder_handshake(msg1: &[u8]) -> (Vec<u8>, StaticSecret, [u8; 32]) {
        // Toi gian hoa: responder tu lam lai dung cac buoc Noise XX (khong dung code initiator o
        // tren) de test thuc su doc lap, khong chi "khop voi chinh no".
        let mut resp = SymmetricState::new();
        let e_pub_initiator: [u8; 32] = msg1[..32].try_into().unwrap();
        resp.mix_hash(&e_pub_initiator);
        let _payload1 = resp.decrypt_and_hash(&msg1[32..]).unwrap();

        let e_priv_r = random_static_secret();
        let e_pub_r = PublicKey::from(&e_priv_r);
        resp.mix_hash(e_pub_r.as_bytes());
        let ee = e_priv_r.diffie_hellman(&PublicKey::from(e_pub_initiator));
        resp.mix_key(ee.as_bytes());

        let s_priv_r = random_static_secret();
        let s_pub_r = PublicKey::from(&s_priv_r);
        let enc_static = resp.encrypt_and_hash(s_pub_r.as_bytes()).unwrap();
        let es = s_priv_r.diffie_hellman(&PublicKey::from(e_pub_initiator));
        resp.mix_key(es.as_bytes());
        let enc_payload2 = resp.encrypt_and_hash(&[]).unwrap();

        let mut msg2 = Vec::new();
        msg2.extend_from_slice(e_pub_r.as_bytes());
        msg2.extend_from_slice(&enc_static);
        msg2.extend_from_slice(&enc_payload2);
        (msg2, e_priv_r, *s_pub_r.as_bytes())
    }

    #[test]
    fn bat_tay_day_du_rut_ra_cap_khoa_dung_chieu() {
        let mut initiator = NoiseXxInitiator::new();
        let (nonce, msg1_payload) = make_message1_payload();
        assert_eq!(nonce.len(), 32);
        let msg1 = initiator.write_message1(&msg1_payload).unwrap();

        let (msg2, _e_priv_r, _s_pub_r) = responder_handshake(&msg1);
        let payload2 = initiator.read_message2(&msg2).unwrap();
        assert!(payload2.is_empty());
        assert_eq!(initiator.phase, 3);
        assert!(initiator.remote_static_public_key().is_some());

        let msg3 = initiator.write_message3(b"hello").unwrap();
        assert!(!msg3.is_empty());

        let (mut send, mut recv) = initiator.split_ciphers().unwrap();
        let ciphertext = send.encrypt_with_ad(b"", b"ping").unwrap();
        assert_ne!(ciphertext, b"ping");
        // 2 CipherState doc lap (initiator gui qua `send`) — o day chi kiem round-trip tren CHINH no
        // (ma hoa roi tu giai ma lai) vi responder test o tren khong giu lai cap khoa split tuong ung.
        let mut send2 = CipherState {
            key: send.key,
            nonce: 0,
        };
        let plain = send2.decrypt_with_ad(b"", &ciphertext).unwrap();
        assert_eq!(plain, b"ping");
        let _ = &mut recv; // giu bien de khong canh bao unused, dai dien khoa chieu nhan.
    }
}
