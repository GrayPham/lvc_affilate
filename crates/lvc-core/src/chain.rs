//! Phan tich JSON tra ve tu `POST /im/chain/single` (Dola: pull hoi thoai) va vai ham phu tro
//! khi phat hien hoi thoai (conversation) MOI sau khi gui. Thuan — khong I/O.
//!
//! Nguon: repo Python `lvcseedance`:
//! - `hub_browser.py:746` `LOGIN_INVALID_CODES`, `hub_browser.py:755-793` `parse_chain` (ban
//!   Python cua `POLL_JS`, dung trong `HttpSession.poll` khi khong con trinh duyet — profile-mode,
//!   `hub_browser.py:853-873`).
//! - `video_worker.py:19-115` `POLL_JS` (JS goc ma `parse_chain` bat chuoc; hai ham LECH nhau vai
//!   chi tiet nho vi duoc viet/bao tri doc lap — xem muc "Khac biet" ben duoi). Theo dung
//!   task-2-brief: cho o dau lech thi PORT THEO `POLL_JS`.
//! - `video_worker_ui.py:678-689` `pick_new_conv_id`, `:1432-1456` `video_ids`/`link_key`.
//!
//! # Khac biet `parse_chain` (Python) vs `POLL_JS` (JS) da phat hien khi doc nguon
//! 1. **So sanh `user_type == 2`**: Python (`hub_browser.py:763`) dung
//!    `str(m.get("user_type")) == "2"` (so CHUOI); JS (`video_worker.py:72`) dung
//!    `Number(m.user_type) === 2` (so SO). Vi du `user_type` la `2.0` (float): Python
//!    `str(2.0)` = `"2.0"` != `"2"` -> KHONG khop; JS `Number(2.0) === 2` -> khop. Ham
//!    `user_type_is_bot` o day theo `POLL_JS` (so sanh bang so).
//! 2. **Fallback DOM khi `latest` rong** (`POLL_JS` dong 103-112 — doc
//!    `document.querySelectorAll(...)` lay tin nhan cuoi tren TRANG khi khong tim duoc `latest`
//!    tu JSON). `parse_chain` (Python, chay trong `HttpSession` KHONG co trinh duyet/DOM —
//!    `hub_browser.py:796` tro xuong) KHONG co doan nay. Ham `parse_chain` (Rust) o day nhan
//!    `serde_json::Value` thuan, KHONG co DOM nen o DIEM NAY buoc phai theo `parse_chain`
//!    (Python), khong phai `POLL_JS` — day la truong hop DOM khong the bieu dien duoc bang JSON
//!    dau vao, khac voi diem 1 (mot loi logic co the port day du sang JSON).
//! 3. Ca hai deu tra `{"ok": true, "status": 200, ...}` cung dinh khi toi duoc buoc parse; `POLL_JS`
//!    con tra `{ok:false,...}` khi `!resp.ok` (loi HTTP) va Python (`hub_browser.py:864-865`,
//!    `:870-872`) kiem `resp.status`/`status_code` TRUOC khi goi `parse_chain` — hai buoc kiem do
//!    nam NGOAI ham `parse_chain` (than no luon tra `ok:true`/`status:200`). Ham `chain_http_status`
//!    o day port rieng nhanh `status_code` (`hub_browser.py:869-872`); nhanh `resp.status != 200`
//!    thuoc lop HTTP (Task 10/11), khong the port trong ham thuan chi nhan body JSON.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// `hub_browser.py:746` `LOGIN_INVALID_CODES` — ma `status_code` cua Dola nghia la
/// "登录校验不合法" (sessionid sai/het han) -> phai bao 401 thay vi 500 chung chung.
pub const LOGIN_INVALID_CODES: &[i64] = &[712_012_001];

/// `hub_browser.py:751` `_LINK_RE` / `POLL_JS` dong 73 `linkRe`.
static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s)\]"'<>]+"#).expect("LINK_RE hop le"));

/// `hub_browser.py:752` `_VIDEO_LINK_RE` / `POLL_JS` dong 88.
static VIDEO_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)mime_type=video_mp4|\.mp4(\?|$)|/video/").expect("VIDEO_LINK_RE hop le")
});

/// `video_worker_ui.py:1432` `VIDEO_ID_RE`. Cong khai (Task 6) de `lvc-browser::dola::labels`
/// re-export thay vi chep lai.
pub static VIDEO_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(v[0-9a-z]{10,})\b").expect("VIDEO_ID_RE hop le"));

/// Ket qua `parse_chain` — cung hinh dang object `POLL_JS` tra ve (`video_worker.py:113`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChainPoll {
    pub ok: bool,
    pub status: u16,
    /// Moi `text_block.text` gap duoc, cat toi da 1000 KY TU (`POLL_JS` `text.slice(0,1000)`,
    /// dong 83; o day dung `chars()` — dem theo codepoint nhu Python `text[:1000]`, co the lech
    /// JS mot chut voi ky tu ngoai BMP hiem gap, xem `truncate_chars`).
    pub texts: Vec<String>,
    /// `creation_block.creations[type==2].video.download_url` bat dau bang `"http"`.
    pub videos: Vec<String>,
    /// Song song voi `videos` — `creations[..].video.video_model` (chuoi rong neu thieu).
    pub video_models: Vec<String>,
    /// Link video (markdown/plain) tim thay trong `texts`, khop `VIDEO_LINK_RE`, khong trung.
    pub links: Vec<String>,
    /// Cac khoi chu CUA TIN BOT MOI NHAT (`user_type == 2`); khong tin nao co `user_type` ->
    /// tin dau tien (`messages[0]`). Cat toi da 4000 ky tu moi khoi.
    pub latest: Vec<String>,
}

/// Cat `s` toi da `n` KY TU (codepoint) — `POLL_JS` dung `.slice(0,n)` (UTF-16 code unit); Python
/// dung `[:n]` (codepoint). Chon codepoint (giong Python) vi cac khoi chu Dola thuc te (Nhat/Viet)
/// khong co ky tu ngoai BMP nen hai cach cho cung ket qua trong pham vi du lieu that.
fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// So sanh "la tin cua bot". CA HAI ban goc deu chap nhan `user_type` la SO LAN CHUOI so:
/// Python (`hub_browser.py:763`) `str(m.get("user_type")) == "2"` khop voi so nguyen `2` (vi
/// `str(2)=="2"`) LAN chuoi `"2"` (vi `str("2")=="2"`); JS (`video_worker.py:72`)
/// `Number(m.user_type) === 2` khop voi ca hai qua ep kieu `Number(...)`. Fix round 1 (RULING):
/// ban truoc chi xu ly `Value::Number`, bo sot `Value::String` -> voi `user_type: "2"` (chuoi)
/// thi `hasType` van dung (co gia tri, khong null) nhung khong tin nao khop "la bot" -> `latest`
/// RONG thay vi la tin bot — sai giong mis-read Python da sua 2026-09-17 (b4f68ec).
///
/// Diem LECH THAT giua hai ban (van giu, xem doc comment dau file muc 1) chi o cach so sanh SO:
/// `user_type = 2.0` (float) -> Python `str(2.0)="2.0"!="2"` KHONG khop, JS `Number(2.0)===2`
/// KHOP; o day theo `POLL_JS` (so sanh xap xi thay vi `==` truc tiep tren f64 de tranh
/// `clippy::float_cmp`).
fn user_type_is_bot(v: Option<&Value>) -> bool {
    let Some(v) = v else { return false };
    match v {
        // JS `Number("2")` (trim khoang trang, parse so) — Python `str("2")=="2"` cung khop.
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .is_ok_and(|f| (f - 2.0).abs() < f64::EPSILON),
        _ => match v.as_i64() {
            Some(n) => n == 2,
            // JSON "2.0" duoc serde_json luu duoi dang f64 (as_i64() = None) -> so sanh xap xi
            // de tranh so sanh f64 truc tiep (clippy::float_cmp).
            None => v.as_f64().is_some_and(|f| (f - 2.0).abs() < f64::EPSILON),
        },
    }
}

/// `msg.content` -> danh sach block. `content` co the la MANG san, hoac CHUOI JSON encode mang do
/// (`parse_chain`/`POLL_JS` deu `JSON.parse`/`json.loads` truoc). Parse loi hoac ket qua khong
/// phai mang -> tra mang rong (bo qua, KHONG panic) — `hub_browser.py:766-772`, `POLL_JS` dong
/// 76-79.
fn message_content_blocks(msg: &Value) -> Vec<Value> {
    match msg.get("content") {
        Some(Value::String(s)) => match serde_json::from_str::<Value>(s) {
            Ok(Value::Array(arr)) => arr,
            _ => Vec::new(),
        },
        Some(Value::Array(arr)) => arr.clone(),
        _ => Vec::new(),
    }
}

/// `hub_browser.py:755-793` `parse_chain` + `POLL_JS` `video_worker.py:19-115` (than ham, khong
/// gom phan fetch HTTP/DOM fallback — xem doc comment dau file).
#[must_use]
pub fn parse_chain(body: &Value) -> ChainPoll {
    let empty = Vec::new();
    let messages: &Vec<Value> = body
        .get("downlink_body")
        .and_then(|d| d.get("pull_singe_chain_downlink_body"))
        .and_then(|d| d.get("messages"))
        .and_then(Value::as_array)
        .unwrap_or(&empty);

    // POLL_JS dong 71 `hasType` / hub_browser.py:760 `has_type`.
    let has_type = messages
        .iter()
        .any(|m| m.get("user_type").is_some_and(|v| !v.is_null()));
    // POLL_JS dong 72 `latestIdx`; hub_browser.py:761-763 `latest_idx` (khac cach so sanh, xem
    // `user_type_is_bot`). `Some(0)` khi khong co `user_type` nao; `None` khi CO user_type nhung
    // khong tin nao la bot (khong tin nao khop `mi == latest_idx`, giu nguyen `-1` cua ban goc).
    let latest_idx: Option<usize> = if has_type {
        messages
            .iter()
            .position(|m| user_type_is_bot(m.get("user_type")))
    } else {
        Some(0)
    };

    let mut texts = Vec::new();
    let mut videos = Vec::new();
    let mut video_models = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut latest = Vec::new();

    for (mi, msg) in messages.iter().enumerate() {
        for block in message_content_blocks(msg) {
            let Some(block_obj) = block.as_object() else {
                continue;
            };
            let text = block_obj
                .get("content")
                .and_then(|c| c.get("text_block"))
                .and_then(|t| t.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if !text.is_empty() {
                texts.push(truncate_chars(text, 1000));
                if latest_idx == Some(mi) {
                    latest.push(truncate_chars(text, 4000));
                }
                for m in LINK_RE.find_iter(text) {
                    let u = m.as_str();
                    if VIDEO_LINK_RE.is_match(u) && !links.iter().any(|l| l == u) {
                        links.push(u.to_string());
                    }
                }
            }
            if block_obj.get("block_type").and_then(Value::as_i64) != Some(2074) {
                continue;
            }
            let creations = block_obj
                .get("content")
                .and_then(|c| c.get("creation_block"))
                .and_then(|cb| cb.get("creations"))
                .and_then(Value::as_array);
            let Some(creations) = creations else {
                continue;
            };
            for cre in creations {
                let Some(cre_obj) = cre.as_object() else {
                    continue;
                };
                if cre_obj.get("type").and_then(Value::as_i64) != Some(2) {
                    continue;
                }
                let video = cre_obj.get("video");
                let url = video
                    .and_then(|v| v.get("download_url"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if url.starts_with("http") {
                    videos.push(url.to_string());
                    let model = video
                        .and_then(|v| v.get("video_model"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    video_models.push(model.to_string());
                }
            }
        }
    }

    ChainPoll {
        ok: true,
        status: 200,
        texts,
        videos,
        video_models,
        links,
        latest,
    }
}

/// `hub_browser.py:869-872` (than `HttpSession.poll`, KHONG phai `parse_chain`):
/// `code = data.get("status_code"); if code not in (0, None): ... 401 neu code in
/// LOGIN_INVALID_CODES, else 500 ... else parse_chain(data)`. Fix round 2 (RULING): port DUNG
/// NGU NGHIA Python `in`/`==`, gom ca hai quirk deu bat nguon tu `bool` la subclass cua `int` va
/// `==` giua so/khong phan biet int/float trong Python:
/// - `False == 0` -> JSON `false` cung la "khong loi" (tra `None`), giong het `0`/`0.0`; JSON
///   `true` (`True == 1`) -> RE loi (`code not in (0, None)` dung) nhung khong khop
///   `LOGIN_INVALID_CODES` -> 500.
/// - So sanh voi `LOGIN_INVALID_CODES` la so sanh SO: `712012001.0` (float) van `==` `712012001`
///   (int) -> 401 (Python khong phan biet kieu so khi so sanh gia tri).
/// - Chuoi (`""`, `"712012001"`, ...) trong Python KHONG BAO GIO `==` mot so nguyen (khac kieu)
///   -> `code not in (0, None)` LUON dung (ke ca `""`/chuoi so cung roi vao nhanh loi) va
///   KHONG BAO GIO khop `LOGIN_INVALID_CODES` du trung chu so -> LUON 500 (ke ca chuoi rong).
///   Mang/object cung khong phai so -> cung nhanh 500 (khong the `== 0` hay `==` ma loi nao).
#[must_use]
pub fn chain_http_status(body: &Value) -> Option<u16> {
    let code = match body.get("status_code") {
        None | Some(Value::Null) => return None,
        Some(v) => v,
    };
    // Quy ve so (f64) neu Python coi la so: `Number` giu nguyen; `Bool` theo dung quy tac
    // Python `False == 0`/`True == 1`. Chuoi/mang/object -> `None` (Python khong ep kieu ngam
    // khi so sanh `==` voi so) -> roi thang xuong nhanh 500 ben duoi.
    let as_number: Option<f64> = match code {
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Number(n) => n.as_f64(),
        _ => None,
    };
    let Some(f) = as_number else {
        return Some(500);
    };
    if f.abs() < f64::EPSILON {
        return None; // Python: code in (0, None) -> 0/0.0/False
    }
    // PF-3: LOGIN_INVALID_CODES la ma loi Dola (< 10 chu so) nen luon bieu dien CHINH XAC trong
    // f64 (mantissa 53 bit du cho gia tri nay) — ep kieu an toan trong pham vi du lieu thuc te,
    // du clippy canh bao mat chinh xac mot cach tong quat cho moi i64 -> f64.
    #[allow(clippy::cast_precision_loss)]
    let is_login_invalid = LOGIN_INVALID_CODES
        .iter()
        .any(|&c| (f - c as f64).abs() < f64::EPSILON);
    Some(if is_login_invalid { 401 } else { 500 })
}

/// `video_worker_ui.py:1432-1437` `VIDEO_ID_RE`/`video_ids` — ma video `ByteDance` (vd
/// `v186a3gm000cdal5gdvog65tvg6d5png`) xuat hien trong link/model. Python tra `set`; o day tra
/// `Vec<String>` da khu trung, GIU THU TU xuat hien dau tien (de test/log on dinh hon set).
#[must_use]
pub fn video_ids(s: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for cap in VIDEO_ID_RE.captures_iter(s) {
        let id = cap[1].to_lowercase();
        if seen.insert(id.clone()) {
            out.push(id);
        }
    }
    out
}

/// Tach (netloc, path) tho gian luoc kieu `urllib.parse.urlsplit` — chi du dung cho `link_key`
/// (link Dola dang `scheme://host/path?query`); KHONG phai parser URL day du (lvc-core phai
/// thuan, khong co crate `url` ngoai workspace — xem rang buoc task-2-brief).
fn split_netloc_path(u: &str) -> (&str, &str) {
    let (has_authority, rest) = if let Some(idx) = u.find("://") {
        (true, &u[idx + 3..])
    } else if let Some(stripped) = u.strip_prefix("//") {
        (true, stripped)
    } else {
        (false, u)
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    if !has_authority {
        return ("", rest);
    }
    match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    }
}

/// `video_worker_ui.py:1440-1456` `link_key` — khoa ON DINH cua link video (host + path tu
/// `/video/`, bo query) de nhan ra "cung mot video" qua cac lan poll (link Dola ky lai moi lan
/// tai trang). Rong (khong co host/path) -> tra nguyen `url` (giong Python `... or u`).
#[must_use]
pub fn link_key(url: &str) -> String {
    let (netloc, path) = split_netloc_path(url);
    let stable = match path.find("/video/") {
        Some(idx) => &path[idx..],
        None => path,
    };
    let stable = stable.trim_end_matches('/');
    let netloc_lower = netloc.to_lowercase();
    let combined = format!("{netloc_lower}{stable}");
    if combined.is_empty() {
        url.to_string()
    } else {
        combined
    }
}

/// Ket qua `pick_new_conv_id` — `video_worker_ui.py:678-689`, phan THUAN (khong nhan `recent`,
/// khong tra `jumped`: buoc do can goi HTTP lay "chat gan day", thuoc Task 10/11 o lop khong thuan).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvPick {
    /// Duoi URL la chuoi so va CHUA co trong `known` -> chac chan la hoi thoai MOI.
    New(String),
    /// Duoi URL la chuoi so nhung DA co trong `known` (URL roi vao chat cu) -> caller (Task
    /// 10/11) can tra them danh sach "chat gan day" de quyet dinh co phai "jumped" khong.
    Known(String),
    /// Duoi URL khong phai chuoi so (vd "chat") -> khong doan duoc.
    None,
}

/// `video_worker_ui.py:678-689` `pick_new_conv_id`, phan xac dinh id THUAN.
// PF-3: task-2-brief dinh nghia chu ky ham nay VOI KIEU CU THE `&HashSet<String>` (khong generic
// hoa theo BuildHasher) — giu dung interface da chot, khong tu y doi rong ra ngoai pham vi brief.
#[allow(clippy::implicit_hasher)]
#[must_use]
pub fn pick_new_conv_id(url_tail: &str, known: &HashSet<String>) -> ConvPick {
    let tail = url_tail.trim();
    if tail.is_empty() || !tail.chars().all(|c| c.is_ascii_digit()) {
        return ConvPick::None;
    }
    if known.contains(tail) {
        ConvPick::Known(tail.to_string())
    } else {
        ConvPick::New(tail.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashSet;

    #[test]
    fn latest_la_tin_bot_moi_nhat_khong_phai_messages_0() {
        let body = json!({
            "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
                { "user_type": 1, "content": [
                    { "content": { "text_block": { "text": "missing parameters, please confirm" } } }
                ] },
                { "user_type": 2, "content": [
                    { "content": { "text_block": { "text": "Đây là video của bạn" } } },
                    { "block_type": 2074, "content": { "creation_block": { "creations": [
                        { "type": 2, "video": { "download_url": "http://v16-dola.dola.com/video/abc.mp4", "video_model": "{}" } }
                    ] } } }
                ] }
            ] } }
        });
        let r = parse_chain(&body);
        assert_eq!(r.latest, vec!["Đây là video của bạn".to_string()]);
        assert_eq!(
            r.videos,
            vec!["http://v16-dola.dola.com/video/abc.mp4".to_string()]
        );
        assert_eq!(r.video_models, vec!["{}".to_string()]);
        assert_eq!(r.texts.len(), 2);
        assert!(r.ok);
        assert_eq!(r.status, 200);
    }

    #[test]
    fn user_type_dang_chuoi_so_cung_duoc_nhan_la_bot() {
        // Fix round 1: user_type = "2" (CHUOI, khong phai so) — Python `str(m.get("user_type"))
        // == "2"` va JS `Number(m.user_type) === 2` DEU khop; day KHONG phai diem lech giua hai
        // ban goc (xem "Khac biet" dau file, muc 1 chi noi ve 2 vs 2.0), ma la truong hop co ban
        // truoc do chua duoc xu ly (bug that: hasType dung nhung latest_idx tra None -> latest
        // rong, khong phai loi "roi ve messages[0]" nhu mo ta ban dau, nhung van la SAI so voi
        // Python/POLL_JS — sua boi RULING vong 1).
        let body = json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "user_type": "2", "content": [ { "content": { "text_block": { "text": "tin bot dang chuoi" } } } ] }
        ] } } });
        let r = parse_chain(&body);
        assert_eq!(r.latest, vec!["tin bot dang chuoi".to_string()]);
    }

    #[test]
    fn user_type_chuoi_1_roi_2_chon_dung_tin_bot() {
        // Fix round 1: tin[0] user_type "1" (chuoi), tin[1] user_type "2" (chuoi) -> latest phai
        // la tin[1], khong phai roi ve tin[0] (day chinh la mis-read Python sua 2026-09-17,
        // b4f68ec, neu latest_idx sai ve 0/None).
        let body = json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "user_type": "1", "content": [ { "content": { "text_block": { "text": "prompt cua tool" } } } ] },
            { "user_type": "2", "content": [ { "content": { "text_block": { "text": "tra loi cua bot" } } } ] }
        ] } } });
        let r = parse_chain(&body);
        assert_eq!(r.latest, vec!["tra loi cua bot".to_string()]);
    }

    #[test]
    fn khong_co_user_type_thi_latest_la_tin_dau() {
        let body = json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "content": [ { "content": { "text_block": { "text": "tin dau" } } } ] },
            { "content": [ { "content": { "text_block": { "text": "tin sau" } } } ] }
        ] } } });
        let r = parse_chain(&body);
        assert_eq!(r.latest, vec!["tin dau".to_string()]);
    }

    #[test]
    fn content_la_chuoi_json_van_parse_duoc() {
        let inner = json!([ { "content": { "text_block": { "text": "tin qua chuoi" } } } ]);
        let body = json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "user_type": 2, "content": inner.to_string() }
        ] } } });
        let r = parse_chain(&body);
        assert_eq!(r.texts, vec!["tin qua chuoi".to_string()]);
        assert_eq!(r.latest, vec!["tin qua chuoi".to_string()]);
    }

    #[test]
    fn content_khong_parse_duoc_thi_bo_qua_khong_panic() {
        let body = json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "user_type": 2, "content": "{ khong phai json hop le" }
        ] } } });
        let r = parse_chain(&body);
        assert!(r.texts.is_empty());
        assert!(r.latest.is_empty());
    }

    #[test]
    fn link_markdown_duoc_tach_dung() {
        let body = json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "user_type": 2, "content": [ { "content": { "text_block": {
                "text": "[動画を再生する](http://v16-dola.dola.com/x/video/abc.mp4?x=1)"
            } } } ] }
        ] } } });
        let r = parse_chain(&body);
        assert_eq!(
            r.links,
            vec!["http://v16-dola.dola.com/x/video/abc.mp4?x=1".to_string()]
        );
    }

    #[test]
    fn download_url_khong_bat_dau_http_thi_bo() {
        let body = json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "user_type": 2, "content": [
                { "block_type": 2074, "content": { "creation_block": { "creations": [
                    { "type": 2, "video": { "download_url": "ftp://x/y.mp4", "video_model": "m" } }
                ] } } }
            ] }
        ] } } });
        let r = parse_chain(&body);
        assert!(r.videos.is_empty());
        assert!(r.video_models.is_empty());
    }

    #[test]
    fn chain_http_status_theo_ma_loi() {
        assert_eq!(
            chain_http_status(&json!({ "status_code": 712_012_001 })),
            Some(401)
        );
        assert_eq!(chain_http_status(&json!({ "status_code": 5 })), Some(500));
        assert_eq!(chain_http_status(&json!({ "status_code": 0 })), None);
        assert_eq!(chain_http_status(&json!({})), None);
    }

    #[test]
    fn chain_http_status_bool_va_float_nhu_python() {
        // Fix round 2: Python `False == 0` -> JSON `false` cung la "khong loi".
        assert_eq!(chain_http_status(&json!({ "status_code": false })), None);
        // Python `True == 1`, khong khop LOGIN_INVALID_CODES -> 500.
        assert_eq!(
            chain_http_status(&json!({ "status_code": true })),
            Some(500)
        );
        // Python so sanh so khong phan biet int/float: 712012001.0 == 712012001 -> 401.
        assert_eq!(
            chain_http_status(&json!({ "status_code": 712_012_001.0 })),
            Some(401)
        );
        // 0.0 cung la "khong loi" nhu 0.
        assert_eq!(chain_http_status(&json!({ "status_code": 0.0 })), None);
    }

    #[test]
    fn chain_http_status_chuoi_khong_bao_gio_khop_so_nhu_python() {
        // Fix round 2: chuoi KHONG BAO GIO == so trong Python -> luon roi vao nhanh loi (500),
        // KE CA chuoi rong (""  != 0 va "" is not None) va chuoi trung chu so voi ma loi
        // (khac kieu, khong the ==).
        assert_eq!(chain_http_status(&json!({ "status_code": "" })), Some(500));
        assert_eq!(
            chain_http_status(&json!({ "status_code": "712012001" })),
            Some(500)
        );
    }

    #[test]
    fn video_id_re_cong_khai_khop_python_1432() {
        // Task 6: `VIDEO_ID_RE` phai cong khai de `lvc-browser::dola::labels` re-export (khong chep lai).
        assert_eq!(VIDEO_ID_RE.as_str(), r"(?i)\b(v[0-9a-z]{10,})\b");
        assert!(crate::VIDEO_ID_RE.is_match("v186a3gm000cdal5gdvog65tvg6d5png"));
    }

    #[test]
    fn video_ids_bat_ma_v_it_nhat_10_ky_tu() {
        let ids = video_ids("https://cdn.example.com/v0d1234567890abc/foo");
        assert_eq!(ids, vec!["v0d1234567890abc".to_string()]);
    }

    #[test]
    fn pick_new_conv_id_ba_truong_hop() {
        let known: HashSet<String> = ["999".to_string()].into_iter().collect();
        assert_eq!(
            pick_new_conv_id("123456", &known),
            ConvPick::New("123456".to_string())
        );
        assert_eq!(
            pick_new_conv_id("999", &known),
            ConvPick::Known("999".to_string())
        );
        assert_eq!(pick_new_conv_id("chat", &known), ConvPick::None);
    }

    #[test]
    fn link_key_giu_host_va_path_tu_video_bo_query() {
        let k = link_key(
            "http://v16-dola.dola.com/abc123/1234567890/video/tos/mya/tos-mya-v-50851/deadbeef/?x=1&x-signature=sig",
        );
        assert_eq!(
            k,
            "v16-dola.dola.com/video/tos/mya/tos-mya-v-50851/deadbeef"
        );
    }

    #[test]
    fn link_key_khong_co_video_thi_dung_ca_path() {
        let k = link_key("http://Host.Example.com/a/b.mp4?x=1");
        assert_eq!(k, "host.example.com/a/b.mp4");
    }
}
