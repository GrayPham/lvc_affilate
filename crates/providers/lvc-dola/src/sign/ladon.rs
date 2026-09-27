//! X-Ladon — port `x_signatures/lib/XLadon.py`.
//!
//! `data = "{ts}-{license}-{aid}"`, khoá = md5(random4 + str(aid)); mã hoá bằng bảng băm mở rộng
//! (272+16 byte, xoay phải 64-bit, 0x22 vòng), đệm kiểu Ladon, ghép `random4` (4 byte) + ciphertext,
//! rồi base64. `random4` là 4 BYTE (đầu ra base64 chứa nó nên kiểm thử tách lại được).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;

use super::pkcs7::ladon_pad;
use super::{le_u64, md5_hex};

/// Số vòng của Ladon.
const ROUNDS: usize = 0x22;

/// Mã hoá một khối 16 byte với bảng băm đã mở rộng.
#[allow(
    clippy::needless_range_loop,
    reason = "doc hash_table theo chi so vong i (i*8)"
)]
fn encrypt_ladon_input(hash_table: &[u8; 288], input: &[u8]) -> [u8; 16] {
    let mut data0 = le_u64(&input[..8]);
    let mut data1 = le_u64(&input[8..16]);
    for i in 0..ROUNDS {
        let hash = le_u64(&hash_table[i * 8..i * 8 + 8]);
        data1 = hash ^ data0.wrapping_add(data1.rotate_right(8));
        data0 = data1 ^ data0.rotate_right(61);
    }
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&data0.to_le_bytes());
    out[8..].copy_from_slice(&data1.to_le_bytes());
    out
}

/// Mở rộng bảng băm từ `md5hex` (32 ký tự ASCII) rồi mã hoá `data`. Trả ciphertext (đã đệm).
#[must_use]
#[allow(
    clippy::needless_range_loop,
    reason = "xu ly theo tung khoi 16 byte bang chi so blk"
)]
pub fn encrypt_ladon(md5hex: &[u8], data: &[u8]) -> Vec<u8> {
    let mut hash_table = [0u8; 288];
    hash_table[..32].copy_from_slice(&md5hex[..32]);

    let mut temp: Vec<u64> = (0..4)
        .map(|i| le_u64(&hash_table[i * 8..i * 8 + 8]))
        .collect();
    let mut buffer_b0 = temp[0];
    let mut buffer_b8 = temp[1];
    temp.remove(0);
    temp.remove(0);

    for i in 0..ROUNDS as u64 {
        let x9 = buffer_b0;
        let mut x8 = buffer_b8;
        x8 = x8.rotate_right(8);
        x8 = x8.wrapping_add(x9);
        x8 ^= i;
        temp.push(x8);
        x8 ^= x9.rotate_right(61);
        let idx = (usize::try_from(i).unwrap_or(0) + 1) * 8;
        hash_table[idx..idx + 8].copy_from_slice(&x8.to_le_bytes());
        buffer_b0 = x8;
        buffer_b8 = temp[0];
        temp.remove(0);
    }

    let padded = ladon_pad(data);
    let mut output = vec![0u8; padded.len()];
    for blk in 0..padded.len() / 16 {
        let enc = encrypt_ladon_input(&hash_table, &padded[blk * 16..blk * 16 + 16]);
        output[blk * 16..blk * 16 + 16].copy_from_slice(&enc);
    }
    output
}

/// Tính chuỗi X-Ladon.
///
/// `random4` là 4 byte ngẫu nhiên (test truyền cố định để khớp vector).
#[must_use]
pub fn xladon(timestamp: u64, license_id: u64, aid: u64, random4: [u8; 4]) -> String {
    let data = format!("{timestamp}-{license_id}-{aid}");
    let mut keygen = random4.to_vec();
    keygen.extend_from_slice(aid.to_string().as_bytes());
    let md5hex = md5_hex(&keygen);

    let enc = encrypt_ladon(md5hex.as_bytes(), data.as_bytes());
    let mut output = random4.to_vec();
    output.extend_from_slice(&enc);
    B64.encode(&output)
}
