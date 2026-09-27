//! Bộ mã hoá protobuf tối giản — port encoder của `x_signatures/lib/protobuf.py` (`ProtoBuf.toBuf`).
//!
//! QUIRK BẮT BUỘC (khớp byte): `writeVarint` dùng `while vint > 0x80` (KHÔNG phải `>=`) và mask
//! `vint & 0xFFFFFFFF` (32-bit) TRƯỚC vòng lặp. Field ghi theo ĐÚNG thứ tự chèn.
//! Kiểu: `Varint` → wire type 0, `Str`/`Nested` → wire type 2 (chuỗi độ dài). key = `(idx<<3)|type`.

/// Một giá trị field protobuf.
#[derive(Debug, Clone)]
pub enum ProtoVal {
    /// Số nguyên (wire type VARINT = 0).
    Varint(u64),
    /// Chuỗi byte / utf8 (wire type STRING = 2).
    Str(Vec<u8>),
    /// Message lồng (wire type STRING = 2, nội dung là protobuf con).
    Nested(Vec<(u32, ProtoVal)>),
}

/// Ghi một varint theo đúng thuật toán Python (mask 32-bit, điều kiện `> 0x80`).
fn write_varint(out: &mut Vec<u8>, vint: u64) {
    let mut v = vint & 0xFFFF_FFFF;
    while v > 0x80 {
        out.push(u8::try_from((v & 0x7F) | 0x80).unwrap_or(0));
        v >>= 7;
    }
    out.push(u8::try_from(v & 0x7F).unwrap_or(0));
}

/// Mã hoá danh sách field (giữ nguyên thứ tự) thành protobuf.
#[must_use]
pub fn protobuf_encode(fields: &[(u32, ProtoVal)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (idx, val) in fields {
        match val {
            ProtoVal::Varint(v) => {
                write_varint(&mut out, u64::from(idx << 3));
                write_varint(&mut out, *v);
            }
            ProtoVal::Str(b) => {
                write_varint(&mut out, u64::from((idx << 3) | 2));
                write_varint(&mut out, u64::try_from(b.len()).unwrap_or(0));
                out.extend_from_slice(b);
            }
            ProtoVal::Nested(inner_fields) => {
                let inner = protobuf_encode(inner_fields);
                write_varint(&mut out, u64::from((idx << 3) | 2));
                write_varint(&mut out, u64::try_from(inner.len()).unwrap_or(0));
                out.extend_from_slice(&inner);
            }
        }
    }
    out
}
