//! SM3 hash — port nguyên `x_signatures/lib/Sm3.py`.
//!
//! Dùng cho `bodyHash`/`queryHash` trong X-Argus. Hàm thuần, không phụ thuộc thời gian/ngẫu nhiên
//! nên khớp golden vector từng byte.

/// Giá trị khởi tạo IV (8 word 32-bit).
const IV: [u32; 8] = [
    1_937_774_191,
    1_226_093_241,
    388_252_375,
    3_666_478_592,
    2_842_636_476,
    372_324_522,
    3_817_729_613,
    2_969_243_214,
];

/// Hằng `T_j`: 16 vòng đầu dùng `0x79cc4519`, 48 vòng sau dùng `0x7a879d8a`.
const T0: u32 = 2_043_430_169;
const T1: u32 = 2_055_708_042;

/// Xoay trái 32-bit (`rotate_left` tự lấy `k % 32`, `k == 0` giữ nguyên) — khớp `__rotate_left` Python.
#[inline]
fn rotl32(a: u32, k: u32) -> u32 {
    a.rotate_left(k)
}

#[inline]
fn ffj(x: u32, y: u32, z: u32, j: usize) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | (x & z) | (y & z)
    }
}

#[inline]
fn ggj(x: u32, y: u32, z: u32, j: usize) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | ((!x) & z)
    }
}

#[inline]
fn p0(x: u32) -> u32 {
    x ^ rotl32(x, 9) ^ rotl32(x, 17)
}

#[inline]
fn p1(x: u32) -> u32 {
    x ^ rotl32(x, 15) ^ rotl32(x, 23)
}

fn tj(j: usize) -> u32 {
    if j < 16 { T0 } else { T1 }
}

/// Hàm nén CF cho một khối 64 byte.
#[allow(
    clippy::many_single_char_names,
    reason = "chep nguyen ten bien A..H cua SM3 de doi chieu voi ban Python"
)]
fn cf(v: &[u32; 8], block: &[u8]) -> [u32; 8] {
    let mut w = [0u32; 68];
    for (i, item) in w.iter_mut().enumerate().take(16) {
        let base = i * 4;
        *item = (u32::from(block[base]) << 24)
            | (u32::from(block[base + 1]) << 16)
            | (u32::from(block[base + 2]) << 8)
            | u32::from(block[base + 3]);
    }
    for j in 16..68 {
        w[j] = p1(w[j - 16] ^ w[j - 9] ^ rotl32(w[j - 3], 15)) ^ rotl32(w[j - 13], 7) ^ w[j - 6];
    }
    let mut w1 = [0u32; 64];
    for (j, item) in w1.iter_mut().enumerate() {
        *item = w[j] ^ w[j + 4];
    }

    let (mut a, mut b, mut c, mut d) = (v[0], v[1], v[2], v[3]);
    let (mut e, mut f, mut g, mut h) = (v[4], v[5], v[6], v[7]);

    for j in 0..64 {
        let ss1 = rotl32(
            rotl32(a, 12)
                .wrapping_add(e)
                .wrapping_add(rotl32(tj(j), u32::try_from(j).unwrap_or(0))),
            7,
        );
        let ss2 = ss1 ^ rotl32(a, 12);
        let tt1 = ffj(a, b, c, j)
            .wrapping_add(d)
            .wrapping_add(ss2)
            .wrapping_add(w1[j]);
        let tt2 = ggj(e, f, g, j)
            .wrapping_add(h)
            .wrapping_add(ss1)
            .wrapping_add(w[j]);
        d = c;
        c = rotl32(b, 9);
        b = a;
        a = tt1;
        h = g;
        g = rotl32(f, 19);
        f = e;
        e = p0(tt2);
    }

    [
        a ^ v[0],
        b ^ v[1],
        c ^ v[2],
        d ^ v[3],
        e ^ v[4],
        f ^ v[5],
        g ^ v[6],
        h ^ v[7],
    ]
}

/// Băm SM3, trả 32 byte.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "len*8 lam do dai bit — dau vao thuc te khong bao gio tran u64"
)]
pub fn sm3_hash(msg: &[u8]) -> [u8; 32] {
    let len1 = msg.len();
    let mut buf = msg.to_vec();

    // Đệm: thêm 0x80 rồi 0x00 cho tới khi còn 8 byte cuối khối chứa độ dài bit.
    buf.push(0x80);
    let reserve = (len1 % 64) + 1;
    let mut range_end = 56usize;
    if reserve > range_end {
        range_end += 64;
    }
    buf.resize(buf.len() + (range_end - reserve), 0x00);
    let bit_length = (len1 as u64).wrapping_mul(8);
    buf.extend_from_slice(&bit_length.to_be_bytes());

    let mut v = IV;
    let group_count = buf.len() / 64;
    for i in 0..group_count {
        v = cf(&v, &buf[i * 64..(i + 1) * 64]);
    }

    let mut out = [0u8; 32];
    for (i, word) in v.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}
