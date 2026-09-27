//! Simon cipher 128/256 (block 128 bit, key 256 bit, 72 vòng) — port `x_signatures/lib/Simon.py`.
//!
//! Dùng để mã hoá protobuf trong X-Argus. `z = 0x3DC94C3A046D678B` (chuỗi hằng vòng của Simon),
//! biến thể ở đây dùng `c = 0` (f = ROL1 & ROL8).

/// Hằng chuỗi vòng z (Simon 128/256).
const Z: u64 = 0x3DC9_4C3A_046D_678B;

/// Lấy bit thứ `pos` của `val`.
#[inline]
fn get_bit(val: u64, pos: u32) -> u64 {
    u64::from(val & (1u64 << pos) != 0)
}

/// Mở rộng khoá: 4 word đầu là khoá gốc, sinh tiếp tới 72 word.
#[must_use]
pub fn key_expansion(k: &[u64; 4]) -> [u64; 72] {
    let mut key = [0u64; 72];
    key[..4].copy_from_slice(k);
    for i in 4..72 {
        let mut tmp = key[i - 1].rotate_right(3);
        tmp ^= key[i - 3];
        tmp ^= tmp.rotate_right(1);
        let bit = get_bit(Z, u32::try_from((i - 4) % 62).unwrap_or(0));
        key[i] = (!key[i - 4]) ^ tmp ^ bit ^ 3;
    }
    key
}

/// Mã hoá một khối 128 bit (`pt[0]`, `pt[1]`) với khoá 256 bit.
#[must_use]
pub fn simon_enc(pt: [u64; 2], k: &[u64; 4]) -> [u64; 2] {
    let key = key_expansion(k);
    let mut x_i = pt[0];
    let mut x_i1 = pt[1];
    for &kw in &key {
        let tmp = x_i1;
        let f = x_i1.rotate_left(1) & x_i1.rotate_left(8);
        x_i1 = x_i ^ f ^ x_i1.rotate_left(2) ^ kw;
        x_i = tmp;
    }
    [x_i, x_i1]
}

/// Giải mã một khối (đối xứng với [`simon_enc`]). Giữ để đối chiếu/kiểm thử.
#[must_use]
pub fn simon_dec(ct: [u64; 2], k: &[u64; 4]) -> [u64; 2] {
    let key = key_expansion(k);
    let mut x_i = ct[0];
    let mut x_i1 = ct[1];
    for i in (0..72).rev() {
        let tmp = x_i;
        let f = x_i.rotate_left(1) & x_i.rotate_left(8);
        x_i = x_i1 ^ f ^ x_i.rotate_left(2) ^ key[i];
        x_i1 = tmp;
    }
    [x_i, x_i1]
}
