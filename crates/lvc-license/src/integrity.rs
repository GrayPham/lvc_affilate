//! Kiểm tra toàn vẹn public key nhúng trong file chạy.
//!
//! Cách crack thực tế nhất cho một client cấp phép dùng chữ ký là THAY public key trong file exe
//! bằng key của kẻ crack, rồi dựng server giả ký bằng private key của họ. Mọi lớp guard chữ ký đều
//! "qua" vì chúng tin vào key đang nhúng. Module này giữ vân tay (SHA-256 của SPKI DER) của key
//! đúng, nên key bị đổi thì [`embedded_key_intact`] trả `false`.
//!
//! GIỚI HẠN (nói thẳng): đây là nâng chi phí, KHÔNG phải bất khả xâm phạm. Kẻ crack đọc được
//! assembly vẫn có thể vá luôn cả hằng số vân tay lẫn các chỗ gọi hàm này. Vì vậy:
//!  - vân tay bị che bằng XOR theo vị trí để không tìm ra bằng grep/hex-search chuỗi hash;
//!  - hàm được gọi ở NHIỀU chỗ không liên quan nhau (kích hoạt, trạng thái, cổng backend), nên vá
//!    một chỗ là chưa đủ;
//!  - kết quả trả về là `bool` do caller chuyển thành lỗi chung, không in lý do ra UI.
//!
//! KHÔNG log vân tay hay lý do.

use base64::Engine as _;
use sha2::{Digest, Sha256};

/// Vân tay SHA-256 của SPKI DER public key đúng, đã che: `masked[i] = fp[i] ^ (0x5A + 7*i)`.
/// Test `van_tay_khop_key_nhung` giữ hằng số này luôn khớp `crate::PUBLIC_KEY_PEM`.
const FP_MASKED: [u8; 32] = [
    0x56, 0xa8, 0xc6, 0x63, 0xcd, 0xce, 0x5e, 0x85, 0x53, 0x40, 0x8d, 0x46, 0xa1, 0xc6, 0x35, 0x30,
    0x37, 0x7e, 0xa0, 0x8e, 0x38, 0x3e, 0x75, 0xc5, 0x31, 0xdc, 0x69, 0x5d, 0xe7, 0xa2, 0x94, 0x03,
];

/// Mặt nạ cho byte thứ `i`. `wrapping_*` để không panic khi `i` lớn.
fn mask(i: usize) -> u8 {
    #[allow(clippy::cast_possible_truncation, reason = "i < 32, vừa u8")]
    let i = i as u8;
    0x5Au8.wrapping_add(i.wrapping_mul(7))
}

/// Vân tay đúng đã giải che.
fn expected_fingerprint() -> [u8; 32] {
    let mut fp = [0u8; 32];
    for (i, b) in FP_MASKED.iter().enumerate() {
        fp[i] = b ^ mask(i);
    }
    fp
}

/// SHA-256 của phần DER trong khối PEM (bỏ dòng `-----BEGIN/END`, giải base64). `None` nếu hỏng.
#[must_use]
pub fn key_fingerprint(pem: &str) -> Option<[u8; 32]> {
    let body: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .map(str::trim)
        .collect();
    let der = base64::engine::general_purpose::STANDARD
        .decode(body)
        .ok()?;
    Some(Sha256::digest(der).into())
}

/// So sánh không rò thời gian (không dừng ở byte lệch đầu tiên).
fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    diff == 0
}

/// `true` nếu public key nhúng ([`crate::PUBLIC_KEY_PEM`]) vẫn đúng là key của nhà phát hành.
#[must_use]
pub fn embedded_key_intact() -> bool {
    key_fingerprint(crate::PUBLIC_KEY_PEM).is_some_and(|fp| ct_eq(&fp, &expected_fingerprint()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn van_tay_khop_key_nhung() {
        assert!(
            embedded_key_intact(),
            "hằng số FP_MASKED lệch PUBLIC_KEY_PEM"
        );
    }

    #[test]
    fn doi_mot_ky_tu_cua_key_thi_bi_phat_hien() {
        // Mô phỏng kẻ crack thay key: đổi một ký tự base64 trong thân PEM.
        let tampered = crate::PUBLIC_KEY_PEM.replacen("MFkw", "MFkx", 1);
        let fp = key_fingerprint(&tampered).expect("vẫn là base64 hợp lệ");
        assert!(!ct_eq(&fp, &expected_fingerprint()));
    }

    #[test]
    fn key_hong_khong_giai_duoc_thi_khong_duoc_coi_la_nguyen_ven() {
        assert!(
            key_fingerprint("-----BEGIN PUBLIC KEY-----\n!!!\n-----END PUBLIC KEY-----").is_none()
        );
    }

    #[test]
    fn mat_na_giai_ra_dung_do_dai() {
        assert_eq!(expected_fingerprint().len(), 32);
    }
}
