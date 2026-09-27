//! Đệm PKCS#7 — port `x_signatures/lib/pkcs7_padding.py`.
//!
//! Có HAI hành vi khác nhau, tương ứng hai chỗ dùng trong bản Python:
//! - [`pkcs7_pad`]: kiểu `Crypto.Util.Padding.pad` — LUÔN thêm đệm, kể cả khi đã tròn khối
//!   (thêm nguyên một khối `block`). Dùng cho protobuf của Argus.
//! - [`ladon_pad`]: kiểu `padding_size` + `pkcs7_padding_pad_buffer` của Ladon — CHỈ đệm phần
//!   thiếu để tròn khối; nếu đã tròn khối thì KHÔNG thêm gì (buffer đúng bằng dữ liệu gốc).

/// Đệm PKCS#7 chuẩn: luôn thêm `pad` byte giá trị `pad` (khi đã tròn khối thì thêm cả một khối).
#[must_use]
pub fn pkcs7_pad(data: &[u8], block: usize) -> Vec<u8> {
    let pad = block - (data.len() % block);
    let mut out = data.to_vec();
    out.resize(data.len() + pad, u8::try_from(pad).unwrap_or(0));
    out
}

/// Làm tròn kích thước lên bội số 16 (`padding_size`).
#[must_use]
pub fn padding_size(size: usize) -> usize {
    let m = size % 16;
    if m > 0 { size + (16 - m) } else { size }
}

/// Đệm kiểu Ladon: cấp buffer `padding_size(size)`, chép dữ liệu, rồi lấp phần còn lại bằng số byte
/// đệm — NHƯNG nếu `size` đã tròn 16 thì không thêm gì (giống nhánh `data_length+pad_byte > buffer`).
#[must_use]
pub fn ladon_pad(data: &[u8]) -> Vec<u8> {
    let size = data.len();
    let new_size = padding_size(size);
    let mut out = vec![0u8; new_size];
    out[..size].copy_from_slice(data);
    let pad_byte = 16 - (size % 16);
    if size + pad_byte <= new_size {
        let value = u8::try_from(pad_byte).unwrap_or(0);
        for b in out.iter_mut().skip(size) {
            *b = value;
        }
    }
    out
}
