//! Đọc phản hồi Dola (THUẦN): luồng SSE `chat/completion` và tin nhắn `im/chain/single` — port
//! `dola_app.py:617-1079` (`_sse`, `find_video`, `extract_urls`, `collect_images`, `send_payload`,
//! `send_image`, `poll_video`, `poll_images`, `message_blocks`, `plain_text`, `walk_video_models`).

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// `creation.status` khi video đã xong (`dola_app.py:133`).
pub const VIDEO_STATUS_DONE: i64 = 3;

/// Biến thể URL ảnh theo thứ tự ưu tiên (`:709`).
const IMG_VARIANTS: [&str; 5] = [
    "image_ori_raw",
    "image_ori",
    "image_preview",
    "image_preview_resize",
    "image_thumb",
];

/// Bộ tách dòng SSE tăng dần: nạp từng mẩu byte (`push`), nhận ra các cặp `(event, data JSON)` đầy
/// đủ. Luật như `_sse` (`:618-636`): `event:` đặt tên, `data:` rỗng/`{}`/JSON hỏng bị bỏ, sau mỗi
/// `data:` tên sự kiện reset.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    event: Option<String>,
}

impl SseParser {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Nạp thêm byte; trả mọi sự kiện đã đủ dòng.
    pub fn push(&mut self, chunk: &[u8]) -> Vec<(Option<String>, Value)> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            self.line(&line, &mut out);
        }
        out
    }

    /// Kết thúc luồng: xử lý dòng cuối không có `\n`.
    pub fn finish(&mut self) -> Vec<(Option<String>, Value)> {
        let mut out = Vec::new();
        if !self.buf.is_empty() {
            let line = std::mem::take(&mut self.buf);
            self.line(&line, &mut out);
        }
        out
    }

    fn line(&mut self, raw: &[u8], out: &mut Vec<(Option<String>, Value)>) {
        let text = String::from_utf8_lossy(raw);
        let line = text.trim_end_matches(['\r', '\n']);
        if let Some(ev) = line.strip_prefix("event:") {
            self.event = Some(ev.trim().to_string());
        } else if let Some(d) = line.strip_prefix("data:") {
            let d = d.trim();
            if !d.is_empty() && d != "{}" {
                if let Ok(v) = serde_json::from_str::<Value>(d) {
                    out.push((self.event.clone(), v));
                }
            }
            self.event = None;
        }
    }
}

/// Tin nhắn trong chain: chuỗi JSON `'[{block…}]'` → danh sách block; dạng khác → rỗng (`:1057`).
#[must_use]
pub fn message_blocks(content: &Value) -> Vec<Value> {
    match content {
        Value::Array(a) => a.clone(),
        Value::String(s) if s.starts_with('[') => serde_json::from_str(s).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Tin `content_type 1` (từ chối nội dung): `content='{"text":"…"}'`; không có thì `tts_content`
/// (`:1069-1079`).
#[must_use]
pub fn plain_text(content: &Value, tts: &str) -> Vec<String> {
    if let Value::String(s) = content {
        if s.starts_with('{') {
            if let Ok(Value::Object(o)) = serde_json::from_str::<Value>(s) {
                if let Some(t) = o.get("text").and_then(Value::as_str) {
                    if !t.trim().is_empty() {
                        return vec![t.trim().to_string()];
                    }
                }
            }
        }
    }
    let t = tts.trim();
    if t.is_empty() {
        Vec::new()
    } else {
        vec![t.to_string()]
    }
}

/// Chữ trong các `text_block` (`_texts`, `:686`).
#[must_use]
pub fn block_texts(blocks: &[Value]) -> Vec<String> {
    blocks
        .iter()
        .filter_map(|b| {
            b.get("content")
                .and_then(|c| c.get("text_block"))
                .and_then(|t| t.get("text"))
                .and_then(Value::as_str)
        })
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// `creations[]` video đầu tiên (`type == 2`) ở bất kỳ độ sâu nào (`find_video`, `:639-654`).
#[must_use]
pub fn find_video(v: &Value) -> Option<Value> {
    match v {
        Value::Object(o) => {
            if let Some(Value::Array(cs)) = o.get("creations") {
                for c in cs {
                    if c.get("type").and_then(Value::as_i64) == Some(2) {
                        if let Some(video) = c.get("video").filter(|x| x.is_object()) {
                            return Some(video.clone());
                        }
                    }
                }
            }
            o.values().find_map(find_video)
        }
        Value::Array(a) => a.iter().find_map(find_video),
        _ => None,
    }
}

/// URL của video (`extract_urls`, `:665-683`): `download_url` (CÓ logo) + `nowm_url` (khung cao
/// nhất trong `video_model.video_list`, `main_url` base64).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VideoUrls {
    pub vid: String,
    pub status: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub download_url: String,
    pub nowm_url: String,
    pub video_model: String,
}

fn b64_text(s: &str) -> String {
    use base64::Engine as _;
    let s = s.trim();
    let pad = (4 - s.len() % 4) % 4;
    let padded = format!("{s}{}", "=".repeat(pad));
    base64::engine::general_purpose::STANDARD
        .decode(padded)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

#[must_use]
pub fn extract_urls(video: &Value) -> VideoUrls {
    let s = |k: &str| {
        video
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut out = VideoUrls {
        vid: s("vid"),
        status: video.get("status").and_then(Value::as_i64),
        width: video.get("width").and_then(Value::as_i64),
        height: video.get("height").and_then(Value::as_i64),
        download_url: s("download_url"),
        nowm_url: String::new(),
        video_model: s("video_model"),
    };
    if out.video_model.is_empty() {
        return out;
    }
    let vm: Value = serde_json::from_str(&out.video_model).unwrap_or(Value::Null);
    let mut best: Option<&Value> = None;
    if let Some(list) = vm.get("video_list").and_then(Value::as_object) {
        for v in list.values().filter(|v| v.is_object()) {
            let h = |x: &Value| x.get("vheight").and_then(Value::as_i64).unwrap_or(0);
            if best.is_none_or(|b| h(v) > h(b)) {
                best = Some(v);
            }
        }
    }
    if let Some(b) = best {
        out.nowm_url = b64_text(b.get("main_url").and_then(Value::as_str).unwrap_or(""));
        out.width = b.get("vwidth").and_then(Value::as_i64).or(out.width);
        out.height = b.get("vheight").and_then(Value::as_i64).or(out.height);
    }
    out
}

/// Gom mọi chuỗi `video_model` ở mọi độ sâu, kể cả JSON lồng trong chuỗi (`walk_video_models`,
/// `:987-1005`).
pub fn walk_video_models(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                let non_empty = match x {
                    Value::Null => false,
                    Value::String(s) => !s.is_empty(),
                    Value::Array(a) => !a.is_empty(),
                    Value::Object(m) => !m.is_empty(),
                    Value::Bool(b) => *b,
                    Value::Number(_) => true,
                };
                if k == "video_model" && non_empty {
                    out.push(match x {
                        Value::String(s) => s.clone(),
                        other => serde_json::to_string(other).unwrap_or_default(),
                    });
                } else {
                    walk_video_models(x, out);
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk_video_models(x, out)),
        Value::String(s) if s.contains("video_model") || s.contains("fallback_api") => {
            if let Ok(inner) = serde_json::from_str::<Value>(s) {
                walk_video_models(&inner, out);
            }
        }
        _ => {}
    }
}

/// Một ảnh Seedream tạo ra.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageEntry {
    pub id: String,
    pub url: String,
    pub width: i64,
    pub height: i64,
    pub key: String,
    /// `uri` để đính lại làm tham chiếu (không upload) — `key` hoặc suy từ URL.
    pub uri: String,
}

#[derive(Debug, Clone, Default)]
struct ImgAcc {
    variants: BTreeMap<&'static str, String>,
    order: Vec<&'static str>,
    width: i64,
    height: i64,
    key: String,
}

/// Bộ gom ảnh giữ thứ tự xuất hiện như `dict` Python (`collect_images`, `:761-781`).
#[derive(Debug, Default, Clone)]
pub struct ImageCollector {
    order: Vec<String>,
    map: BTreeMap<String, ImgAcc>,
}

fn image_id(url: &str) -> String {
    let base = url.split('?').next().unwrap_or("");
    let base = base.split('~').next().unwrap_or("");
    base.rsplit('/').next().unwrap_or("").to_string()
}

static IMG_URI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^https?://[^/]+/((?:tos|rc)[^~?#]+)").unwrap_or_else(|_| unreachable!())
});

/// `https://…/tos-mya-i-xxx/rc_gen_image/abc.jpeg~tplv-…` → `tos-mya-i-xxx/rc_gen_image/abc.jpeg`;
/// lạ → rỗng (`image_uri_from_url`, `:796-802`).
#[must_use]
pub fn image_uri_from_url(url: &str) -> String {
    IMG_URI_RE
        .captures(url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default()
}

impl ImageCollector {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn collect(&mut self, v: &Value) {
        match v {
            Value::Object(o) => {
                if let Some(Value::Array(cs)) = o.get("creations") {
                    for c in cs {
                        self.creation(c);
                    }
                }
                for x in o.values() {
                    self.collect(x);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| self.collect(x)),
            _ => {}
        }
    }

    fn creation(&mut self, c: &Value) {
        let Some(img) = c.get("image").and_then(Value::as_object) else {
            return;
        };
        // Thu tu bien the = thu tu khoa trong object `image` (nhu dict Python).
        let mut variants: Vec<(&'static str, String)> = Vec::new();
        for (k, val) in img {
            let Some(name) = IMG_VARIANTS.iter().find(|n| **n == k.as_str()) else {
                continue;
            };
            let Some(url) = val.get("url").and_then(Value::as_str) else {
                continue;
            };
            if url.starts_with("http") {
                variants.push((name, url.to_string()));
            }
        }
        let Some((_, first)) = variants.first() else {
            return;
        };
        let iid = image_id(first);
        if !self.map.contains_key(&iid) {
            self.order.push(iid.clone());
        }
        let ent = self.map.entry(iid).or_default();
        for (name, url) in variants {
            if !ent.variants.contains_key(name) {
                ent.order.push(name);
            }
            ent.variants.insert(name, url);
        }
        let ori = img
            .get("image_ori_raw")
            .or_else(|| img.get("image_ori"))
            .cloned()
            .unwrap_or(Value::Null);
        if let Some(w) = ori.get("width").and_then(Value::as_i64) {
            ent.width = w;
        }
        if let Some(h) = ori.get("height").and_then(Value::as_i64) {
            ent.height = h;
        }
        if let Some(k) = ori
            .get("key")
            .and_then(Value::as_str)
            .filter(|k| !k.is_empty())
        {
            ent.key = k.to_string();
        }
    }

    /// Ảnh đã gom, URL tốt nhất theo thứ tự ưu tiên biến thể (`_image_entries`, `:818-820`).
    #[must_use]
    pub fn entries(&self) -> Vec<ImageEntry> {
        self.order
            .iter()
            .filter_map(|id| {
                let e = self.map.get(id)?;
                let url = IMG_VARIANTS
                    .iter()
                    .find_map(|k| e.variants.get(k).cloned())
                    .or_else(|| e.order.first().and_then(|k| e.variants.get(k).cloned()))?;
                let uri = if e.key.is_empty() {
                    image_uri_from_url(&url)
                } else {
                    e.key.clone()
                };
                Some(ImageEntry {
                    id: id.clone(),
                    url,
                    width: e.width,
                    height: e.height,
                    key: e.key.clone(),
                    uri,
                })
            })
            .collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

/// Kết quả đọc HẾT một luồng SSE `chat/completion` (`send_payload` `:905-936` + `send_image`
/// `:848-872` — gộp: cả video lẫn ảnh đều gom).
#[derive(Debug, Clone, Default)]
pub struct ChatStream {
    pub conversation_id: String,
    pub section_id: String,
    pub text: String,
    pub video: Option<Value>,
    pub images: ImageCollector,
    /// JSON `STREAM_ERROR` (cắt 300 ký tự), rỗng nếu không lỗi.
    pub error: String,
    /// Mã lỗi số trong `STREAM_ERROR.error_code` (nếu có).
    pub error_code: String,
    /// Chuỗi tên sự kiện đã nhận (chẩn đoán).
    pub events: Vec<String>,
    /// Đã gặp `STREAM_ERROR` — ngừng nhận tiếp.
    pub stopped: bool,
}

impl ChatStream {
    #[must_use]
    pub fn new(conversation_id: &str) -> Self {
        Self {
            conversation_id: conversation_id.to_string(),
            ..Self::default()
        }
    }

    /// Nạp MỘT sự kiện. Sau `STREAM_ERROR` mọi sự kiện sau bị bỏ (Python `break`).
    pub fn feed(&mut self, event: Option<&str>, data: &Value) {
        if self.stopped {
            return;
        }
        self.events.push(event.unwrap_or("?").to_string());
        match event {
            Some("STREAM_ERROR") => {
                let full = serde_json::to_string(data).unwrap_or_default();
                self.error = full.chars().take(300).collect();
                self.error_code = match data.get("error_code") {
                    Some(Value::String(s)) => s.clone(),
                    Some(Value::Number(n)) => n.to_string(),
                    _ => String::new(),
                };
                self.stopped = true;
            }
            Some("SSE_ACK") => {
                let meta = data.get("ack_client_meta");
                if let Some(c) = meta
                    .and_then(|m| m.get("conversation_id"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    self.conversation_id = c.to_string();
                }
                if let Some(s) = meta
                    .and_then(|m| m.get("section_id"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    self.section_id = s.to_string();
                }
            }
            Some("STREAM_MSG_NOTIFY" | "STREAM_CHUNK" | "FULL_MSG_NOTIFY") => {
                let content = data.get("content").cloned().unwrap_or(Value::Null);
                let mut blocks = match &content {
                    Value::Object(o) => o
                        .get("content_block")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default(),
                    other => message_blocks(other),
                };
                if let Some(ops) = data.get("patch_op").and_then(Value::as_array) {
                    for op in ops {
                        if let Some(b) = op
                            .get("patch_value")
                            .and_then(|p| p.get("content_block"))
                            .and_then(Value::as_array)
                        {
                            blocks.extend(b.iter().cloned());
                        }
                    }
                }
                let mut chunk = block_texts(&blocks).concat();
                if chunk.is_empty() {
                    if let Some(t) = content.get("text").and_then(Value::as_str) {
                        chunk = t.to_string();
                    }
                }
                self.text.push_str(&chunk);
                if let Some(v) = find_video(data) {
                    self.video = Some(v);
                }
                self.images.collect(data);
            }
            _ => {}
        }
    }
}

/// Kết quả MỘT lần poll chat (`poll_video` `:1025-1054` + `poll_images` `:875-894`).
#[derive(Debug, Clone, Default)]
pub struct ChainPoll {
    pub video: Option<Value>,
    /// Chữ bot (tin mới nhất trước — thứ tự server trả).
    pub texts: Vec<String>,
    pub last_index: i64,
    pub section_id: String,
    /// Tin mới nhất có `ext.is_finish == "1"`.
    pub finished: bool,
    pub images: Vec<ImageEntry>,
}

/// Đọc phản hồi `im/chain/single` cmd 3100.
#[must_use]
pub fn parse_chain(resp: &Value) -> ChainPoll {
    let msgs = resp
        .get("downlink_body")
        .and_then(|d| d.get("pull_singe_chain_downlink_body"))
        .and_then(|d| d.get("messages"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = ChainPoll::default();
    let mut images = ImageCollector::new();
    for (i, m) in msgs.iter().enumerate() {
        let idx = match m.get("index_in_conv") {
            Some(Value::String(s)) => s.parse::<i64>().unwrap_or(0),
            Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
            _ => 0,
        };
        out.last_index = out.last_index.max(idx);
        if out.section_id.is_empty() {
            if let Some(s) = m.get("section_id").and_then(Value::as_str) {
                out.section_id = s.to_string();
            }
        }
        if m.get("user_type").and_then(Value::as_i64) == Some(1) {
            continue;
        }
        let content = m.get("content").cloned().unwrap_or(Value::Null);
        let blocks = message_blocks(&content);
        let blocks_v = Value::Array(blocks.clone());
        if let Some(v) = find_video(&blocks_v) {
            let done = v.get("status").and_then(Value::as_i64) == Some(VIDEO_STATUS_DONE);
            if out.video.is_none() || done {
                out.video = Some(v);
            }
        }
        images.collect(&blocks_v);
        let texts = block_texts(&blocks);
        if texts.is_empty() {
            let tts = m.get("tts_content").and_then(Value::as_str).unwrap_or("");
            out.texts.extend(plain_text(&content, tts));
        } else {
            out.texts.extend(texts);
        }
        if i == 0 {
            let fin = match m.get("ext").and_then(|e| e.get("is_finish")) {
                Some(Value::String(s)) => s == "1",
                Some(Value::Number(n)) => n.as_i64() == Some(1),
                _ => false,
            };
            out.finished = fin;
        }
    }
    out.images = images.entries();
    out
}

/// Mã lỗi Dola đã gặp → giải thích (`DOLA_ERROR_CODES`, `:940-952`).
pub const KNOWN_ERRORS: [(&str, &str); 8] = [
    (
        "710022002",
        "Dola gắn cờ TÀI KHOẢN này (shark block) — đổi tài khoản khác",
    ),
    (
        "710022013",
        "Dola từ chối xử lý (system error) — thường do hết credit ngày, model/thời lượng quá đắt, hoặc tài khoản/IP bị hạn chế",
    ),
    ("710022004", "Dola bắt xác minh captcha (risk control)"),
    ("710010202", "Request thiếu/sai chữ ký (X-Gorgon)"),
    (
        "712010202",
        "Sai conversation (chat không thuộc tài khoản này)",
    ),
    ("712017001", "Dữ liệu không tồn tại (chat đã bị xoá)"),
    ("710020202", "Tham số không hợp lệ (body bị sửa)"),
    (
        "710012001",
        "Phiên đăng nhập không còn hợp lệ — Đăng nhập lại tài khoản này",
    ),
];

/// Giải thích tiếng Việt cho mã lỗi Dola nằm trong `err` (rỗng nếu chưa biết).
#[must_use]
pub fn explain_error(err: &str) -> &'static str {
    KNOWN_ERRORS
        .iter()
        .find(|(code, _)| err.contains(code))
        .map_or("", |(_, why)| why)
}

static LOGGED_OUT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)ログイン|log ?in|đăng nhập|session (?:expired|invalid)|712012001|710010202")
        .unwrap_or_else(|_| unreachable!())
});

/// Lỗi có dấu hiệu mất phiên (`looks_logged_out`, `:1090-1094`).
#[must_use]
pub fn looks_logged_out(err: &str) -> bool {
    !err.is_empty() && LOGGED_OUT_RE.is_match(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sse_tach_dong_qua_nhieu_manh_va_bo_data_rong() {
        let mut p = SseParser::new();
        let mut ev =
            p.push(b"event: SSE_ACK\ndata: {\"ack_client_meta\":{\"conversation_id\":\"c1\"");
        assert!(ev.is_empty());
        ev.extend(p.push(b"}}\r\n\nevent: X\ndata: {}\ndata: hong\n"));
        ev.extend(p.push(b"event: FULL_MSG_NOTIFY\ndata: {\"a\":1}"));
        ev.extend(p.finish());
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].0.as_deref(), Some("SSE_ACK"));
        assert_eq!(ev[1].0.as_deref(), Some("FULL_MSG_NOTIFY"));
    }

    #[test]
    fn chat_stream_gom_conv_chu_video_va_dung_o_loi() {
        let mut s = ChatStream::new("");
        s.feed(
            Some("SSE_ACK"),
            &json!({"ack_client_meta": {"conversation_id": "c9", "section_id": "s1"}}),
        );
        s.feed(
            Some("STREAM_CHUNK"),
            &json!({"content": {"content_block": [{"content": {"text_block": {"text": "Xin "}}}]},
                    "patch_op": [{"patch_value": {"content_block": [{"content": {"text_block": {"text": "chao"}}}]}}]}),
        );
        s.feed(
            Some("FULL_MSG_NOTIFY"),
            &json!({"x": {"creations": [{"type": 2, "video": {"vid": "v1", "status": 3}}]}}),
        );
        s.feed(
            Some("STREAM_ERROR"),
            &json!({"error_code": 710_022_013, "error_msg": "x"}),
        );
        s.feed(
            Some("STREAM_CHUNK"),
            &json!({"content": {"text": "bo qua"}}),
        );
        assert_eq!(s.conversation_id, "c9");
        assert_eq!(s.section_id, "s1");
        assert_eq!(s.text, "Xin chao");
        assert_eq!(s.video.unwrap()["vid"], "v1");
        assert_eq!(s.error_code, "710022013");
        assert!(explain_error(&s.error).contains("system error"));
    }

    #[test]
    fn tu_choi_noi_dung_content_type_1_lay_chu() {
        let mut s = ChatStream::new("c");
        s.feed(
            Some("FULL_MSG_NOTIFY"),
            &json!({"content": {"text": "ご希望の"}}),
        );
        assert_eq!(s.text, "ご希望の");
    }

    #[test]
    fn extract_urls_chon_khung_cao_nhat_giai_base64() {
        use base64::Engine as _;
        let enc = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
        let vm = json!({"video_list": {
            "a": {"vheight": 720, "vwidth": 404, "main_url": enc("https://x/720.mp4")},
            "b": {"vheight": 1280, "vwidth": 720, "main_url": enc("https://x/1280.mp4").trim_end_matches('=')}
        }});
        let v = json!({"vid": "v", "status": 3, "download_url": "https://x/wm.mp4",
                       "video_model": serde_json::to_string(&vm).unwrap()});
        let u = extract_urls(&v);
        assert_eq!(u.nowm_url, "https://x/1280.mp4");
        assert_eq!(u.width, Some(720));
        assert_eq!(u.download_url, "https://x/wm.mp4");
    }

    #[test]
    fn parse_chain_bo_tin_user_uu_tien_video_xong_va_is_finish_tin_dau() {
        let blocks = |st: i64| {
            serde_json::to_string(&json!([{"content": {"creation_block": {"creations": [{"type": 2, "video": {"status": st}}]}}},
                                          {"content": {"text_block": {"text": format!("st{st}")}}}])).unwrap()
        };
        let resp = json!({"downlink_body": {"pull_singe_chain_downlink_body": {"messages": [
            {"index_in_conv": "7", "section_id": "s", "user_type": 2, "content": blocks(1), "ext": {"is_finish": "1"}},
            {"index_in_conv": 5, "user_type": 2, "content": blocks(3)},
            {"index_in_conv": 9, "user_type": 1, "content": blocks(3)},
            {"index_in_conv": 2, "user_type": 2, "content": "{\"text\":\"tu choi\"}"}
        ]}}});
        let p = parse_chain(&resp);
        assert_eq!(p.last_index, 9);
        assert_eq!(p.section_id, "s");
        assert!(p.finished);
        assert_eq!(p.video.unwrap()["status"], 3);
        assert_eq!(p.texts, ["st1", "st3", "tu choi"]);
    }

    #[test]
    fn gom_anh_uu_tien_ori_raw_va_uri_tu_key_hoac_url() {
        let mut c = ImageCollector::new();
        c.collect(&json!({"creations": [
            {"image": {"image_thumb": {"url": "https://p/tos-mya-i-a/rc_gen_image/abc.jpeg~tplv-t.jpg"},
                       "image_ori_raw": {"url": "https://p/tos-mya-i-a/rc_gen_image/abc.jpeg~tplv-r.jpg", "width": 2048, "height": 2048}}},
            {"image": {"image_ori": {"url": "https://p/tos-b/xyz.png?x=1", "width": 10, "height": 20, "key": "tos-b/xyz.png"}}},
            {"image": {"image_ori": {"url": "ftp://bo-qua"}}}
        ]}));
        let e = c.entries();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].id, "abc.jpeg");
        assert!(e[0].url.contains("tplv-r"));
        assert_eq!(e[0].uri, "tos-mya-i-a/rc_gen_image/abc.jpeg");
        assert_eq!(e[1].uri, "tos-b/xyz.png");
        assert_eq!((e[1].width, e[1].height), (10, 20));
    }

    #[test]
    fn walk_video_models_ca_json_long_trong_chuoi() {
        let inner = serde_json::to_string(&json!({"video_model": "VM2"})).unwrap();
        let v = json!({"a": {"video_model": "VM1"}, "b": [inner], "c": {"video_model": ""}});
        let mut out = Vec::new();
        walk_video_models(&v, &mut out);
        assert_eq!(out, ["VM1", "VM2"]);
    }

    #[test]
    fn mat_phien_nhan_dien_theo_chu_va_ma() {
        assert!(looks_logged_out("vui long ログイン"));
        assert!(looks_logged_out("code 710010202"));
        assert!(!looks_logged_out(""));
        assert!(!looks_logged_out("710022013"));
    }
}
