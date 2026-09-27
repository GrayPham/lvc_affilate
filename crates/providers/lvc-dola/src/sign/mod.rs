//! Ký request app Android Dola (P4b): X-Gorgon / X-Khronos / X-Ladon / X-Argus.
//!
//! Port THUẦN Rust từ `A:\ToolLVC\lvcseedance\...\x_signatures` (chỉ đọc), khớp golden vector từng
//! byte (`tests/sign_vectors.rs`). Chia nhỏ mỗi thuật toán một file (luật module-split):
//! [`sm3`], [`simon`], [`protobuf`], [`gorgon`], [`ladon`], [`argus`], [`pkcs7`].
//!
//! Các hàm lõi là HÀM THUẦN nhận thời gian/ngẫu nhiên làm THAM SỐ để kiểm thử tất định. [`RealSigner`]
//! bọc chúng cho runtime (thời gian/ngẫu nhiên thật) và impl [`crate::session::RequestSigner`].

pub mod argus;
pub mod gorgon;
pub mod ladon;
pub mod pkcs7;
pub mod protobuf;
pub mod simon;
pub mod sm3;

use md5::{Digest as _, Md5};
use rand::Rng as _;

use crate::session::RequestSigner;

pub use argus::xargus;
pub use gorgon::xgorgon;
pub use ladon::xladon;

/// md5 → chuỗi hex thường (32 ký tự).
#[must_use]
pub(crate) fn md5_hex(data: &[u8]) -> String {
    let mut h = Md5::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// md5 → 16 byte.
#[must_use]
pub(crate) fn md5_bytes(data: &[u8]) -> [u8; 16] {
    let mut h = Md5::new();
    h.update(data);
    h.finalize().into()
}

/// Đọc u64 little-endian từ 8 byte đầu của lát cắt.
#[must_use]
pub(crate) fn le_u64(s: &[u8]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&s[..8]);
    u64::from_le_bytes(b)
}

/// Bộ ký thật cho phiên app Android. Giữ cấu hình cố định của app; thời gian/ngẫu nhiên lấy lúc ký.
///
/// KHÔNG khớp vector ở runtime (random/time thật) — chỉ các hàm thuần bên dưới khớp vector.
#[derive(Debug, Clone)]
pub struct RealSigner {
    /// `aid` của app (vd 489823).
    pub aid: u64,
    /// `license_id` của app.
    pub license_id: u64,
    /// `sec_device_id` (rỗng = không chèn field 16 của Argus).
    pub sec_device_id: String,
    /// Chuỗi phiên bản SDK (vd `v04.04.05-ov-android`).
    pub sdk_version: String,
    /// Số phiên bản SDK (vd 134744640).
    pub sdk_version_int: u64,
    /// Nền tảng (android = 0, ios = 1).
    pub platform: u64,
}

impl RequestSigner for RealSigner {
    fn sign(
        &self,
        query: &str,
        _headers: &[(String, String)],
        body: Option<&[u8]>,
        cookie: &str,
        now_ms: u64,
    ) -> Vec<(String, String)> {
        let secs = now_ms / 1000;
        // x-ss-stub = md5(body) HOA (hex) khi có body; dùng chung cho Gorgon + Argus.
        let stub = body.map(|b| md5_hex(b).to_uppercase());
        let cookie_opt = if cookie.is_empty() {
            None
        } else {
            Some(cookie)
        };

        let (x_gorgon, x_khronos) = xgorgon(query, stub.as_deref(), cookie_opt, secs);

        let mut rng = rand::thread_rng();
        let random4: [u8; 4] = rng.r#gen();
        let x_ladon = xladon(secs, self.license_id, self.aid, random4);

        let rand_field3: u32 = rng.r#gen::<u32>() & 0x7FFF_FFFF;
        let x_argus = xargus(
            query,
            stub.as_deref(),
            secs,
            self.aid,
            self.license_id,
            self.platform,
            &self.sec_device_id,
            &self.sdk_version,
            self.sdk_version_int,
            rand_field3,
        );

        vec![
            ("X-Gorgon".to_string(), x_gorgon),
            ("X-Khronos".to_string(), x_khronos),
            ("X-Ladon".to_string(), x_ladon),
            ("X-Argus".to_string(), x_argus),
        ]
    }
}
