//! Codec protobuf tối giản cho vận chuyển Muse Hatch Noise.
//!
//! Port 1:1 từ `wire.py` (tham khảo, CHỈ ĐỌC) — hình dạng dây lấy từ bundle trình duyệt đã bắt
//! được. Giữ codec cục bộ để tránh phụ thuộc build-time vào `protoc` (giống lý do bản Python nêu).

use rand::RngCore;

pub const MAX_NOISE_CHUNK_PAYLOAD: usize = 65_489;
pub const MAX_NOISE_CHUNKS: u32 = 256;
pub const MAX_ASSEMBLY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("varint bi cat cut")]
    TruncatedVarint,
    #[error("varint qua dai")]
    VarintTooLong,
    #[error("truong do dai bi cat cut")]
    TruncatedLengthDelimited,
    #[error("fixed64 bi cat cut")]
    TruncatedFixed64,
    #[error("fixed32 bi cat cut")]
    TruncatedFixed32,
    #[error("wire type protobuf khong ho tro: {0}")]
    UnsupportedWireType(u64),
    #[error("payload qua lon de phan manh Noise: {0} manh")]
    PayloadTooLarge(usize),
    #[error("total_chunks khong hop le")]
    InvalidTotalChunks,
    #[error("chunk_index khong hop le")]
    InvalidChunkIndex,
    #[error("payload manh Noise qua lon")]
    FrameTooLarge,
    #[error("total_chunks khong khop giua cac manh")]
    InconsistentTotalChunks,
    #[error("manh Noise trung lap")]
    DuplicateChunk,
    #[error("ghep manh Noise vuot 16 MiB")]
    AssemblyTooLarge,
    #[error("ServiceResponse thieu payload")]
    MissingServiceResponsePayload,
}

type Result<T> = std::result::Result<T, WireError>;

#[must_use]
pub fn varint(value: u64) -> Vec<u8> {
    let mut value = value;
    let mut out = Vec::new();
    loop {
        let b = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            out.push(b | 0x80);
        } else {
            out.push(b);
            return out;
        }
    }
}

fn key(field: u64, wire_type: u64) -> Vec<u8> {
    varint((field << 3) | wire_type)
}

fn f_varint(field: u64, value: u64) -> Vec<u8> {
    let mut out = key(field, 0);
    out.extend(varint(value));
    out
}

fn f_bytes(field: u64, value: &[u8]) -> Vec<u8> {
    let mut out = key(field, 2);
    out.extend(varint(value.len() as u64));
    out.extend_from_slice(value);
    out
}

fn f_string(field: u64, value: &str) -> Vec<u8> {
    f_bytes(field, value.as_bytes())
}

fn f_bool(field: u64, value: bool) -> Vec<u8> {
    f_varint(field, u64::from(value))
}

fn read_varint(data: &[u8], pos: usize) -> Result<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut pos = pos;
    loop {
        let b = *data.get(pos).ok_or(WireError::TruncatedVarint)?;
        pos += 1;
        value |= u64::from(b & 0x7F) << shift;
        if b & 0x80 == 0 {
            return Ok((value, pos));
        }
        shift += 7;
        if shift > 70 {
            return Err(WireError::VarintTooLong);
        }
    }
}

enum FieldValue<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

/// Duyệt các trường wire-format top-level của `data`, gọi `visit(field_num, value)` cho mỗi
/// trường — tránh cấp phát `Vec` trung gian như bản Python (iterator) vì Rust không có generator
/// ẩn danh gọn tương đương; dùng closure thay thế.
fn walk_fields<'a>(
    data: &'a [u8],
    mut visit: impl FnMut(u64, FieldValue<'a>) -> Result<()>,
) -> Result<()> {
    let mut pos = 0usize;
    while pos < data.len() {
        let (tag, next) = read_varint(data, pos)?;
        pos = next;
        let num = tag >> 3;
        let wire_type = tag & 7;
        match wire_type {
            0 => {
                let (value, next) = read_varint(data, pos)?;
                pos = next;
                visit(num, FieldValue::Varint(value))?;
            }
            2 => {
                let (length, next) = read_varint(data, pos)?;
                pos = next;
                let len =
                    usize::try_from(length).map_err(|_| WireError::TruncatedLengthDelimited)?;
                let end = pos
                    .checked_add(len)
                    .ok_or(WireError::TruncatedLengthDelimited)?;
                if end > data.len() {
                    return Err(WireError::TruncatedLengthDelimited);
                }
                visit(num, FieldValue::Bytes(&data[pos..end]))?;
                pos = end;
            }
            1 => {
                let end = pos.checked_add(8).ok_or(WireError::TruncatedFixed64)?;
                if end > data.len() {
                    return Err(WireError::TruncatedFixed64);
                }
                visit(num, FieldValue::Bytes(&data[pos..end]))?;
                pos = end;
            }
            5 => {
                let end = pos.checked_add(4).ok_or(WireError::TruncatedFixed32)?;
                if end > data.len() {
                    return Err(WireError::TruncatedFixed32);
                }
                visit(num, FieldValue::Bytes(&data[pos..end]))?;
                pos = end;
            }
            other => return Err(WireError::UnsupportedWireType(other)),
        }
    }
    Ok(())
}

// Dien giai lai bit u64 -> i64 kieu bu-2 (khop `signed64` ben Python, tu `value &= (1<<64)-1` khi
// ma hoa so am) — day la ep kieu CO CHU DICH, khong phai loi tran/mat du lieu.
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    reason = "dien giai lai bit u64 -> i64 bu-2 co chu dich, khop quy uoc protobuf sint/int64"
)]
#[must_use]
fn signed64(value: u64) -> i64 {
    value as i64
}

#[allow(
    clippy::cast_sign_loss,
    reason = "dien giai lai bit i64 -> u64 bu-2 co chu dich, chieu nguoc lai signed64"
)]
#[must_use]
fn unsigned64(value: i64) -> u64 {
    value as u64
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub key: String,
    pub value: String,
}

impl Header {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = f_string(1, &self.key);
        out.extend(f_string(2, &self.value));
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationRequest {
    pub verb: String,
    pub path: String,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
    pub end_body: bool,
}

impl ApplicationRequest {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = f_string(1, &self.verb);
        out.extend(f_string(2, &self.path));
        for header in &self.headers {
            out.extend(f_bytes(3, &header.encode()));
        }
        if !self.body.is_empty() {
            out.extend(f_bytes(4, &self.body));
        }
        out.extend(f_bool(5, self.end_body));
        out
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplicationResponse {
    pub status: i64,
    pub headers: Vec<Header>,
    pub body: Vec<u8>,
    pub end_body: bool,
}

impl ApplicationResponse {
    /// # Errors
    /// Lỗi parse wire-format của [`walk_fields`] khi `data` không phải protobuf hợp lệ.
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut status = 0i64;
        let mut headers = Vec::new();
        let mut body = Vec::new();
        let mut end_body = false;
        walk_fields(data, |num, value| {
            match (num, value) {
                (1, FieldValue::Varint(v)) => status = signed64(v),
                (2, FieldValue::Bytes(raw)) => {
                    let mut hkey = String::new();
                    let mut hvalue = String::new();
                    walk_fields(raw, |n2, v2| {
                        match (n2, v2) {
                            (1, FieldValue::Bytes(b)) => {
                                hkey = String::from_utf8_lossy(b).into_owned();
                            }
                            (2, FieldValue::Bytes(b)) => {
                                hvalue = String::from_utf8_lossy(b).into_owned();
                            }
                            _ => {}
                        }
                        Ok(())
                    })?;
                    headers.push(Header {
                        key: hkey,
                        value: hvalue,
                    });
                }
                (3, FieldValue::Bytes(raw)) => body = raw.to_vec(),
                (4, FieldValue::Varint(v)) => end_body = v != 0,
                _ => {}
            }
            Ok(())
        })?;
        Ok(Self {
            status,
            headers,
            body,
            end_body,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceFramePayload {
    Response(ApplicationResponse),
    BodyChunk { data: Vec<u8>, end_body: bool },
    Reset { code: i64, reason: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedServiceFrame {
    pub stream_id: i64,
    pub payload: ServiceFramePayload,
}

#[must_use]
pub fn encode_service_frame_request(stream_id: i64, request: &ApplicationRequest) -> Vec<u8> {
    let mut out = f_varint(1, unsigned64(stream_id));
    out.extend(f_bytes(2, &request.encode()));
    out
}

#[must_use]
pub fn encode_service_frame_body(stream_id: i64, data: &[u8], end_body: bool) -> Vec<u8> {
    let mut chunk = f_bytes(1, data);
    chunk.extend(f_bool(2, end_body));
    let mut out = f_varint(1, unsigned64(stream_id));
    out.extend(f_bytes(4, &chunk));
    out
}

/// # Errors
/// Lỗi parse wire-format của [`walk_fields`] khi `data` không phải protobuf hợp lệ.
pub fn decode_service_frame(data: &[u8]) -> Result<DecodedServiceFrame> {
    let mut stream_id = 0i64;
    let mut payload = ServiceFramePayload::Unknown;
    walk_fields(data, |num, value| {
        match (num, value) {
            (1, FieldValue::Varint(v)) => stream_id = signed64(v),
            (3, FieldValue::Bytes(raw)) => {
                payload = ServiceFramePayload::Response(ApplicationResponse::decode(raw)?);
            }
            (4, FieldValue::Bytes(raw)) => {
                let mut chunk_data = Vec::new();
                let mut end_body = false;
                walk_fields(raw, |n2, v2| {
                    match (n2, v2) {
                        (1, FieldValue::Bytes(b)) => chunk_data = b.to_vec(),
                        (2, FieldValue::Varint(v)) => end_body = v != 0,
                        _ => {}
                    }
                    Ok(())
                })?;
                payload = ServiceFramePayload::BodyChunk {
                    data: chunk_data,
                    end_body,
                };
            }
            (5, FieldValue::Bytes(raw)) => {
                let mut code = 0i64;
                let mut reason = String::new();
                walk_fields(raw, |n2, v2| {
                    match (n2, v2) {
                        (1, FieldValue::Varint(v)) => code = signed64(v),
                        (2, FieldValue::Bytes(b)) => {
                            reason = String::from_utf8_lossy(b).into_owned();
                        }
                        _ => {}
                    }
                    Ok(())
                })?;
                payload = ServiceFramePayload::Reset { code, reason };
            }
            _ => {}
        }
        Ok(())
    })?;
    Ok(DecodedServiceFrame { stream_id, payload })
}

pub const SERVICE_DAEMON: u64 = 0;
pub const SERVICE_SENTINEL: u64 = 1;
pub const SERVICE_VAULT: u64 = 2;
pub const SERVICE_AUTHD: u64 = 3;

#[must_use]
pub fn encode_service_request(service: u64, service_frame: &[u8]) -> Vec<u8> {
    let mut out = f_varint(1, service);
    out.extend(f_bytes(2, service_frame));
    out
}

/// # Errors
/// [`WireError::MissingServiceResponsePayload`] khi `data` không có trường payload (số 1, bytes).
pub fn decode_service_response(data: &[u8]) -> Result<Vec<u8>> {
    let mut found = None;
    walk_fields(data, |num, value| {
        if num == 1 {
            if let FieldValue::Bytes(raw) = value {
                found = Some(raw.to_vec());
            }
        }
        Ok(())
    })?;
    found.ok_or(WireError::MissingServiceResponsePayload)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoiseChunk {
    pub chunk_id: i64,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub payload: Vec<u8>,
}

impl NoiseChunk {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = f_varint(1, unsigned64(self.chunk_id));
        out.extend(f_varint(2, u64::from(self.chunk_index)));
        out.extend(f_varint(3, u64::from(self.total_chunks)));
        out.extend(f_bytes(4, &self.payload));
        out
    }

    /// # Errors
    /// Lỗi parse wire-format của [`walk_fields`] khi `data` không phải protobuf hợp lệ.
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut chunk_id = 0i64;
        let mut chunk_index = 0u32;
        let mut total_chunks = 1u32;
        let mut payload = Vec::new();
        walk_fields(data, |num, value| {
            match (num, value) {
                (1, FieldValue::Varint(v)) => chunk_id = signed64(v),
                // Gia tri ngoai u32 (du lieu hong/doc hai) -> ep ve MAX de buoc kiem bien sau do
                // (`total_chunks > MAX_NOISE_CHUNKS`, `chunk_index >= total_chunks`) chac chan tu
                // choi thay vi tron (wrap) thanh mot so nho trong de "hop le gia".
                (2, FieldValue::Varint(v)) => chunk_index = u32::try_from(v).unwrap_or(u32::MAX),
                (3, FieldValue::Varint(v)) => total_chunks = u32::try_from(v).unwrap_or(u32::MAX),
                (4, FieldValue::Bytes(b)) => payload = b.to_vec(),
                _ => {}
            }
            Ok(())
        })?;
        Ok(Self {
            chunk_id,
            chunk_index,
            total_chunks,
            payload,
        })
    }
}

#[must_use]
pub fn random_int64() -> i64 {
    let mut buf = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    i64::from_le_bytes(buf)
}

/// # Errors
/// [`WireError::PayloadTooLarge`] khi `data` cần nhiều hơn [`MAX_NOISE_CHUNKS`] mảnh để truyền.
pub fn split_noise_payload(data: &[u8], max_payload: usize) -> Result<Vec<Vec<u8>>> {
    let total = if data.is_empty() {
        1
    } else {
        data.len().div_ceil(max_payload)
    };
    if total > MAX_NOISE_CHUNKS as usize {
        return Err(WireError::PayloadTooLarge(total));
    }
    let chunk_id = random_int64();
    if data.is_empty() {
        return Ok(vec![
            NoiseChunk {
                chunk_id,
                chunk_index: 0,
                total_chunks: 1,
                payload: Vec::new(),
            }
            .encode(),
        ]);
    }
    Ok((0..total)
        .map(|index| {
            let start = index * max_payload;
            let end = (start + max_payload).min(data.len());
            NoiseChunk {
                chunk_id,
                // `index < total <= MAX_NOISE_CHUNKS` (256, kiem o tren) nen khong bao gio that su
                // tran — van dung try_from+unwrap_or thay vi `as`/`.expect()` de ham nay khong co
                // duong panic nao (khong can tai lieu `# Panics`).
                chunk_index: u32::try_from(index).unwrap_or(u32::MAX),
                total_chunks: u32::try_from(total).unwrap_or(u32::MAX),
                payload: data[start..end].to_vec(),
            }
            .encode()
        })
        .collect())
}

struct PendingAssembly {
    total: u32,
    parts: std::collections::BTreeMap<u32, Vec<u8>>,
    size: usize,
}

#[derive(Default)]
pub struct NoiseReassembler {
    pending: std::collections::HashMap<i64, PendingAssembly>,
}

impl NoiseReassembler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Trả `Some(data)` khi đủ mảnh để ghép lại hoàn chỉnh, `None` khi còn đang chờ mảnh khác.
    ///
    /// # Errors
    /// Các biến thể `WireError` khi mảnh không hợp lệ (số mảnh/chỉ số ngoài phạm vi, trùng lặp,
    /// không khớp `total_chunks` với mảnh trước đó, hoặc tổng kích thước ghép vượt 16 MiB).
    pub fn feed(&mut self, raw: &[u8]) -> Result<Option<Vec<u8>>> {
        let chunk = NoiseChunk::decode(raw)?;
        if chunk.total_chunks < 1 || chunk.total_chunks > MAX_NOISE_CHUNKS {
            return Err(WireError::InvalidTotalChunks);
        }
        if chunk.chunk_index >= chunk.total_chunks {
            return Err(WireError::InvalidChunkIndex);
        }
        if chunk.payload.len() > MAX_NOISE_CHUNK_PAYLOAD {
            return Err(WireError::FrameTooLarge);
        }
        let entry = self
            .pending
            .entry(chunk.chunk_id)
            .or_insert_with(|| PendingAssembly {
                total: chunk.total_chunks,
                parts: std::collections::BTreeMap::new(),
                size: 0,
            });
        if entry.total != chunk.total_chunks {
            self.pending.remove(&chunk.chunk_id);
            return Err(WireError::InconsistentTotalChunks);
        }
        if entry.parts.contains_key(&chunk.chunk_index) {
            self.pending.remove(&chunk.chunk_id);
            return Err(WireError::DuplicateChunk);
        }
        entry.size += chunk.payload.len();
        if entry.size > MAX_ASSEMBLY_BYTES {
            self.pending.remove(&chunk.chunk_id);
            return Err(WireError::AssemblyTooLarge);
        }
        entry.parts.insert(chunk.chunk_index, chunk.payload);
        // `entry.parts.len()` bi chan boi `entry.total <= MAX_NOISE_CHUNKS` (256, kiem o tren) nen
        // khong bao gio that su tran khi ep ve u32.
        if u32::try_from(entry.parts.len()).unwrap_or(u32::MAX) != entry.total {
            return Ok(None);
        }
        // Lay du lieu truc tiep tu `entry` (dang muon) thay vi `remove` roi `.expect()` ton tai —
        // tranh hoan toan duong panic, ham nay khong can tai lieu `# Panics`.
        let parts = std::mem::take(&mut entry.parts);
        self.pending.remove(&chunk.chunk_id);
        Ok(Some(parts.into_values().flatten().collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip_cac_gia_tri_moc() {
        for v in [0u64, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
            let encoded = varint(v);
            let (decoded, pos) = read_varint(&encoded, 0).unwrap();
            assert_eq!(decoded, v);
            assert_eq!(pos, encoded.len());
        }
    }

    #[test]
    fn application_request_response_round_trip() {
        let req = ApplicationRequest {
            verb: "POST".into(),
            path: "/chat/stream".into(),
            headers: vec![Header {
                key: "content-type".into(),
                value: "application/json".into(),
            }],
            body: b"{\"x\":1}".to_vec(),
            end_body: true,
        };
        let frame = encode_service_frame_request(7, &req);
        let decoded = decode_service_frame(&frame).unwrap();
        assert_eq!(decoded.stream_id, 7);
        // encode_service_frame_request chi ma hoa field request (so 2), decode_service_frame hien
        // tai chi hieu field 3 la Response/4 la BodyChunk/5 la Reset -> payload Unknown la dung (vi
        // request KHONG phai mot trong ba kieu frame server->client nay, dung lam test khong-vo).
        assert_eq!(decoded.payload, ServiceFramePayload::Unknown);
    }

    #[test]
    fn application_response_decode_dung_status_header_body() {
        let resp = ApplicationResponse {
            status: 200,
            headers: vec![Header {
                key: "x".into(),
                value: "y".into(),
            }],
            body: b"hello".to_vec(),
            end_body: true,
        };
        let mut encoded = f_varint(1, 200);
        for h in &resp.headers {
            encoded.extend(f_bytes(2, &h.encode()));
        }
        encoded.extend(f_bytes(3, &resp.body));
        encoded.extend(f_bool(4, resp.end_body));
        let decoded = ApplicationResponse::decode(&encoded).unwrap();
        assert_eq!(decoded, resp);
    }

    #[test]
    fn noise_chunk_round_trip() {
        let chunk = NoiseChunk {
            chunk_id: -42,
            chunk_index: 1,
            total_chunks: 3,
            payload: vec![1, 2, 3],
        };
        let encoded = chunk.encode();
        let decoded = NoiseChunk::decode(&encoded).unwrap();
        assert_eq!(decoded, chunk);
    }

    #[test]
    fn split_va_ghep_lai_payload_nho_hon_mot_manh() {
        let data = b"xin chao the gioi";
        let chunks = split_noise_payload(data, 1024).unwrap();
        assert_eq!(chunks.len(), 1);
        let mut reassembler = NoiseReassembler::new();
        let out = reassembler.feed(&chunks[0]).unwrap();
        assert_eq!(out.unwrap(), data);
    }

    #[test]
    fn split_va_ghep_lai_payload_nhieu_manh_bat_ky_thu_tu() {
        let data: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
        let chunks = split_noise_payload(&data, 4096).unwrap();
        assert!(chunks.len() > 1);
        let mut reassembler = NoiseReassembler::new();
        let mut result = None;
        // Dua vao theo thu tu DAO NGUOC — ghep phai khong phu thuoc thu tu toi.
        for raw in chunks.iter().rev() {
            result = reassembler.feed(raw).unwrap();
        }
        assert_eq!(result.unwrap(), data);
    }

    #[test]
    fn split_payload_rong_tra_dung_1_manh_rong() {
        let chunks = split_noise_payload(b"", 1024).unwrap();
        assert_eq!(chunks.len(), 1);
        let decoded = NoiseChunk::decode(&chunks[0]).unwrap();
        assert!(decoded.payload.is_empty());
        assert_eq!(decoded.total_chunks, 1);
    }

    #[test]
    fn truncated_varint_bao_loi_khong_panic() {
        let err = read_varint(&[0x80], 0).unwrap_err();
        assert_eq!(err, WireError::TruncatedVarint);
    }

    #[test]
    fn service_request_response_round_trip() {
        let inner = b"noi dung ben trong".to_vec();
        let encoded = encode_service_request(SERVICE_AUTHD, &inner);
        // decode_service_response doc field 1 (truoc do la service id varint, field DUNG cho
        // response la field 1 KIEU bytes trong ham nay theo dung quy uoc Python goc).
        let resp_encoded = f_bytes(1, &inner);
        let decoded = decode_service_response(&resp_encoded).unwrap();
        assert_eq!(decoded, inner);
        // encoded (request) khong co field 1 kieu bytes (field 1 la varint service id) nen decode
        // nhu response se khong tim thay payload -> loi ro rang, khong doc nham du lieu.
        assert!(decode_service_response(&encoded).is_err());
    }
}
