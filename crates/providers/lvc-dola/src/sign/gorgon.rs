//! X-Gorgon / X-Khronos — port `x_signatures/lib/XGorgon.py`.
//!
//! Thuật toán RC4-like tự chứa: dựng danh sách 20 byte từ md5(params) / x-ss-stub / md5(cookie) /
//! zero / khronos, rồi qua ba bước `encryption`/`initialize`/`handle`. Hằng: `length = 20`,
//! `hex_str = [30,64,224,217,147,69,0,180]`. Khronos dùng THỜI GIAN GIÂY.

use super::md5_hex;

/// `hex_str` — 8 hằng của thuật toán.
const HEX_STR: [u32; 8] = [30, 64, 224, 217, 147, 69, 0, 180];
/// Độ dài danh sách gorgon.
const LENGTH: usize = 20;

/// Đọc cặp hex thứ `i` (byte thứ `i`) của chuỗi hex; trả 0 nếu thiếu.
fn hexpair(s: &str, i: usize) -> u32 {
    s.get(2 * i..2 * i + 2)
        .and_then(|p| u32::from_str_radix(p, 16).ok())
        .unwrap_or(0)
}

/// `__hex2string`: byte → 2 ký tự hex thường.
fn hex2(n: u32) -> String {
    format!("{:02x}", n & 0xFF)
}

/// `__reverse`: đảo hai nibble của một byte.
fn reverse_nibbles(n: u32) -> u32 {
    ((n & 0x0F) << 4) | ((n >> 4) & 0x0F)
}

/// `__RBIT`: đảo thứ tự 8 bit.
fn rbit8(n: u32) -> u32 {
    u32::from(u8::try_from(n & 0xFF).unwrap_or(0).reverse_bits())
}

/// `__encryption`: sinh bảng hoán vị 256 phần tử (chỉ phụ thuộc `HEX_STR`).
#[allow(
    clippy::needless_range_loop,
    reason = "chi so i tham gia phep tinh (A+i+B, hex_zu[i-1]) — giu vong for i de doi chieu Python"
)]
fn encryption() -> [u32; 256] {
    let mut hex_zu: [u32; 256] = core::array::from_fn(|i| u32::try_from(i).unwrap_or(0));
    // tmp: None = chuỗi rỗng '' (falsy). Some(v) truthy khi v != 0.
    let mut tmp: Option<u32> = None;
    for i in 0..256 {
        let mut a = if i == 0 {
            0
        } else if matches!(tmp, Some(v) if v != 0) {
            tmp.unwrap_or(0)
        } else {
            hex_zu[i - 1]
        };
        let b = HEX_STR[i % 8];
        if a == 85 && i != 1 {
            let tmp_ne_85 = match tmp {
                Some(v) => v != 85,
                None => true,
            };
            if tmp_ne_85 {
                a = 0;
            }
        }
        let iu = u32::try_from(i).unwrap_or(0);
        let mut c = a + iu + b;
        while c >= 256 {
            c -= 256;
        }
        tmp = if c < iu { Some(c) } else { None };
        hex_zu[i] = hex_zu[c as usize];
    }
    hex_zu
}

/// `__initialize`: trộn danh sách gorgon với bảng hoán vị.
#[allow(
    clippy::needless_range_loop,
    clippy::many_single_char_names,
    reason = "chep nguyen ten bien + truy cap hex_zu[i+1] theo chi so de doi chieu Python"
)]
fn initialize(input: &mut [u32; LENGTH], hex_zu: &[u32; 256]) {
    let mut tmp_hex = *hex_zu;
    let mut last_add: Option<u32> = None;
    for i in 0..LENGTH {
        let a = input[i];
        let b = last_add.unwrap_or(0);
        let mut c = hex_zu[i + 1] + b;
        while c >= 256 {
            c -= 256;
        }
        last_add = Some(c);
        let d = tmp_hex[c as usize];
        tmp_hex[i + 1] = d;
        let mut e = d + d;
        while e >= 256 {
            e -= 256;
        }
        let f = tmp_hex[e as usize];
        input[i] = a ^ f;
    }
}

/// `__handle`: bước cuối — sửa tại chỗ nên `input[(i+1)%LENGTH]` có thể đã đổi (khi `i == 19`).
#[allow(
    clippy::needless_range_loop,
    clippy::many_single_char_names,
    reason = "sua tai cho + doc hang xom (i+1)%LENGTH — giu vong for i va ten bien de doi chieu Python"
)]
fn handle(input: &mut [u32; LENGTH]) {
    for i in 0..LENGTH {
        let a = input[i];
        let bb = reverse_nibbles(a);
        let c = input[(i + 1) % LENGTH];
        let d = bb ^ c;
        let e = rbit8(d);
        let f = e ^ u32::try_from(LENGTH).unwrap_or(0);
        input[i] = (!f) & 0xFF;
    }
}

/// `__main`: dựng chuỗi X-Gorgon.
fn main_gorgon(mut gorgon: [u32; LENGTH]) -> String {
    let hex_zu = encryption();
    initialize(&mut gorgon, &hex_zu);
    handle(&mut gorgon);

    let mut result = String::from("0404");
    result.push_str(&hex2(HEX_STR[7]));
    result.push_str(&hex2(HEX_STR[3]));
    result.push_str(&hex2(HEX_STR[1]));
    result.push_str(&hex2(HEX_STR[6]));
    for item in gorgon {
        result.push_str(&hex2(item));
    }
    result
}

/// Tính `(X-Gorgon, X-Khronos)`.
///
/// - `params`: query string (không có `?` đứng đầu).
/// - `stub`: giá trị header `x-ss-stub` (đã là md5 hex hoa) nếu có.
/// - `cookie`: chuỗi cookie thô nếu có (sẽ md5 trong hàm).
/// - `khronos_secs`: thời gian UNIX theo GIÂY.
#[must_use]
#[allow(
    clippy::needless_range_loop,
    reason = "dat gorgon[offset+i] tu cap hex thu i — chi so co nghia, giu vong for i"
)]
pub fn xgorgon(
    params: &str,
    stub: Option<&str>,
    cookie: Option<&str>,
    khronos_secs: u64,
) -> (String, String) {
    let khronos_hex = format!("{khronos_secs:x}");
    let url_md5 = md5_hex(params.as_bytes());

    let mut gorgon = [0u32; LENGTH];
    for i in 0..4 {
        gorgon[i] = hexpair(&url_md5, i);
    }
    if let Some(s) = stub {
        for i in 0..4 {
            gorgon[4 + i] = hexpair(s, i);
        }
    }
    if let Some(c) = cookie {
        let cm = md5_hex(c.as_bytes());
        for i in 0..4 {
            gorgon[8 + i] = hexpair(&cm, i);
        }
    }
    // gorgon[12..16] giữ 0.
    for i in 0..4 {
        gorgon[16 + i] = hexpair(&khronos_hex, i);
    }

    (main_gorgon(gorgon), khronos_secs.to_string())
}
