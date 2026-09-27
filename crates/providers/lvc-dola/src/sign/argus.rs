//! X-Argus — port `x_signatures/lib/XArgus.py`.
//!
//! Dựng `argus_bean` (protobuf) → đệm PKCS7 (block 16) → mã hoá Simon (khoá từ hằng `SM3_OUTPUT`) →
//! `encrypt_enc_pb` (XOR theo 8 byte đầu rồi ĐẢO NGƯỢC) → bọc byte cố định → AES-CBC (key = md5
//! `SIGN_KEY[:16]`, iv = md5 `SIGN_KEY[16:]`) → prepend `\xf2\x81` → base64.
//! `SIGN_KEY`/`SM3_OUTPUT` là HẰNG cứng chép nguyên từ bản Python.

use aes::Aes128;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use cbc::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};

use super::protobuf::{ProtoVal, protobuf_encode};
use super::simon::simon_enc;
use super::sm3::sm3_hash;
use super::{le_u64, md5_bytes};

type Aes128CbcEnc = cbc::Encryptor<Aes128>;

/// `sign_key` — hằng cứng (32 byte).
const SIGN_KEY: [u8; 32] = [
    0xac, 0x1a, 0xda, 0xae, 0x95, 0xa7, 0xaf, 0x94, 0xa5, 0x11, 0x4a, 0xb3, 0xb3, 0xa9, 0x7d, 0xd8,
    0x00, 0x50, 0xaa, 0x0a, 0x39, 0x31, 0x4c, 0x40, 0x52, 0x8c, 0xae, 0xc9, 0x52, 0x56, 0xc2, 0x8c,
];

/// `sm3_output` — hằng cứng (32 byte) = `sm3_hash(sign_key + b'\xf2\x81ao' + sign_key)`.
const SM3_OUTPUT: [u8; 32] = [
    0xfc, 0x78, 0xe0, 0xa9, 0x65, 0x7a, 0x0c, 0x74, 0x8c, 0xe5, 0x15, 0x59, 0x90, 0x3c, 0xcf, 0x03,
    0x51, 0x0e, 0x51, 0xd3, 0xcf, 0xf2, 0x32, 0xd7, 0x13, 0x43, 0xe8, 0x8a, 0x32, 0x1c, 0x53, 0x04,
];

/// `magic = 0x20200929 << 1`.
const MAGIC: u64 = 0x2020_0929 << 1;

/// `encrypt_enc_pb`: XOR các byte từ vị trí 8 theo 8 byte đầu, rồi ĐẢO NGƯỢC toàn bộ.
#[allow(
    clippy::needless_range_loop,
    reason = "can chi so i cho xor_array[i%8] va sua tai cho"
)]
fn encrypt_enc_pb(data: &[u8]) -> Vec<u8> {
    let mut d = data.to_vec();
    let xor: [u8; 8] = d[..8].try_into().unwrap_or([0u8; 8]);
    for i in 8..d.len() {
        d[i] ^= xor[i % 8];
    }
    d.reverse();
    d
}

/// `bodyHash` = 6 byte đầu SM3 của stub (hex) hoặc của `bytes(16)` khi rỗng.
fn get_bodyhash(stub: Option<&str>) -> Vec<u8> {
    let bytes = match stub {
        Some(s) if !s.is_empty() => hex::decode(s).unwrap_or_default(),
        _ => vec![0u8; 16],
    };
    sm3_hash(&bytes)[..6].to_vec()
}

/// `queryHash` = 6 byte đầu SM3 của params hoặc của `bytes(16)` khi rỗng.
fn get_queryhash(query: &str) -> Vec<u8> {
    if query.is_empty() {
        sm3_hash(&[0u8; 16])[..6].to_vec()
    } else {
        sm3_hash(query.as_bytes())[..6].to_vec()
    }
}

/// `calculate_constant`: bỏ dấu chấm, chèn 0 bên trái cho đủ 6 chữ số, nhân trọng số rồi cộng.
fn calc_constant(os_version: &str) -> u64 {
    let stripped: String = os_version.chars().filter(|c| *c != '.').collect();
    let padded = format!("{stripped:0>6}");
    let weights: [u64; 6] = [
        20480,
        2048,
        20_971_520,
        2_097_152,
        1_342_177_280,
        134_217_728,
    ];
    padded
        .chars()
        .zip(weights.iter())
        .map(|(c, w)| u64::from(c.to_digit(10).unwrap_or(0)) * w)
        .sum()
}

/// Giải mã percent-encoding của một giá trị query (giống `urllib.parse.parse_qs`: `+` → khoảng trắng).
fn pct_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex_val = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                    out.push(h * 16 + l);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Lấy giá trị đầu tiên của `key` trong query (bỏ giá trị rỗng như `parse_qs` mặc định).
fn qs_get(params: &str, key: &str) -> String {
    for pair in params.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key && !v.is_empty() {
                return pct_decode(v);
            }
        }
    }
    String::new()
}

/// Khoá/iv AES của Argus = md5(`SIGN_KEY[:16]`), md5(`SIGN_KEY[16:]`). Public để kiểm thử vector.
#[must_use]
pub fn aes_key_iv() -> ([u8; 16], [u8; 16]) {
    (md5_bytes(&SIGN_KEY[0..16]), md5_bytes(&SIGN_KEY[16..32]))
}

/// Dựng `argus_bean` theo đúng thứ tự field của bản Python.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "gom du tham so ky Argus giong chu ky get_sign() ben Python — tach ra se lam kho doi chieu"
)]
pub fn build_bean(
    params: &str,
    stub: Option<&str>,
    timestamp: u64,
    aid: u64,
    license_id: u64,
    platform: u64,
    sec_device_id: &str,
    sdk_version: &str,
    sdk_version_int: u64,
    rand_field3: u32,
) -> Vec<(u32, ProtoVal)> {
    let device_id = qs_get(params, "device_id");
    let version_name = qs_get(params, "version_name");
    let device_type = qs_get(params, "device_type");
    let os_version = qs_get(params, "os_version");

    let mut bean: Vec<(u32, ProtoVal)> = vec![
        (1, ProtoVal::Varint(MAGIC)),
        (2, ProtoVal::Varint(2)),
        (3, ProtoVal::Varint(u64::from(rand_field3))),
        (4, ProtoVal::Str(aid.to_string().into_bytes())),
        (5, ProtoVal::Str(device_id.into_bytes())),
        (6, ProtoVal::Str(license_id.to_string().into_bytes())),
        (7, ProtoVal::Str(version_name.into_bytes())),
        (8, ProtoVal::Str(sdk_version.as_bytes().to_vec())),
        (9, ProtoVal::Varint(sdk_version_int)),
        (10, ProtoVal::Str(vec![0u8; 8])),
        (11, ProtoVal::Varint(platform)),
        (12, ProtoVal::Varint(timestamp << 1)),
        (13, ProtoVal::Str(get_bodyhash(stub))),
        (14, ProtoVal::Str(get_queryhash(params))),
    ];
    if !sec_device_id.is_empty() {
        bean.push((16, ProtoVal::Str(sec_device_id.as_bytes().to_vec())));
    }
    bean.push((20, ProtoVal::Varint(738)));
    bean.push((
        23,
        ProtoVal::Nested(vec![
            (1, ProtoVal::Str(device_type.into_bytes())),
            (2, ProtoVal::Str(os_version.clone().into_bytes())),
            (3, ProtoVal::Str(b"googleplay".to_vec())),
            (4, ProtoVal::Varint(calc_constant(&os_version))),
        ]),
    ));
    bean
}

/// Tính chuỗi X-Argus.
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "gom du tham so ky Argus giong chu ky get_sign() ben Python — tach ra se lam kho doi chieu"
)]
pub fn xargus(
    params: &str,
    stub: Option<&str>,
    timestamp: u64,
    aid: u64,
    license_id: u64,
    platform: u64,
    sec_device_id: &str,
    sdk_version: &str,
    sdk_version_int: u64,
    rand_field3: u32,
) -> String {
    let bean = build_bean(
        params,
        stub,
        timestamp,
        aid,
        license_id,
        platform,
        sec_device_id,
        sdk_version,
        sdk_version_int,
        rand_field3,
    );
    let pb = protobuf_encode(&bean);
    encrypt_bean(&pb)
}

/// Bước mã hoá cuối cùng của Argus (tách riêng để test dựa vào `bean_protobuf_hex`).
#[must_use]
#[allow(
    clippy::needless_range_loop,
    reason = "xu ly theo tung khoi 16 byte bang chi so blk vao padded/enc_pb"
)]
pub fn encrypt_bean(pb: &[u8]) -> String {
    let padded = super::pkcs7::pkcs7_pad(pb, 16);
    let new_len = padded.len();

    let key_list = [
        le_u64(&SM3_OUTPUT[0..8]),
        le_u64(&SM3_OUTPUT[8..16]),
        le_u64(&SM3_OUTPUT[16..24]),
        le_u64(&SM3_OUTPUT[24..32]),
    ];

    let mut enc_pb = vec![0u8; new_len];
    for blk in 0..new_len / 16 {
        let pt = [
            le_u64(&padded[blk * 16..blk * 16 + 8]),
            le_u64(&padded[blk * 16 + 8..blk * 16 + 16]),
        ];
        let ct = simon_enc(pt, &key_list);
        enc_pb[blk * 16..blk * 16 + 8].copy_from_slice(&ct[0].to_le_bytes());
        enc_pb[blk * 16 + 8..blk * 16 + 16].copy_from_slice(&ct[1].to_le_bytes());
    }

    let mut pre = vec![0xf2, 0xf7, 0xfc, 0xff, 0xf2, 0xf7, 0xfc, 0xff];
    pre.extend_from_slice(&enc_pb);
    let b_buffer = encrypt_enc_pb(&pre);

    let mut wrapped = vec![0xa6, 0x6e, 0xad, 0x9f, 0x77, 0x01, 0xd0, 0x0c, 0x18];
    wrapped.extend_from_slice(&b_buffer);
    wrapped.extend_from_slice(b"ao");

    let key = md5_bytes(&SIGN_KEY[0..16]);
    let iv = md5_bytes(&SIGN_KEY[16..32]);
    let ct =
        Aes128CbcEnc::new((&key).into(), (&iv).into()).encrypt_padded_vec_mut::<Pkcs7>(&wrapped);

    let mut out = vec![0xf2, 0x81];
    out.extend_from_slice(&ct);
    B64.encode(&out)
}
