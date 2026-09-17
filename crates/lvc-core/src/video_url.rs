//! Chọn URL video "raw" (không logo "Dola AI") từ `video_model` (chuỗi JSON nhúng trong
//! creation block) — bản Rust của `src/media/unwatermark.py` (repo Python `lvcseedance`,
//! v0.55b 2026-09-15) cộng 2 hàm phụ trợ `extract_unwatermarked_url` (`video_worker.py:118-141`)
//! và `is_watermarked_url` (`postprocess.py:58-63`). Thuần — không I/O (Task 10 gọi
//! `fetch_raw_video` bọc HTTP quanh `find_fallback_apis`/`pick_raw_video` ở đây).
//!
//! # Tổng quan luồng (đúng docstring đầu `unwatermark.py`)
//! `video_model` (JSON) → [`find_fallback_apis`] tìm URL `fallback_api` → (Task 10 GET URL đó
//! với `RAW_PARAMS`) → JSON trả về đưa vào [`pick_raw_video`] → mỗi `main_url` giải mã bằng
//! [`decode_main_url`] (base64 thường, hoặc token `qAAB…` mã hoá AES-128-CBC) → chọn bản có
//! điểm `bitrate + w*h` cao nhất.
//!
//! # Đính chính so với `task-3-brief.md`
//! Brief tóm tắt qAAB trong 1 dòng; nguồn thật là `unwatermark.py:153-171` `decode_qaab` —
//! đã đọc và port từng hàm (không chỉ công thức tóm tắt), gồm CẢ các lượt thử phụ (key/iv hoán
//! đổi, offset 36 byte) — xem hàm nội bộ (không public) cùng tên định nghĩa bên dưới.
//!
//! Brief bước 1(a) mô tả test `find_fallback_apis` là "2 `fallback_api` (1 host đúng, 1 host
//! khác) → CHỈ host đúng, giữ thứ tự". Đã xác minh bằng Python thật (`.venv`, chỉ đọc, xem lệnh
//! trong test `tim_ca_hai_fallback_api_khong_loc_host`): hàm `find_fallback_apis`
//! (`unwatermark.py:51-83`) KHÔNG lọc theo host — `add(u)` chỉ kiểm `u.startswith(("http://",
//! "https://"))` và khử trùng, không có điều kiện host nào. Theo đúng chỉ dẫn "Python là nguồn
//! xác thực, lệch thì theo Python": test ở đây ghim ĐÚNG hành vi đã xác minh (giữ CẢ HAI URL,
//! đúng thứ tự xuất hiện), không phải mô tả (sai) của brief.

use std::sync::LazyLock;

use aes::Aes128;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use cbc::cipher::block_padding::NoPadding;
use cbc::cipher::{BlockDecryptMut, KeyIvInit};
use regex::Regex;
use serde_json::{Map, Value};
use sha2::{Digest, Sha512};

/// `unwatermark.py:26` `RAW_PARAMS` — tham số ép `fallback_api` trả bản KHÔNG logo (giống hệt
/// extension: `channel=no&codec_type=8&logo_type=unwatermarked`). Dùng `Vec`/mảng thay `dict`
/// để GIỮ THỨ TỰ (Task 10 gọi `replace_query(fallback_api, RAW_PARAMS)`).
pub const RAW_PARAMS: [(&str, &str); 3] = [
    ("channel", "no"),
    ("codec_type", "8"),
    ("logo_type", "unwatermarked"),
];

/// `unwatermark.py:24-25` `QAAB_SALT_HEX` — CHÉP NGUYÊN VĂN (nối 4 đoạn hex, không đổi 1 ký tự).
const QAAB_SALT_HEX: &str = concat!(
    "4dd4c2e6b83162090e52b3c7a6733ba4",
    "1cb2462b829ab58a196b39db57177524",
    "f49baf7f08e8d68d26a72e37c1a95a2f",
    "1f05a51892aef2949732b62a38aadd58"
);

/// `unwatermark.py:28` `_FALLBACK_RE` — chép nguyên pattern (cú pháp regex Python/Rust giống
/// hệt cho các token dùng ở đây: `\\`, `\s`, `.` không `DOTALL`).
static FALLBACK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"fallback_api\\*"\s*:\s*\\*"(.*?)\\*""#).expect("FALLBACK_RE hợp lệ")
});

/// `unwatermark.py:91` nhánh query-string của `find_key_seed`.
static KEY_SEED_QUERY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:^|[?&])key_seed=([^&"'<>\\\s]+)"#).expect("KEY_SEED_QUERY_RE hợp lệ")
});

/// `unwatermark.py:91` nhánh JSON-fragment của `find_key_seed`.
static KEY_SEED_JSON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["']key_seed["']\s*:\s*["']([^"']+)"#).expect("KEY_SEED_JSON_RE hợp lệ")
});

static QAAB_SALT: LazyLock<Vec<u8>> =
    LazyLock::new(|| hex_decode(QAAB_SALT_HEX).expect("QAAB_SALT_HEX hợp lệ (đã chép nguyên văn)"));

/// Bản tốt nhất của một rendition `video_list` sau khi đã giải mã `main_url` —
/// `unwatermark.py:202-222` `pick_raw_video` (phần thân vòng lặp, trước khi rút gọn về
/// `dict {url,codec,width,height,size}`). Public vì `task-3-brief.md` liệt kê trong interface;
/// dùng nội bộ để tính điểm `bitrate + w*h` (`_score`, `unwatermark.py:189-200`) tách khỏi việc
/// đọc JSON, dễ test riêng phần "chọn max" mà không cần dựng `serde_json::Value` đầy đủ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawCandidate {
    pub url: String,
    pub bitrate: u64,
    pub w: u64,
    pub h: u64,
}

impl RawCandidate {
    /// `unwatermark.py:189-200` `_score` — `n("bitrate","real_bitrate") + n("vwidth","width") *
    /// n("vheight","height")`; ở đây `bitrate`/`w`/`h` đã được tính sẵn theo đúng công thức đó
    /// khi dựng `RawCandidate` (xem `entry_score` + `pick_raw_video`), nên điểm chỉ còn
    /// `bitrate + w*h`. Dùng `saturating_*` — không panic dù dữ liệu JSON bất thường/rất lớn.
    #[must_use]
    fn score(&self) -> u64 {
        self.bitrate.saturating_add(self.w.saturating_mul(self.h))
    }
}

// ===== `replace_query` (unwatermark.py:31) — mini urlsplit/parse_qsl/urlencode thuần =====
//
// lvc-core không được thêm crate `url` (phải giữ THUẦN, xem rằng buộc `cargo tree` trong
// scripts/check.ps1) nên tự viết bộ tách/ráp URL rút gọn — CHỈ đủ dùng cho link Dola dạng
// `scheme://host/path?query#frag` (giống mini splitter `split_netloc_path` trong chain.rs, mở
// rộng thêm query/fragment). Không phải parser URL tổng quát.

struct SplitUrl<'a> {
    scheme: &'a str,
    netloc: &'a str,
    path: &'a str,
    query: &'a str,
    fragment: &'a str,
}

/// Tách thô kiểu `urllib.parse.urlsplit` — thứ tự tách: fragment trước (`#`), rồi query (`?`),
/// rồi scheme (`://`) + netloc (đến `/` đầu tiên).
fn urlsplit(url: &str) -> SplitUrl<'_> {
    let (before_fragment, fragment) = match url.find('#') {
        Some(idx) => (&url[..idx], &url[idx + 1..]),
        None => (url, ""),
    };
    let (before_query, query) = match before_fragment.find('?') {
        Some(idx) => (&before_fragment[..idx], &before_fragment[idx + 1..]),
        None => (before_fragment, ""),
    };
    let (scheme, rest) = match before_query.find("://") {
        Some(idx) => (&before_query[..idx], &before_query[idx + 3..]),
        None => ("", before_query),
    };
    let (netloc, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    SplitUrl {
        scheme,
        netloc,
        path,
        query,
        fragment,
    }
}

/// Ráp lại kiểu `urllib.parse.urlunsplit` — bỏ `?`/`#` khi phần tương ứng rỗng (khớp Python).
fn urlunsplit(parts: &SplitUrl<'_>, query: &str) -> String {
    let mut out = String::new();
    if !parts.scheme.is_empty() {
        out.push_str(parts.scheme);
        out.push(':');
    }
    if !parts.netloc.is_empty() {
        out.push_str("//");
        out.push_str(parts.netloc);
    }
    out.push_str(parts.path);
    if !query.is_empty() {
        out.push('?');
        out.push_str(query);
    }
    if !parts.fragment.is_empty() {
        out.push('#');
        out.push_str(parts.fragment);
    }
    out
}

/// Percent-decode tối giản (giống `urllib.parse.unquote`: byte lỗi UTF-8 → thay thế, không
/// panic). Dùng cho cả `unquote_plus` (query string) lẫn `find_key_seed` (`unquote`, không có
/// bước `+`→space).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = char::from(bytes[i + 1]).to_digit(16);
            let lo = char::from(bytes[i + 2]).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "hi,lo trong 0..=15 (to_digit(16)) nên (hi<<4)|lo luôn <=255, ep u8 an toan"
                )]
                out.push(((hi << 4) | lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `urllib.parse.unquote_plus` — `+` → space rồi percent-decode.
fn unquote_plus(s: &str) -> String {
    percent_decode(&s.replace('+', " "))
}

/// `urllib.parse.quote_plus` (safe mặc định `""`, encoding UTF-8): giữ nguyên chữ/số/`_.-~`,
/// space → `+`, còn lại `%XX` (theo byte UTF-8).
fn quote_plus(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'~' => {
                out.push(char::from(b));
            }
            b' ' => out.push('+'),
            _ => {
                // `write!` vao String khong bao gio loi -> `let _ =` bo qua Result (khong panic).
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

/// `urllib.parse.parse_qsl(query, keep_blank_values=True)` — tách theo `&`, bỏ đoạn RỖNG (vd
/// `"a=1&&b=2"` chỉ ra 2 cặp, đã xác minh bằng Python thật), cặp không có `=` → value rỗng.
fn parse_qsl(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.find('=') {
            Some(idx) => (unquote_plus(&pair[..idx]), unquote_plus(&pair[idx + 1..])),
            None => (unquote_plus(pair), String::new()),
        })
        .collect()
}

/// `urllib.parse.urlencode(dict)` trên `Vec<(String,String)>` (giữ thứ tự thay vì dict/HashMap).
fn urlencode_pairs(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `unwatermark.py:31-35` `replace_query` — giữ query cũ (đúng thứ tự xuất hiện), ghi đè giá trị
/// của khoá đã có, khoá mới (trong `params`, chưa có trong query) nối thêm vào cuối theo đúng
/// thứ tự trong `params` (giống Python: `dict.update` không đổi vị trí khoá đã tồn tại).
#[must_use]
pub fn replace_query(url: &str, params: &[(&str, &str)]) -> String {
    let parts = urlsplit(url);
    let mut pairs = parse_qsl(parts.query);
    for &(k, v) in params {
        if let Some(existing) = pairs.iter_mut().find(|(pk, _)| pk == k) {
            existing.1 = v.to_string();
        } else {
            pairs.push((k.to_string(), v.to_string()));
        }
    }
    let query = urlencode_pairs(&pairs);
    urlunsplit(&parts, &query)
}

// ===== `find_fallback_apis` (unwatermark.py:38-83) =====

/// `unwatermark.py:38-48` `_unescape_json_fragment` — JSON-decode tối đa 3 lượt (dừng sớm nếu
/// không đổi hoặc lỗi), rồi thay nốt 2 chuỗi thoát còn sót (`\u0026`, `\/`).
fn unescape_json_fragment(text: &str) -> String {
    let mut out = text.to_string();
    for _ in 0..3 {
        let escaped = out.replace('"', "\\\"");
        let wrapped = format!("\"{escaped}\"");
        match serde_json::from_str::<String>(&wrapped) {
            Ok(dec) if dec == out => break,
            Ok(dec) => out = dec,
            Err(_) => break,
        }
    }
    out.replace("\\u0026", "&").replace("\\/", "/")
}

fn is_http_url(u: &str) -> bool {
    u.starts_with("http://") || u.starts_with("https://")
}

fn add_fallback(found: &mut Vec<String>, raw: &str) {
    let u = unescape_json_fragment(raw);
    if is_http_url(&u) && !found.contains(&u) {
        found.push(u);
    }
}

/// `unwatermark.py:60-81` hàm lồng `walk` của `find_fallback_apis` — đệ quy JSON/chuỗi JSON
/// lồng nhau, giới hạn độ sâu 12 (`depth > 12` thì dừng, giống Python).
fn walk_fallback(v: &Value, depth: u32, found: &mut Vec<String>) {
    if depth > 12 {
        return;
    }
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') || trimmed.starts_with('[') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    walk_fallback(&parsed, depth + 1, found);
                }
            }
            for cap in FALLBACK_RE.captures_iter(s) {
                if let Some(m) = cap.get(1) {
                    add_fallback(found, m.as_str());
                }
            }
        }
        Value::Object(map) => {
            if let Some(fa) = map.get("fallback_api") {
                if is_truthy_json(Some(fa)) {
                    add_fallback(found, &json_display(fa));
                }
            }
            for x in map.values() {
                walk_fallback(x, depth + 1, found);
            }
        }
        Value::Array(arr) => {
            for x in arr {
                walk_fallback(x, depth + 1, found);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// `unwatermark.py:51-83` `find_fallback_apis` — mọi URL `fallback_api` (khoá dict hoặc khớp
/// `_FALLBACK_RE` trong chuỗi JSON lồng nhau), khử trùng, GIỮ THỨ TỰ xuất hiện. `video_model`
/// LUÔN là chuỗi (JSON) ở tầng gọi thật (creation block) — tương ứng lượt gọi `walk(value)` đầu
/// tiên của Python với `value` là chuỗi.
#[must_use]
pub fn find_fallback_apis(video_model: &str) -> Vec<String> {
    let mut found = Vec::new();
    walk_fallback(&Value::String(video_model.to_string()), 0, &mut found);
    found
}

// ===== `find_key_seed` (unwatermark.py:86-108) =====

fn json_display(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn is_truthy_json(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        #[allow(
            clippy::float_cmp,
            reason = "so voi 0.0 chinh xac (khong tinh toan trung gian) de khop Python `if number:`"
        )]
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn walk_key_seed(v: &Value, depth: u32) -> Option<String> {
    if depth > 10 {
        return None;
    }
    match v {
        Value::String(s) => {
            let cap = KEY_SEED_QUERY_RE
                .captures(s)
                .or_else(|| KEY_SEED_JSON_RE.captures(s))?;
            let raw = cap.get(1)?.as_str();
            Some(percent_decode(raw))
        }
        Value::Object(map) => {
            if let Some(Value::String(ks)) = map.get("key_seed") {
                let trimmed = ks.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            map.values().find_map(|x| walk_key_seed(x, depth + 1))
        }
        Value::Array(arr) => arr.iter().find_map(|x| walk_key_seed(x, depth + 1)),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

/// `unwatermark.py:86-108` `find_key_seed` — dò `key_seed` trong `payload` (đệ quy JSON, sâu tối
/// đa 10), rồi fallback dò trong `url` (cùng 2 regex, coi `url` như MỘT chuỗi).
fn find_key_seed(payload: &Value, url: &str) -> Option<String> {
    walk_key_seed(payload, 0).or_else(|| walk_key_seed(&Value::String(url.to_string()), 0))
}

// ===== `b64_loose` / `ascii_url` (unwatermark.py:110-129) =====

/// Áp 1 bảng thay ký tự (giống `str.translate(str.maketrans({...}))` của Python).
fn translate(s: &str, map: &[(char, char)]) -> String {
    s.chars()
        .map(|c| {
            map.iter()
                .find(|&&(from, _)| from == c)
                .map_or(c, |&(_, to)| to)
        })
        .collect()
}

/// Giữ lại đúng bảng chữ base64 chuẩn (`A-Za-z0-9+/`) — mô phỏng `CPython`
/// `base64.b64decode(c)` với `validate=False` (mặc định): hàm C `binascii.a2b_base64` ÂM THẦM BỎ
/// (không lỗi) mọi ký tự KHÔNG thuộc bảng chữ base64 TRƯỚC khi ráp lại thành nhóm 4 ký tự để giải
/// mã — đã xác nhận bằng Python thật (`.venv`, chỉ đọc):
/// `base64.b64decode(b"aGVs.bG8=")` → `b"hello"` (dấu `.` biến mất, phần còn lại `"aGVsbG8="` là
/// base64 hợp lệ của `"hello"`). Ký tự `=` cũng bị loại ở đây (Fix round 1, RULING): `CPython` coi
/// `=` là ký hiệu đệm CÓ Ý NGHĨA RIÊNG (dừng/định vị nhóm 4), khác hẳn ký tự lạ bị bỏ đơn thuần —
/// hành vi đó (đệm `=` giữa chuỗi) là edge case bệnh hoạn không xảy ra với dữ liệu Dola thật, nên
/// coi `=` như MỌI ký tự lạ khác (loại bỏ) rồi giải mã bằng biến thể KHÔNG ĐỆM (`STANDARD_NO_PAD`)
/// — không cần tính lại số `=` cần thêm như Python (Python tính đệm DỰA TRÊN độ dài THÔ trước khi
/// lọc, có thể ra SAI số đệm nếu chuỗi có ký tự lạ, gây lỗi `Incorrect padding` ở biến thể mà bản
/// gốc lẽ ra đã giải được nếu lọc trước — bản Rust ở đây, nhờ lọc-rồi-mới-đệm, THỰC TẾ giải mã
/// được ở biến thể SỚM HƠN Python trong một số trường hợp, xem test
/// `alt_alphabet_dola_thay_the_dola_qua_bien_the` và phần "Fix round 1" trong `task-3-report.md`
/// để biết vì sao bài toán "chỉ biến thể 2 thắng" không thể tái lập ĐÚNG NGHĨA ĐEN dưới thiết kế
/// lọc-trước-rồi-giải-KHÔNG-ĐỆM này — mọi input chỉ chứa `$`/`@`/`#` khiến biến thể 1 và biến thể
/// 2 LUÔN cùng thành/bại (chứng minh: `#` luôn biến mất ở CẢ HAI biến thể vì `.` và `=` đều ngoài
/// bảng chữ giữ lại; `$`/`@` luôn được GIỮ ở cả hai biến thể — chỉ khác giá trị ký tự cuối cùng
/// `/`(63) so với `+`(62), mà 2 giá trị này có 4 bit thấp lần lượt `1111`/`1110` — CẢ HAI đều khác
/// 0 nên đều KHÔNG hợp lệ như ký tự cuối một nhóm dư 2 hoặc dư 3, tức không bao giờ đúng lúc một
/// biến thể qua còn biến thể kia rớt vì lý do bit-đuôi).
fn keep_base64_alphabet(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/')
        .collect()
}

/// `unwatermark.py:110-122` `b64_loose` — Fix round 1 (RULING, thay bản trước): với MỖI biến thể
/// (3 cách thay `$`/`@`/`#`, giống Python), sau khi thay `-`→`+`, `_`→`/`: (a) lọc chỉ giữ
/// `[A-Za-z0-9+/]` (bỏ `=` và mọi ký tự khác — xem [`keep_base64_alphabet`]), (b) giải bằng
/// `STANDARD_NO_PAD` (không quan tâm đệm); lỗi (vd độ dài dư 1 sau khi lọc — `CPython` cũng báo
/// "Incorrect padding"/"cannot be 1 more than a multiple of 4" cho trường hợp này) → thử biến thể
/// kế tiếp. Bỏ hẳn lượt thử `URL_SAFE_NO_PAD` (đã tự chuẩn hoá `-`/`_` ở bước trên, thử thêm bảng
/// url-safe là thừa).
fn b64_loose(text: &str) -> Option<Vec<u8>> {
    let s = text.trim();
    if s.is_empty() {
        return None;
    }
    let variants = [
        s.to_string(),
        translate(s, &[('$', '_'), ('@', '/'), ('#', '.')]),
        translate(s, &[('$', '+'), ('@', '/'), ('#', '=')]),
    ];
    for cand in &variants {
        if cand.is_empty() {
            continue;
        }
        let normalized = cand.replace('-', "+").replace('_', "/");
        let filtered = keep_base64_alphabet(&normalized);
        if filtered.is_empty() {
            continue;
        }
        if let Ok(bytes) = STANDARD_NO_PAD.decode(&filtered) {
            return Some(bytes);
        }
    }
    None
}

/// `unwatermark.py:125-129` `ascii_url` — `b` toàn byte ASCII in được (32-126) hoặc
/// tab/LF/CR (9,10,13), VÀ bắt đầu bằng `http://`/`https://`; ngược lại `None` (Python: `""`).
fn ascii_url(b: &[u8]) -> Option<String> {
    if b.is_empty()
        || b.iter()
            .any(|&x| !(32..=126).contains(&x) && !matches!(x, 9 | 10 | 13))
    {
        return None;
    }
    // Mọi byte đã được kiểm nằm trong 0..=126 (ASCII) ở trên nên ép char 1-1 an toàn, tương
    // đương `bytes.decode("ascii", "ignore")` khi không có byte nào bị loại.
    let s: String = b.iter().map(|&x| char::from(x)).collect();
    is_http_url(&s).then_some(s)
}

/// `unwatermark.py:132-138` `_strip_pkcs7` — bỏ đệm PKCS7 hợp lệ; không hợp lệ → giữ nguyên.
fn strip_pkcs7(b: &[u8]) -> &[u8] {
    let Some(&pad) = b.last() else {
        return b;
    };
    let pad = pad as usize;
    if (1..=16).contains(&pad)
        && pad <= b.len()
        && b[b.len() - pad..].iter().all(|&x| x as usize == pad)
    {
        &b[..b.len() - pad]
    } else {
        b
    }
}

type Aes128CbcDec = cbc::Decryptor<Aes128>;

/// Giải AES-128-CBC THÔ (không tự bỏ đệm — dùng `NoPadding`, khớp Python: `Cipher(...).decryptor()
/// .update()+.finalize()` cũng không tự unpad). `None` khi độ dài/khoá/iv sai hoặc lỗi giải mã —
/// KHÔNG BAO GIỜ panic (khớp `except Exception: return ""` của `_aes_cbc_url`).
fn aes128_cbc_decrypt_raw(payload: &[u8], key: &[u8], iv: &[u8]) -> Option<Vec<u8>> {
    let key: &[u8; 16] = key.try_into().ok()?;
    let iv: &[u8; 16] = iv.try_into().ok()?;
    let mut buf = payload.to_vec();
    let pt = Aes128CbcDec::new(key.into(), iv.into())
        .decrypt_padded_mut::<NoPadding>(&mut buf)
        .ok()?;
    Some(pt.to_vec())
}

/// `unwatermark.py:141-150` `_aes_cbc_url` — giải AES-CBC rồi thử `ascii_url` trên bản thô, sau
/// đó trên bản đã bỏ đệm PKCS7 (thứ tự y hệt Python).
fn aes_cbc_url(payload: &[u8], key: &[u8], iv: &[u8]) -> Option<String> {
    if payload.is_empty() || payload.len() % 16 != 0 {
        return None;
    }
    let plain = aes128_cbc_decrypt_raw(payload, key, iv)?;
    ascii_url(&plain).or_else(|| ascii_url(strip_pkcs7(&plain)))
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = char::from(bytes[i]).to_digit(16)?;
        let lo = char::from(bytes[i + 1]).to_digit(16)?;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "hi,lo la chu so hex (0..=15) nen (hi<<4)|lo luon <=255"
        )]
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

/// `unwatermark.py:153-171` `decode_qaab` — CẢ các lượt thử (không chỉ công thức tóm tắt trong
/// brief): payload sau 4 byte `a8 00 01 00`; thử (key,iv) rồi HOÁN ĐỔI (iv,key); nếu token dài
/// hơn 36 byte, thử thêm offset 36 với iv lấy từ `data[20:36]`, rồi lại (key,iv) gốc trên payload
/// offset 36. Không có tiền tố `a8 00 01 00` → chỉ thử nguyên `data` với (key,iv).
fn decode_qaab(token: &str, key_seed: &str) -> Option<String> {
    let data = b64_loose(token)?;
    let seed = b64_loose(key_seed)?;
    let seed_head = &seed[..seed.len().min(32)];
    let d1 = Sha512::digest(seed_head);
    let mut salted = Vec::with_capacity(d1.len() + QAAB_SALT.len());
    salted.extend_from_slice(&d1);
    salted.extend_from_slice(&QAAB_SALT);
    let d2 = Sha512::digest(&salted);
    let key = &d2[..16];
    let iv = &d2[16..32];

    let mut attempts: Vec<(&[u8], &[u8], &[u8])> = Vec::new();
    if data.len() >= 4 && data[..4] == [0xa8, 0x00, 0x01, 0x00] {
        let rest = &data[4..];
        attempts.push((rest, key, iv));
        attempts.push((rest, iv, key));
        if data.len() > 36 {
            let rest36 = &data[36..];
            attempts.push((rest36, key, &data[20..36]));
            attempts.push((rest36, key, iv));
        }
    } else {
        attempts.push((&data[..], key, iv));
    }
    attempts
        .into_iter()
        .find_map(|(payload, k, i)| aes_cbc_url(payload, k, i))
}

/// `unwatermark.py:174-187` `decode_main_url` — `main_url` HTTP thẳng → giữ nguyên; base64
/// thường (`b64_loose` + `ascii_url`) → URL; tiền tố `qAAB` + có `key_seed` → [`decode_qaab`];
/// còn lại → `None` (Python `""`).
#[must_use]
pub fn decode_main_url(main_url: &str, key_seed: Option<&str>) -> Option<String> {
    let token = main_url.trim();
    if token.is_empty() {
        return None;
    }
    if is_http_url(token) {
        return Some(token.to_string());
    }
    if let Some(b) = b64_loose(token) {
        if let Some(u) = ascii_url(&b) {
            return Some(u);
        }
    }
    let seed = key_seed.unwrap_or("");
    if token.starts_with("qAAB") && !seed.is_empty() {
        return decode_qaab(token, seed);
    }
    None
}

// ===== `pick_raw_video` (unwatermark.py:202-222) =====

/// Đọc `entry.get(key)` rồi `int(float(...))` kiểu Python (`_score.n`, `unwatermark.py:190-198`):
/// chuỗi/khoá thiếu/không parse được → 0; số âm (không mong đợi cho bitrate/width) cũng chốt 0
/// (không panic, không lùi về số âm vô nghĩa cho điểm `u64`).
fn json_number_trunc(entry: &Map<String, Value>, key: &str) -> u64 {
    let f = match entry.get(key) {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    match f {
        Some(v) if v.is_finite() && v > 0.0 => {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "da kiem v>0.0 va huu han; bitrate/width/height thuc te << u64::MAX"
            )]
            {
                v.trunc() as u64
            }
        }
        _ => 0,
    }
}

/// `unwatermark.py:190-198` `_score.n(*keys)` — thử từng khoá theo thứ tự, khoá ĐẦU TIÊN cho
/// giá trị khác 0 thắng; hết khoá mà vẫn 0 → 0.
fn first_nonzero(entry: &Map<String, Value>, keys: &[&str]) -> u64 {
    for k in keys {
        let v = json_number_trunc(entry, k);
        if v != 0 {
            return v;
        }
    }
    0
}

fn truthy_str(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::String(s)) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    }
}

/// `unwatermark.py:204-209` phần chọn `vi`/`data` — trả `None` sớm nếu `body` không phải object
/// HOẶC không rút ra được `data` là object (Python cuối cùng cũng không tạo được `entries` có
/// `main_url` hợp lệ trong các nhánh đó — xem chứng minh trong doc-comment `pick_raw_video`).
fn pick_video_data<'a>(body: &'a Value) -> Option<&'a Map<String, Value>> {
    let obj = body.as_object()?;
    let truthy = |v: Option<&'a Value>| v.filter(|val| is_truthy_json(Some(val)));
    let vi: &Value = truthy(obj.get("video_info"))
        .or_else(|| {
            truthy(
                obj.get("data")
                    .and_then(Value::as_object)
                    .and_then(|d| d.get("video_info")),
            )
        })
        .unwrap_or(body);
    let data: &Value = match vi.as_object().and_then(|vi_obj| vi_obj.get("data")) {
        Some(d) if d.is_object() => d,
        _ => vi,
    };
    data.as_object()
}

/// `unwatermark.py:202-222` `pick_raw_video` — duyệt `video_list` (hoặc coi cả `data` là MỘT
/// entry nếu không có `video_list` non-empty), giải mã `main_url`/`play_url` bằng
/// [`decode_main_url`] (seed dò bằng [`find_key_seed`] trên chính `body`), chọn entry điểm
/// `bitrate + w*h` cao nhất. Chữ ký rút gọn theo đúng `task-3-brief.md` (trả thẳng
/// `Option<String>` — URL của bản tốt nhất — thay vì `dict` đầy đủ như Python; không nhận
/// `api_url` riêng vì brief không đưa tham số đó, tương đương gọi Python với `api_url=""`).
#[must_use]
pub fn pick_raw_video(body: &Value) -> Option<String> {
    let data_obj = pick_video_data(body)?;
    let seed = find_key_seed(body, "");
    // Không bọc lại `data_obj` vào `Value` tạm (tránh giữ tham chiếu tới giá trị tạm thời) —
    // làm việc thẳng trên `&Map<String, Value>`: nhánh `video_list` lọc bớt phần tử KHÔNG phải
    // object (khớp Python `if not isinstance(e, dict): continue` ngay từ bước gom `entries`,
    // cùng kết quả vì phần tử không phải dict sẽ không bao giờ tạo được candidate).
    let entries: Vec<&Map<String, Value>> =
        match data_obj.get("video_list").and_then(Value::as_object) {
            Some(vl) if !vl.is_empty() => vl.values().filter_map(Value::as_object).collect(),
            _ => vec![data_obj],
        };

    let mut best: Option<RawCandidate> = None;
    for e_obj in entries {
        let main_url = truthy_str(e_obj.get("main_url"))
            .or_else(|| truthy_str(e_obj.get("play_url")))
            .unwrap_or("");
        let Some(url) = decode_main_url(main_url, seed.as_deref()) else {
            continue;
        };
        let candidate = RawCandidate {
            url,
            bitrate: first_nonzero(e_obj, &["bitrate", "real_bitrate"]),
            w: first_nonzero(e_obj, &["vwidth", "width"]),
            h: first_nonzero(e_obj, &["vheight", "height"]),
        };
        let better = best.as_ref().is_none_or(|b| candidate.score() > b.score());
        if better {
            best = Some(candidate);
        }
    }
    best.map(|c| c.url)
}

// ===== `extract_unwatermarked_url` (video_worker.py:118-141) — bản CŨ, đơn giản hơn =====

/// Giống `.decode("utf-8", "ignore")` của Python: BỎ (không thay thế) chuỗi byte không hợp lệ —
/// khác `String::from_utf8_lossy` (chèn U+FFFD). Không panic với mọi input.
fn utf8_decode_ignore(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut rest = bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                out.push_str(s);
                return out;
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                if let Ok(s) = std::str::from_utf8(&rest[..valid_up_to]) {
                    out.push_str(s);
                }
                let invalid_len = e.error_len().unwrap_or(1);
                rest = &rest[valid_up_to + invalid_len..];
                if rest.is_empty() {
                    return out;
                }
            }
        }
    }
}

/// `video_worker.py:118-141` `extract_unwatermarked_url` — bản CŨ/đơn giản (KHÔNG dùng
/// `b64_loose`/qAAB, chỉ base64 chuẩn + `str.startswith("http")` lỏng hơn `ascii_url`), dùng làm
/// fallback khi không có `fallback_api`. PHẢI port ĐÚNG một quirk của Python: TOÀN BỘ thân hàm
/// nằm trong 1 `try/except Exception: pass` DUY NHẤT bao ngoài — nếu `int(v.get("bitrate") or
/// v.get("real_bitrate") or 0)` NÉM lỗi ở BẤT KỲ entry nào (vd bitrate là chuỗi không phải số
/// nguyên thuần, `int("12.5")` ném `ValueError` trong Python — khác `_score` dùng
/// `int(float(...))`), NGOẠI LỆ ĐÓ THOÁT KHỎI CẢ VÒNG LẶP, mọi candidate đã tìm được TRƯỚC ĐÓ bị
/// bỏ, hàm rơi thẳng về `fallback_url`. Ở đây mô phỏng bằng `return` sớm nguyên hàm khi gặp
/// trường hợp đó (không phải `continue`).
#[must_use]
pub fn extract_unwatermarked_url(video_model: &str, download_url: &str) -> String {
    let vm_str = if video_model.is_empty() {
        "{}"
    } else {
        video_model
    };
    let Ok(vm) = serde_json::from_str::<Value>(vm_str) else {
        return download_url.to_string();
    };
    let Some(video_list) = vm.get("video_list").and_then(Value::as_object) else {
        return download_url.to_string();
    };

    let mut candidates: Vec<(i64, String)> = Vec::new();
    for v in video_list.values() {
        let Some(v_obj) = v.as_object() else {
            continue;
        };
        let main_url = v_obj.get("main_url").and_then(Value::as_str).unwrap_or("");
        if main_url.is_empty() {
            continue;
        }
        let Ok(raw) = STANDARD.decode(main_url) else {
            continue;
        };
        let decoded = utf8_decode_ignore(&raw);
        if !decoded.starts_with("http") {
            continue;
        }
        let bitrate_val = v_obj
            .get("bitrate")
            .filter(|val| is_truthy_json(Some(val)))
            .or_else(|| {
                v_obj
                    .get("real_bitrate")
                    .filter(|val| is_truthy_json(Some(val)))
            });
        let bitrate = match bitrate_val {
            None => 0,
            Some(val) => match python_int_strict(val) {
                Some(n) => n,
                // Python: `int(...)` ném ValueError -> ngoại lệ thoát ra ngoài TOÀN BỘ try ở
                // đầu hàm -> return fallback_url NGAY (bỏ mọi candidate đã gom trước đó).
                None => return download_url.to_string(),
            },
        };
        candidates.push((bitrate, decoded));
    }
    if candidates.is_empty() {
        return download_url.to_string();
    }
    // Python: `candidates.sort(key=lambda x: x[0], reverse=True)` — sort ổn định, `reverse=True`
    // vẫn giữ thứ tự gốc giữa các phần tử bằng điểm (đã kiểm bằng Python thật). `sort_by_key`
    // của Rust cũng ổn định nên khoá `Reverse(bitrate)` (tăng dần trên Reverse = giảm dần trên
    // bitrate) cho kết quả y hệt.
    candidates.sort_by_key(|c| std::cmp::Reverse(c.0));
    candidates
        .into_iter()
        .next()
        .map_or_else(|| download_url.to_string(), |(_, url)| url)
}

/// `int(x)` kiểu Python áp cho giá trị JSON ĐÃ chọn (không phải `int(float(x))` như `_score`):
/// số nguyên JSON → giữ; số thực JSON → Python `int(12.0)` OK (JSON không phân biệt số nguyên
/// viết dạng thực, `12.0` parse thành số Python thực `float`, `int(12.0)=12` — vẫn hợp lệ, chỉ
/// `int("12.5")` (CHUỖI có dấu chấm) mới lỗi); chuỗi → phải là số nguyên thuần (`i64::from_str`,
/// chấp nhận dấu +/-). `None` = Python ném `ValueError`/`TypeError`.
fn python_int_strict(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(i)
            } else {
                n.as_f64().map(|f| {
                    #[allow(
                        clippy::cast_possible_truncation,
                        reason = "gia tri bitrate JSON thuc te << i64::MAX; Python int(float) cat phan le ve 0"
                    )]
                    {
                        f.trunc() as i64
                    }
                })
            }
        }
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

// ===== `is_watermarked_url` (postprocess.py:58-63) =====

/// `postprocess.py:58-63` `is_watermarked_url` — rỗng hoặc chứa `"unwatermarked"` → false (sạch);
/// còn lại: chứa `"watermark"` → true.
#[must_use]
pub fn is_watermarked_url(url: &str) -> bool {
    let u = url.to_lowercase();
    if u.is_empty() || u.contains("unwatermarked") {
        return false;
    }
    u.contains("watermark")
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;
    use serde_json::json;

    // ---- (a) find_fallback_apis ----

    #[test]
    fn tim_ca_hai_fallback_api_khong_loc_host() {
        // Xác minh bằng Python thật (chỉ đọc, .venv):
        //   python -c "
        //   import sys; sys.path.insert(0, r'A:\ToolLVC\lvcseedance')
        //   from src.media import unwatermark as uw
        //   vm = '{\"video_list\":{\"1\":{\"fallback_api\":\"https://vod-urls-mya.byteintlapi.com/x?key_seed=abc\"}},
        //          \"other\":{\"fallback_api\":\"https://some-other-host.example.com/x?y=1\"}}'
        //   print(uw.find_fallback_apis(vm))"
        // -> ['https://vod-urls-mya.byteintlapi.com/x?key_seed=abc', 'https://some-other-host.example.com/x?y=1']
        // find_fallback_apis (unwatermark.py:51-83) KHÔNG lọc host — `add()` chỉ kiểm tiền tố
        // http(s) + khử trùng. Test ghim đúng hành vi Python thật, không theo mô tả (sai) trong
        // task-3-brief.md bước 1(a) — xem doc comment đầu file.
        let vm = r#"{"video_list":{"1":{"fallback_api":"https://vod-urls-mya.byteintlapi.com/x?key_seed=abc"}},"other":{"fallback_api":"https://some-other-host.example.com/x?y=1"}}"#;
        let found = find_fallback_apis(vm);
        assert_eq!(
            found,
            vec![
                "https://vod-urls-mya.byteintlapi.com/x?key_seed=abc".to_string(),
                "https://some-other-host.example.com/x?y=1".to_string(),
            ]
        );
    }

    #[test]
    fn fallback_api_khop_qua_regex_trong_chuoi_thoat() {
        // Mô phỏng video_model là CHUỖI JSON chứa fallback_api đã escape backslash-quote (giống
        // dữ liệu Dola thật: video_model là 1 chuỗi JSON nằm trong creation block).
        let vm = r#"{"a":"{\"fallback_api\":\"https://vod-urls-mya.byteintlapi.com/y\"}"}"#;
        let found = find_fallback_apis(vm);
        assert_eq!(
            found,
            vec!["https://vod-urls-mya.byteintlapi.com/y".to_string()]
        );
    }

    // ---- (b) replace_query ----

    #[test]
    fn replace_query_giu_khoa_cu_ghi_de_va_them_khoa_moi() {
        let out = replace_query("https://h/p?a=1&channel=x", &RAW_PARAMS);
        assert_eq!(
            out,
            "https://h/p?a=1&channel=no&codec_type=8&logo_type=unwatermarked"
        );
    }

    // ---- (c) decode_main_url — base64 thường ----

    #[test]
    fn decode_main_url_base64_thuong() {
        let token = B64.encode("https://x/y.mp4");
        assert_eq!(
            decode_main_url(&token, None),
            Some("https://x/y.mp4".to_string())
        );
    }

    #[test]
    fn decode_main_url_http_thang_giu_nguyen() {
        assert_eq!(
            decode_main_url("https://already/plain.mp4", None),
            Some("https://already/plain.mp4".to_string())
        );
    }

    // ---- (d) qAAB round-trip + vector âm ----
    //
    // Vector sinh bằng Python thật (đảo ngược decode_qaab, chỉ đọc unwatermark.py):
    //   python -c "
    //   import sys, base64, hashlib
    //   sys.path.insert(0, r'A:\ToolLVC\lvcseedance')
    //   from src.media import unwatermark as uw
    //   from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
    //   from cryptography.hazmat.primitives import padding as pad
    //   key_seed_raw = b'0123456789abcdef0123456789abcdef'
    //   key_seed_b64 = base64.b64encode(key_seed_raw).decode()
    //   seed = base64.b64decode(key_seed_b64)
    //   d1 = hashlib.sha512(seed[:32]).digest()
    //   d2 = hashlib.sha512(d1 + bytes.fromhex(uw.QAAB_SALT_HEX)).digest()
    //   key, iv = d2[:16], d2[16:32]
    //   padder = pad.PKCS7(128).padder()
    //   padded = padder.update(b'https://ok/') + padder.finalize()
    //   enc = Cipher(algorithms.AES(key), modes.CBC(iv)).encryptor()
    //   ct = enc.update(padded) + enc.finalize()
    //   token = base64.b64encode(b'\xa8\x00\x01\x00' + ct).decode()
    //   print(key_seed_b64); print(token)
    //   assert uw.decode_qaab(token, key_seed_b64) == 'https://ok/'"
    // -> key_seed_b64 = MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=
    //    token        = qAABAD1HepG3thmtRikhsvPW1Tk=
    // (đã xác nhận uw.decode_qaab(token, key_seed_b64) == "https://ok/" bằng chính bản Python.)

    const QAAB_KEY_SEED_B64: &str = "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=";
    const QAAB_TOKEN: &str = "qAABAD1HepG3thmtRikhsvPW1Tk=";

    #[test]
    fn qaab_round_trip_dung_key_seed() {
        assert_eq!(
            decode_main_url(QAAB_TOKEN, Some(QAAB_KEY_SEED_B64)),
            Some("https://ok/".to_string())
        );
    }

    #[test]
    fn qaab_sai_key_seed_tra_none() {
        // key_seed khác (nhưng vẫn base64 hợp lệ) -> key/iv suy ra khác -> AES giải sai -> không
        // ra được URL ascii hợp lệ -> None. Đã xác nhận bằng Python thật: uw.decode_qaab(token,
        // wrong_seed_b64) == '' (rỗng).
        let wrong_seed_b64 = B64.encode([b'X'; 33]);
        assert_eq!(decode_main_url(QAAB_TOKEN, Some(&wrong_seed_b64)), None);
    }

    #[test]
    fn qaab_khong_co_key_seed_tra_none() {
        assert_eq!(decode_main_url(QAAB_TOKEN, None), None);
    }

    // ---- Fix round 1: b64_loose bỏ ký tự lạ như base64.b64decode(validate=False) ----
    //
    // Cả 3 vector dưới đây đã được đối chiếu với `unwatermark.b64_loose` THẬT (`.venv`, chỉ đọc):
    //   python -c "
    //   import sys; sys.path.insert(0, r'A:\ToolLVC\lvcseedance')
    //   from src.media import unwatermark as uw
    //   print(uw.b64_loose('aHR0cH\nM6Ly94L3kubXA0'))   # -> b'https://x/y.mp4'
    //   print(uw.b64_loose('$$$0cHM6Ly9vay9hYmNk'))      # -> b'\xff\xff\xf4ps://ok/abcd'
    //   print(uw.b64_loose('abcde'))                      # -> None"

    #[test]
    fn b64_loose_bo_qua_khoang_trang_newline_nhu_python() {
        // base64("https://x/y.mp4") = "aHR0cHM6Ly94L3kubXA0"; chèn '\n' vào giữa — CPython
        // b64decode(validate=False) bỏ ký tự lạ (kể cả whitespace) trước khi ráp nhóm 4, nên vẫn
        // giải đúng. Test qua decode_main_url (đường đi thật của main_url dạng base64 thường).
        assert_eq!(
            decode_main_url("aHR0cH\nM6Ly94L3kubXA0", None),
            Some("https://x/y.mp4".to_string())
        );
    }

    #[test]
    fn b64_loose_alt_alphabet_dola_khop_python_tung_byte() {
        // Thay 3 ký tự của base64("https://ok/abcd") (20 ký tự, không cần đệm) bằng '$' ở vị trí
        // 0,1,2 — biến thể 0 (nguyên văn) LỌC BỎ cả 3 '$' -> còn 17 ký tự, dư 1 (17%4==1) ->
        // "Incorrect padding" ở Python / lỗi ở STANDARD_NO_PAD -> rớt; biến thể 1 ($→_→/) GIỮ cả
        // 3 ký tự (thành '/') -> đủ lại 20 ký tự, dư 0 -> giải được, ra byte "rác" xác định (không
        // phải URL hợp lệ — do đó test trực tiếp qua hàm nội bộ, không qua decode_main_url/ascii_url).
        // ĐÃ THỬ (xem task-3-report.md phần "Fix round 1"): với đúng thuật toán lọc-trước-rồi-giải
        // -KHÔNG-ĐỆM mà controller chỉ định, biến thể 1 và biến thể 2 LUÔN cùng thành/bại với mọi
        // tổ hợp `$`/`@`/`#` (chứng minh trong doc-comment `keep_base64_alphabet`) — không tồn tại
        // input nào khiến CHỈ RIÊNG biến thể 2 thắng dưới thuật toán này. Vector dưới đây là ca gần
        // nhất có thể: biến thể 0 rớt (đúng vì lý do độ dài), biến thể 1 thắng, VÀ khớp byte-for-byte
        // với `unwatermark.b64_loose` THẬT (dù có thể Python nội bộ cũng dừng ở biến thể 1 vì cùng
        // lý do độ dài — không quan sát được index biến thể từ ngoài, chỉ so được kết quả cuối).
        let token = "$$$0cHM6Ly9vay9hYmNk";
        let expected: Vec<u8> = vec![
            0xff, 0xff, 0xf4, b'p', b's', b':', b'/', b'/', b'o', b'k', b'/', b'a', b'b', b'c',
            b'd',
        ];
        assert_eq!(b64_loose(token), Some(expected));
    }

    #[test]
    fn b64_loose_do_dai_du_1_tra_none() {
        // "abcde": 5 ký tự thuần chữ/số (không $/@/#) -> cả 3 biến thể giống hệt nhau, lọc xong
        // vẫn 5 ký tự, 5%4==1 -> luôn rớt ở cả 3 lượt -> None. Đã xác nhận bằng Python thật:
        // uw.b64_loose("abcde") == None.
        assert_eq!(b64_loose("abcde"), None);
    }

    // ---- (e) pick_raw_video theo bitrate + w*h ----

    #[test]
    fn pick_raw_video_chon_diem_bitrate_cong_wh_cao_nhat() {
        let low = B64.encode("https://low/720p.mp4");
        let high = B64.encode("https://high/1080p.mp4");
        let body = json!({
            "video_info": {
                "data": {
                    "video_list": {
                        "v1": {"main_url": low, "bitrate": 500, "vwidth": 640, "vheight": 480},
                        "v2": {"main_url": high, "bitrate": 2000, "vwidth": 1280, "vheight": 720}
                    }
                }
            }
        });
        // v1: 500 + 640*480 = 307700; v2: 2000 + 1280*720 = 923600 -> v2 thắng.
        assert_eq!(
            pick_raw_video(&body),
            Some("https://high/1080p.mp4".to_string())
        );
    }

    #[test]
    fn pick_raw_video_khong_co_entry_hop_le_tra_none() {
        let body = json!({"video_info": {"data": {"video_list": {}}}});
        assert_eq!(pick_raw_video(&body), None);
    }

    // ---- (f) extract_unwatermarked_url ----

    #[test]
    fn extract_unwatermarked_chon_bitrate_cao_nhat() {
        let low = B64.encode("http://a/low.mp4");
        let high = B64.encode("http://a/high.mp4");
        let vm = json!({
            "video_list": {
                "1": {"main_url": low, "bitrate": 100},
                "2": {"main_url": high, "bitrate": 900}
            }
        })
        .to_string();
        assert_eq!(
            extract_unwatermarked_url(&vm, "http://fallback/old.mp4"),
            "http://a/high.mp4".to_string()
        );
    }

    #[test]
    fn extract_unwatermarked_khong_co_candidate_thi_fallback() {
        let vm = json!({"video_list": {}}).to_string();
        assert_eq!(
            extract_unwatermarked_url(&vm, "http://fallback/old.mp4"),
            "http://fallback/old.mp4".to_string()
        );
    }

    #[test]
    fn extract_unwatermarked_json_hong_thi_fallback() {
        assert_eq!(
            extract_unwatermarked_url("khong phai json", "http://fallback/old.mp4"),
            "http://fallback/old.mp4".to_string()
        );
    }

    // ---- (g) is_watermarked_url ----

    #[test]
    fn is_watermarked_url_ba_truong_hop() {
        assert!(is_watermarked_url(
            "https://v.dola.com/x.mp4?lr=video_gen_watermark_dyn"
        ));
        assert!(!is_watermarked_url(
            "https://v.dola.com/x.mp4?lr=unwatermarked"
        ));
        assert!(!is_watermarked_url(""));
    }
}
