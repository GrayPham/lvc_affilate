//! Đọc kích thước ảnh (rộng, cao) THUẦN từ byte — chỉ PNG và JPEG, KHÔNG thêm dep ảnh.
//!
//! Bản Python dùng PIL đọc `im.size`; nếu không đọc được thì để `(0, 0)` và Dola tự đo
//! (`dola_client.py:466-486`). Ta cũng chấp nhận `(0, 0)` cho định dạng khác.

/// Rộng/cao của ảnh, hoặc `(0, 0)` nếu không nhận dạng được (chấp nhận như bản Python).
#[must_use]
pub fn image_size(bytes: &[u8]) -> (u32, u32) {
    png_size(bytes)
        .or_else(|| jpeg_size(bytes))
        .unwrap_or((0, 0))
}

/// PNG: chữ ký 8 byte + IHDR chứa width/height big-endian tại offset 16..24.
fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if b.len() < 24 || b[..8] != SIG {
        return None;
    }
    let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    Some((w, h))
}

/// JPEG: SOI `FFD8` rồi duyệt các marker tới SOF0..SOFF (trừ các marker không phải khung) đọc
/// height/width big-endian.
fn jpeg_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = b[i + 1];
        // Các marker độc lập không có payload/không phải khung.
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        let seg_len = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
        // SOF0..SOF15, trừ SOF4(C4=DHT), SOF8(C8=JPG), SOF12(CC=DAC) — các marker khung thật.
        let is_sof =
            (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC;
        if is_sof && i + 9 < b.len() {
            let h = u32::from(u16::from_be_bytes([b[i + 5], b[i + 6]]));
            let w = u32::from(u16::from_be_bytes([b[i + 7], b[i + 8]]));
            return Some((w, h));
        }
        i += 2 + seg_len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_doc_duoc_kich_thuoc() {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        b.extend_from_slice(&[0, 0, 0, 13]); // len IHDR
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&640u32.to_be_bytes());
        b.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(image_size(&b), (640, 480));
    }

    #[test]
    fn jpeg_doc_duoc_kich_thuoc() {
        // FFD8 SOI, APP0 (len 4, 2 byte payload), SOF0 (len 17) height=200 width=100.
        let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        b.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        b.extend_from_slice(&200u16.to_be_bytes());
        b.extend_from_slice(&100u16.to_be_bytes());
        b.extend_from_slice(&[0x03, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(image_size(&b), (100, 200));
    }

    #[test]
    fn la_thi_khong() {
        assert_eq!(image_size(b"not an image"), (0, 0));
    }
}
