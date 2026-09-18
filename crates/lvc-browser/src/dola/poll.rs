//! Vong poll va quyet dinh tai video (Task 11, P2) — port `video_worker_ui.py` `poll_conversation`
//! (`:1475-1525`) + `_handle_poll` (`:1275-1429`) + `_download_result` (`:1265-1272`) +
//! `_raw_for_link` (`:1459-1472`) + `unwatermark.resolve_raw_video` (`:243-251`).
//!
//! Chia hai lop: [`handle_poll`] THUAN (khong I/O — nhan `ChainPoll` + trang thai + `now` + chu DOM
//! da doc san, tra [`PollDecision`]); [`poll_and_download_at`] la vong async: doc trang qua trait
//! [`PollPage`] (cookie, captcha, DOM, gui tra loi), poll HTTP qua [`DolaHttp`], roi thuc hien
//! quyet dinh (lay ban raw, tai file). Moi quyet dinh mang → chi o vong async.
//!
//! | Rust | Python | Ghi chu |
//! |---|---|---|
//! | [`poll_and_download`] / [`poll_and_download_with`] / [`poll_and_download_at`] | `poll_conversation` `:1475-1525` | vong lap; `_at` nhan endpoint (test wiremock) |
//! | — captcha moi nhip | (khong co) | ruling Task 9 I-1: [`super::has_captcha`] → [`super::captcha_error`] |
//! | — cookie | `:1486-1487` doc 1 lan | ruling #3: doc lai MOI nhip tu CDP + de `Set-Cookie` cua lan truoc ([`merge_cookie_header`]) |
//! | — `sleep` | `:1499` `asyncio.sleep(5)` | `Limits::poll_interval_s`, TRUOC poll (ke ca lan dau) |
//! | — `pull_chain` loi | `:1500-1509` | "closed" → [`DolaError::BrowserClosed`] ([`is_browser_closed_msg`], tu loi CDP); loi HTTP → `warn` + `continue` |
//! | — 401/403 | `:1510-1512` `is_auth_status` | [`DolaError::LoggedOut`] `revoked: false` |
//! | — `on_poll` | `:1514-1517` | ≤ 1 lan / 30 s ([`ON_POLL_EVERY`]), lan dau ngay |
//! | [`should_extend`] | `modes.py:361-363` | gia han khi agent con noi |
//! | — timeout | `:1522-1524` | `TIMEOUT: No video generated within {N}s (conversation_id={id}) \| agent: {duoi 400 ky tu}` ([`latest_tail`]) |
//! | [`handle_poll`] | `_handle_poll` `:1275-1429` | bang thu tu duoi |
//! | [`resolve_raw_video`] | `unwatermark.resolve_raw_video` `:243-251` | ≤ 3 `fallback_api`; `Err` cua `fetch_raw_video` coi nhu `None` (ruling #4) |
//! | [`raw_models_for_link`] | `_raw_for_link` `:1459-1472` (phan thuan) | model khop ma video trong link, MOI NHAT truoc |
//! | [`download_result`] | `_download_result` `:1265-1272` | stage `downloading`, [`output_path`] UTC, [`VideoArtifact`] |
//!
//! # Thu tu quyet dinh trong [`handle_poll`] (NOTES §4.4, `_handle_poll`)
//! | # | Python | Rust |
//! |---|---|---|
//! | 1 | `:1280-1283` activity | `latest` doi → `last_activity = now`; `last_latest` giu neu `latest` rong |
//! | 2 | `:1285-1288` balance | **BO** (R7: khong `on_balance`) |
//! | 3 | `:1289-1299` classify tung tin `latest` (bo `looks_like_json`) | `Limit`/`Refusal`/`Credit` → loi, thong diep nhu Python (`text[:120]` / `REFUSAL_NOTE + text[:200]`) |
//! | 4 | `:1300-1320` quet DOM `<main>` | `dom_text` do vong doc bang [`DOM_TAIL_JS`] (5 s, loi nuot); bo qua khi co `ignore_*` (`:1301` chat dung lai); `Refusal` chi khi [`refusal_snippet`] co (Python nuot `AttributeError` khi chi khop policy) |
//! | 5 | `:1321-1343` loc `ignore_*` | theo chuoi, [`link_key`], [`video_ids`] (tinh san trong [`PollState::new`]) |
//! | 6 | `:1344-1353` `policy_rejection(latest)` | khong video/link; `PORTRAIT_NOTE` khi [`is_portrait_hint`] |
//! | 7 | `:1354-1368` `needs_confirmation` | MOI mode nhu Python (khong gate pro-like — fix vong 1); khoa [`confirm_key`], toi da `confirm_max_replies`; tra [`PollDecision::ConfirmReply`] (vong gui + stage `confirm`) |
//! | 8 | `:1369-1375` `agent_question` ≥ `agent_question_grace_s` | [`question_grace`]; luu y `lvc_core::agent_question` KHONG BAO GIO `Some` (parity co chu dich, doc o do) |
//! | 9 | `:1376-1384` `AGENT_IDLE_FAIL` | `latest` la chu, khong phai lenh cua tool (`DIRECTIVE:` + `Content:`), im ≥ `agent_idle_fail_s`, khong `is_generating` → `AGENT_NO_VIDEO` |
//! | 10 | `:1385-1393` videos | [`PollDecision::NeedRaw`] `models = [video_models[0]]`, fallback [`RawFallback::Unwatermarked`] |
//! | 11 | `:1394-1428` links | `NeedRaw` `models` khop ma video; trong `link_raw_grace_s` → [`RawFallback::Wait`]; het → [`RawFallback::Link`]; **`LINK_PREVIEW_ASK` = false** (khong port `:1414-1425`) |
//! | 12 | `:1429` | [`PollDecision::Continue`] |
//!
//! # Khac Python (co chu y)
//! - **Captcha moi nhip** (ruling Task 9 I-1): Python khong kiem captcha khi poll; o day
//!   `has_captcha` TRUOC `pull_chain` moi nhip → `RISK_CONTROL` (pool cooldown).
//! - **Cookie** (ruling #3): Python doc cookie 1 lan (`:1486`) + `HttpSession` giu dict rieng. O day
//!   moi nhip lay chuoi `Cookie` MOI tu CDP (trinh duyet co the xoay `msToken`) lam nen, roi de
//!   cac cap `Set-Cookie` ma poll truoc tra ve (`PullOutcome::set_cookies`, moi nhat thang) len tren:
//!   ten da co → thay gia tri, chua co → them. Neu trinh duyet tu xoay cookie SAU khi ta nhan
//!   `Set-Cookie`, gia tri cua ta van thang (Dola cap cho phien HTTP nay; ca hai deu hop le).
//! - **Confirmation MOI mode** (fix vong 1, ruling): Python `:1354` khong xet mode — port y nguyen
//!   (ban dau gate pro-like theo brief; brief sai). Tham so `accept_links` cua `_handle_poll` khong
//!   duoc doc trong than ham (tham so chet) → `PollOpts::accept_links` giu de khop giao dien, KHONG doc.
//!   `PollOpts::mode` hien chi de log/giao dien (Task 12/14 co the dung).
//! - **`LINK_PREVIEW_ASK`**: `config.py:298` mac dinh `"0"` (tat) — khong port nhanh xin preview
//!   `:1414-1425` (toi uu hoa, can gui chat); hang [`LINK_PREVIEW_ASK`] = `false` ghi lai quyet dinh.
//! - **Gui tra loi xac nhan**: Python `_send_chat_message` (`:1242-1262`) = go + Enter + bam nut gui neu
//!   con chu — [`PollPage::send_reply`] cua `Page` goi [`skill::send_chat_message`] (Task 12).
//! - **`fetch_raw_video` `Err`** (loi mang/timeout) coi nhu `None` + `warn` (Python nuot moi loi
//!   `unwatermark.py:239`), thu `fallback_api` ke tiep (≤ 3). Cong tac `DOLA_RAW_UNWATERMARK` (`:245`)
//!   khong port — luon bat.
//! - **`confirm_key`**: Python sha1 cua `latest` da strip (`modes.py:343-347`) chi de lam khoa dict;
//!   o day dung thang chuoi noi (tap ≤ `confirm_max_replies` phan tu, khong luu dau) — cung ngu nghia.
//! - **Stage**: Python `:1493-1494` chi dat `generating` neu stage hien tai khac; khong co kho stage o
//!   day → `on_stage("generating")` luon goi 1 lan khi bat dau. `on_agent_reply(text)` (brief, khong co
//!   trong Python) goi khi `latest` DOI (cung dieu kien buoc 1) voi cac khoi noi bang `\n`.
//! - **Ten file** UTC (`output_path`, ruling Task 10 #4); Python gio may.
//! - **"Browser closed"**: Python soi `"has been closed"`/`"target closed"` trong loi `page.evaluate`
//!   (`:1503-1506`). Poll o day la HTTP; loi CDP tu `cookie_header`/`has_captcha` moi nhip la noi tuong
//!   duong — [`is_browser_closed_msg`] them cac cach chromiumoxide dien dat khi Chrome dong (`Target
//!   closed`, `Session with given id not found`, `Connection closed`, `receiver is gone`...); loi CDP
//!   khac → `warn` + `continue` nhu `Polling exception` `:1508-1509`.
//!
//! Log: KHONG BAO GIO log cookie, KHONG log URL day du (chi `host/path` — [`url_host_path`]), KHONG
//! log noi dung tra loi gui di (chi do dai).

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use chromiumoxide::Page;
use lvc_core::{
    ChainPoll, DOLA_URL, DolaTextClass, Limits, Mode, PORTRAIT_NOTE, REFUSAL_NOTE, agent_question,
    classify_dola_text, confirmation_reply, extract_unwatermarked_url, find_fallback_apis,
    is_generating, is_portrait_hint, is_watermarked_url, link_key, looks_like_json,
    needs_confirmation, policy_rejection, refusal_snippet, video_ids,
};
use tracing::{debug, info, warn};

use super::error::DolaError;
use super::http::{DolaHttp, FETCH_TIMEOUT, POLL_URL, output_path, url_host_path};
use super::labels::DOM_TAIL_JS;
use super::skill;
use super::submit::{ConversationId, captcha_error, has_captcha};
use crate::BrowserError;
use crate::ops;

/// `_handle_poll` `:1390` — tai duoc ban raw qua `fallback_api`.
pub const NOTE_RAW: &str = " (raw, no logo)";
/// `:1393` — khong co raw, lay `main_url` trong `video_model` (hoac `download_url`).
pub const NOTE_UNWATERMARKED: &str = " (unwatermarked priority)";
/// `:1428` — chi co link trong cau tra loi, khong khop raw sau grace.
pub const NOTE_LINK: &str = " (link in text)";
/// `task_stage.set_stage("generating", conversation_id)` `:1494`.
pub const STAGE_GENERATING: &str = "generating";
/// `task_stage.set_stage("confirm", f"#{n}")` `:1362`.
pub const STAGE_CONFIRM: &str = "confirm";
/// `task_stage.set_stage("downloading", note.strip())` `:1268`.
pub const STAGE_DOWNLOADING: &str = "downloading";
/// `:1516` `now - last_callback >= 30`.
pub const ON_POLL_EVERY: Duration = Duration::from_secs(30);
/// `:1305-1308` `asyncio.wait_for(page.evaluate(...), timeout=5)`.
pub const DOM_TAIL_TIMEOUT: Duration = Duration::from_secs(5);
/// `:1522` `[-400:]` — duoi thong diep timeout (ky tu).
pub const LATEST_TAIL_CHARS: usize = 400;
/// `config.py:298` `LINK_PREVIEW_ASK = os.getenv("DOLA_LINK_PREVIEW_ASK", "0") == "1"` — mac dinh
/// TAT; nhanh xin preview `:1414-1425` KHONG port (xem doc module).
pub const LINK_PREVIEW_ASK: bool = false;
/// `:1296` `text[:120]`, `:1310`/`:1316` `dom_text[:120]`.
const MSG_HEAD_120: usize = 120;
/// `:1298` `text[:200]`.
const MSG_HEAD_200: usize = 200;
/// `:1374` `question[:300]`.
const MSG_HEAD_300: usize = 300;
/// `:1384` `latest_text[:400]`.
const MSG_HEAD_400: usize = 400;
/// `unwatermark.py:249` `find_fallback_apis(video_model)[:3]`.
const MAX_FALLBACK_APIS: usize = 3;

/// Ket qua cuoi cua vong poll (`_download_result` `:1272` tra dict `video_url`/`local_path`/
/// `conversation_id`/`account` + `watermarked` do caller `:1822`/`:2508` them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoArtifact {
    pub video_url: String,
    pub local_path: PathBuf,
    pub conversation_id: ConversationId,
    pub account: String,
    /// `postprocess.is_watermarked_url(video_url)`.
    pub watermarked: bool,
    pub bytes: u64,
    /// [`NOTE_RAW`] | [`NOTE_UNWATERMARKED`] | [`NOTE_LINK`].
    pub note: &'static str,
}

/// Tuy chon mot lan poll (tham so `poll_conversation` `:1475-1479` + `current_duration` `:1360`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollOpts {
    /// `timeout` `:1476` (da qua `submit_timeout`).
    pub timeout: Duration,
    /// Mode cua task (`mode` `:1788`/`:2387`) — KHONG anh huong quyet dinh (fix vong 1: Python
    /// `_handle_poll` khong gate theo mode); giu cho log/giao dien.
    pub mode: Mode,
    /// `current_duration.get()` `:1360` — tra loi khi Dola hoi so giay.
    pub duration: u16,
    /// `ignore_videos` `:1478` — chat dung lai (R5: luon rong o P2).
    pub ignore_videos: HashSet<String>,
    /// `ignore_links` `:1479`.
    pub ignore_links: HashSet<String>,
    /// `accept_links` `:1477` — Python KHONG doc trong `_handle_poll` (tham so chet); giu de khop
    /// giao dien, khong anh huong quyet dinh.
    pub accept_links: bool,
}

impl PollOpts {
    /// `:1301` `state.get("ignore_videos") or state.get("ignore_links")` — dang dung lai chat cu:
    /// bo quet DOM (tin cu tren trang khop credit/limit oan).
    #[must_use]
    pub fn reuses_chat(&self) -> bool {
        !self.ignore_videos.is_empty() || !self.ignore_links.is_empty()
    }
}

/// Callback tien trinh (`on_poll` `:1516-1517`, `task_stage.set_stage` `:1268`/`:1362`/`:1494`;
/// `on_agent_reply` la cua brief — chu moi cua agent de UI hien).
pub trait PollHooks: Send + Sync {
    /// ≤ 1 lan / 30 s, `elapsed` ke tu khi bat dau poll.
    fn on_poll(&self, elapsed: Duration) {
        let _ = elapsed;
    }
    /// [`STAGE_GENERATING`] | [`STAGE_CONFIRM`] | [`STAGE_DOWNLOADING`].
    fn on_stage(&self, stage: &str) {
        let _ = stage;
    }
    /// `latest` doi (buoc 1) — cac khoi noi bang `\n`.
    fn on_agent_reply(&self, text: &str) {
        let _ = text;
    }
}

/// Khong lam gi.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopHooks;

impl PollHooks for NoopHooks {}

/// Ba lan doc trang + mot lan gui trong vong poll, tach khoi `Page` de test vong bang wiremock
/// khong can Chrome (ruling #9). [`Page`] implement bang `ops`/`submit`/`compose`.
#[async_trait]
pub trait PollPage: Sync {
    /// Chuoi header `Cookie` cho `www.dola.com` (`ops::cookie_header`). KHONG log.
    async fn cookie_header(&self) -> Result<String, BrowserError>;
    /// Co iframe `bdcaptcha.html` (`submit::has_captcha`).
    async fn has_captcha(&self) -> Result<bool, BrowserError>;
    /// [`DOM_TAIL_JS`] — 3000 ky tu cuoi cua `<main>`.
    async fn dom_tail(&self) -> Result<String, BrowserError>;
    /// Go `text` vao o soan cua chat dang mo va gui (`_send_chat_message` `:1242-1262`).
    async fn send_reply(&self, text: &str) -> Result<(), BrowserError>;
}

#[async_trait]
impl PollPage for Page {
    async fn cookie_header(&self) -> Result<String, BrowserError> {
        ops::cookie_header(self, DOLA_URL).await
    }

    async fn has_captcha(&self) -> Result<bool, BrowserError> {
        has_captcha(self).await
    }

    async fn dom_tail(&self) -> Result<String, BrowserError> {
        ops::eval_fn::<Option<String>>(self, DOM_TAIL_JS)
            .await
            .map(Option::unwrap_or_default)
    }

    async fn send_reply(&self, text: &str) -> Result<(), BrowserError> {
        // `_send_chat_message` `:1242-1262` (Task 12): go + Enter, con chu thi bam nut gui cuoi.
        skill::send_chat_message(self, text).await
    }
}

/// Trang thai giua cac nhip (dict `state` `:1489-1492` + cac khoa `_handle_poll` them dan).
#[derive(Debug, Clone)]
pub(crate) struct PollState {
    /// `state["confirmed"]` `:1365`.
    pub confirmed: bool,
    /// `state["last_latest"]`.
    pub last_latest: Vec<String>,
    /// `state["last_activity"]` (bat dau = luc bat dau poll `:1490`).
    pub last_activity: Instant,
    /// `state["question_since"]` `:1371`.
    pub question_since: Option<Instant>,
    /// `state["confirm_keys"]` `:1357` — khoa cac tin da tra loi.
    pub confirm_keys: HashSet<String>,
    /// `state["link_seen_at"]` `:1407`.
    pub link_first_seen: Option<Instant>,
    /// `state["link_wait_logged"]` `:1409`.
    pub link_wait_logged: bool,
    /// `state["ignore_ids"]` `:1327-1332` — tinh san tu `ignore_*`.
    pub ignore_ids: HashSet<String>,
    /// `state["ignore_keys"]` `:1333`.
    pub ignore_keys: HashSet<String>,
}

impl PollState {
    /// `:1489-1492` + tinh san `ignore_ids`/`ignore_keys` (`:1327-1333`, Python tinh o nhip dau).
    pub(crate) fn new(start: Instant, opts: &PollOpts) -> Self {
        let all = opts.ignore_videos.iter().chain(opts.ignore_links.iter());
        // :1329-1331 `for u in ...: ignore_ids |= video_ids(u)`; :1333 `{link_key(u) for u in ...}`
        let ignore_ids = all
            .clone()
            .flat_map(|u| video_ids(u))
            .collect::<HashSet<String>>();
        let ignore_keys = all.map(|u| link_key(u)).collect::<HashSet<String>>();
        Self {
            confirmed: false,
            last_latest: Vec::new(),
            last_activity: start,
            question_since: None,
            confirm_keys: HashSet::new(),
            link_first_seen: None,
            link_wait_logged: false,
            ignore_ids,
            ignore_keys,
        }
    }

    /// `_old(u)` `:1335-1339`: chuoi co trong `ignore_*`, hoac `link_key` trung, hoac co ma video
    /// trung.
    fn is_old(&self, url: &str, opts: &PollOpts) -> bool {
        if opts.ignore_videos.contains(url)
            || opts.ignore_links.contains(url)
            || self.ignore_keys.contains(&link_key(url))
        {
            return true;
        }
        video_ids(url).iter().any(|id| self.ignore_ids.contains(id))
    }
}

/// Quyet dinh cua [`handle_poll`] — moi viec can mang (raw, tai) do vong async lam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PollDecision {
    /// `return None` — poll tiep.
    Continue,
    /// Buoc 7: gui `text` vao chat (stage `confirm` `#nth`).
    ConfirmReply { text: String, nth: u32 },
    /// Buoc 10/11: thu [`resolve_raw_video`] cho tung `models` (theo thu tu) → tai [`NOTE_RAW`];
    /// khong co → `fallback`.
    NeedRaw {
        models: Vec<String>,
        fallback: RawFallback,
    },
}

/// Duong lui khi khong lay duoc ban raw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawFallback {
    /// `:1392-1393` `extract_unwatermarked_url(video_model, video_url)` → [`NOTE_UNWATERMARKED`].
    Unwatermarked {
        video_model: String,
        video_url: String,
    },
    /// `:1428` tai `links[0]` → [`NOTE_LINK`].
    Link(String),
    /// `:1408-1412` trong `LINK_RAW_GRACE` — poll tiep.
    Wait,
}

/// `modes.py:361-363` `should_extend` — da qua `timeout` nhung con duoi tran `max_timeout` va agent
/// con noi (`silence < window`) → gia han (IDEA-010).
#[must_use]
pub fn should_extend(
    elapsed: Duration,
    timeout: Duration,
    max_timeout: Duration,
    silence: Duration,
    window: Duration,
) -> bool {
    elapsed >= timeout && elapsed < max_timeout && silence < window
}

/// `session_state.py:77-82` `is_auth_status` — 401/403 = mat phien giua chung.
#[must_use]
pub fn is_auth_status(status: u16) -> bool {
    matches!(status, 401 | 403)
}

/// `:1503-1506`: chuoi loi (chu thuong) cho biet trinh duyet/tab da dong (Python `"has been
/// closed"`/`"target closed"` + cac cach chromiumoxide dien dat — xem doc module).
#[must_use]
pub fn is_browser_closed_msg(msg: &str) -> bool {
    const CLOSED: [&str; 9] = [
        "has been closed",
        "target closed",
        "session closed",
        "connection closed",
        "connection reset",
        "receiver is gone",
        "oneshot canceled",
        "session with given id not found",
        "no target with given id",
    ];
    let m = msg.to_lowercase();
    CLOSED.iter().any(|k| m.contains(k))
}

/// `:1522` `" | ".join(str(t).replace("\n", " ") for t in last_latest)[-400:]` — theo KY TU.
#[must_use]
pub fn latest_tail(last_latest: &[String]) -> String {
    let joined = last_latest
        .iter()
        .map(|t| t.replace('\n', " "))
        .collect::<Vec<_>>()
        .join(" | ");
    let n = joined.chars().count();
    joined
        .chars()
        .skip(n.saturating_sub(LATEST_TAIL_CHARS))
        .collect()
}

/// Ruling #3: chuoi `Cookie` tu CDP lam nen; `overrides` (`Set-Cookie` cac poll truoc, moi nhat
/// cuoi) ghi de theo ten, ten chua co → them cuoi. Giu thu tu nen. KHONG log.
#[must_use]
pub fn merge_cookie_header(base: &str, overrides: &[(String, String)]) -> String {
    let mut pairs: Vec<(String, String)> = base
        .split(';')
        .filter_map(|kv| {
            let kv = kv.trim();
            let (k, v) = kv.split_once('=')?;
            (!k.is_empty()).then(|| (k.to_string(), v.to_string()))
        })
        .collect();
    for (name, value) in overrides {
        match pairs.iter_mut().find(|(k, _)| k == name) {
            Some(slot) => slot.1.clone_from(value),
            None => pairs.push((name.clone(), value.clone())),
        }
    }
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// `modes.py:343-347` `confirm_key` — khoa cua tin moi nhat (xem doc module: khong sha1).
#[must_use]
pub(crate) fn confirm_key(latest: &[String]) -> String {
    latest
        .iter()
        .map(|t| t.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `_raw_for_link` `:1459-1472` phan THUAN: `video_model` co ma video (`video_ids`) trung voi ma
/// trong `link`, duyet `reversed(video_models)` (moi nhat thuong o cuoi). Rong khi link khong co ma
/// hoac khong model nao khop (Python tra `None` ngay, khong goi mang).
#[must_use]
pub(crate) fn raw_models_for_link(link: &str, video_models: &[String]) -> Vec<String> {
    let ids = video_ids(link);
    if ids.is_empty() {
        return Vec::new();
    }
    video_models
        .iter()
        .rev()
        .filter(|vm| video_ids(vm).iter().any(|id| ids.contains(id)))
        .cloned()
        .collect()
}

/// Buoc 8 (`:1369-1375`): `question` = `agent_question(latest)` khi khong video/link. Co cau hoi →
/// ghi `question_since` lan dau, qua `grace` → `AGENT_QUESTION`; khong → xoa moc.
///
/// # Errors
/// [`DolaError::AgentQuestion`].
pub(crate) fn question_grace(
    state: &mut PollState,
    question: Option<&str>,
    now: Instant,
    grace: Duration,
) -> Result<(), DolaError> {
    let Some(q) = question else {
        state.question_since = None; // :1375 `state.pop("question_since", None)`
        return Ok(());
    };
    let since = *state.question_since.get_or_insert(now); // :1371 setdefault
    if now.duration_since(since) >= grace {
        // `#[error("AGENT_QUESTION: {0}")]` da them ma -> KHONG lap trong thong diep (fix vong 2, I-1)
        return Err(DolaError::AgentQuestion(format!(
            "Dola asks the user, no one answers -> fix the prompt: {}",
            head(q, MSG_HEAD_300)
        )));
    }
    Ok(())
}

/// Buoc 3 (`:1289-1299`): het luot/het diem/tu choi chi xet TIN MOI NHAT, bo metadata JSON.
fn classify_latest(cur_latest: &[String]) -> Result<(), DolaError> {
    for text in cur_latest {
        if looks_like_json(text) {
            continue;
        }
        match classify_dola_text(text) {
            Some(DolaTextClass::Limit) => {
                return Err(DolaError::AccountLimited(format!(
                    "Account daily limit reached: {}",
                    head(text, MSG_HEAD_120)
                )));
            }
            Some(DolaTextClass::Refusal) => {
                warn!(
                    head = %head(text, MSG_HEAD_120),
                    "Dola từ chối nội dung (không phải credit) -> failing now"
                );
                return Err(DolaError::PromptViolation(format!(
                    "{REFUSAL_NOTE}{}",
                    head(text, MSG_HEAD_200)
                )));
            }
            Some(DolaTextClass::Credit) => {
                return Err(DolaError::Credit(format!(
                    "Insufficient quota: {}",
                    head(text, MSG_HEAD_120)
                )));
            }
            None => {}
        }
    }
    Ok(())
}

/// Buoc 4 (`:1309-1318`): cung phan loai tren 3000 ky tu cuoi cua `<main>`, hau to `(DOM)`.
fn classify_dom(dom: &str) -> Result<(), DolaError> {
    match classify_dola_text(dom) {
        Some(DolaTextClass::Limit) => Err(DolaError::AccountLimited(format!(
            "Account daily limit reached (DOM): {}",
            head(dom, MSG_HEAD_120)
        ))),
        Some(DolaTextClass::Refusal) => {
            // :1312-1314 — chi khop policy (khong GENERIC_REFUSAL_RE) -> Python nuot loi, di tiep
            let Some(snippet) = refusal_snippet(dom) else {
                debug!("DOM khop refusal chi qua policy_rejection -> bo qua (nhu Python)");
                return Ok(());
            };
            Err(DolaError::PromptViolation(format!(
                "{REFUSAL_NOTE}{snippet}"
            )))
        }
        Some(DolaTextClass::Credit) => Err(DolaError::Credit(format!(
            "Insufficient quota (DOM): {}",
            head(dom, MSG_HEAD_120)
        ))),
        None => Ok(()),
    }
}

/// Buoc 6 (`:1344-1353`): tu choi vi chinh sach -> fail ngay (chi tin moi nhat), goi y chan dung.
fn policy_step(cur_latest: &[String]) -> Result<(), DolaError> {
    let Some(reason) = policy_rejection(cur_latest) else {
        return Ok(());
    };
    warn!(
        head = %head(&reason, MSG_HEAD_120),
        "Dola rejected the prompt (policy) -> failing now"
    );
    let reason = if is_portrait_hint(&reason) {
        format!("{PORTRAIT_NOTE}{reason}")
    } else {
        reason
    };
    Err(DolaError::PromptViolation(reason))
}

/// Buoc 7 (`:1354-1368`): agent xin xac nhan / lua chon -> tu tra loi, moi tin KHAC mot lan, toi da
/// `confirm_max_replies`. `None` khi khong can tra loi (khong khop, da tra loi, hoac da toi max).
fn confirm_step(
    cur_latest: &[String],
    state: &mut PollState,
    opts: &PollOpts,
    limits: &Limits,
    now: Instant,
) -> Option<PollDecision> {
    needs_confirmation(cur_latest)?;
    let key = confirm_key(cur_latest);
    let replied = state.confirm_keys.len();
    let max = limits.confirm_max_replies as usize;
    if state.confirm_keys.contains(&key) {
        return None;
    }
    if replied >= max {
        info!(
            max,
            "Agent asks for confirmation/parameters again but CONFIRM_MAX_REPLIES reached"
        );
        return None;
    }
    // :1360 `get_confirmation_reply(cur_latest, duration=current_duration.get())`
    let reply = confirmation_reply(cur_latest, Some(opts.duration));
    let nth = u32::try_from(replied + 1).unwrap_or(u32::MAX);
    info!(
        nth,
        reply_chars = reply.chars().count(),
        "Agent asks for confirmation/parameters -> auto replying"
    );
    state.confirm_keys.insert(key);
    state.confirmed = true;
    state.last_activity = now;
    Some(PollDecision::ConfirmReply { text: reply, nth })
}

/// Buoc 9 (`:1376-1384`): agent tra loi XONG bang chu (khong video/link, khong dau hieu dang tao) va
/// im >= `agent_idle_fail_s` -> `AGENT_NO_VIDEO` kem cau agent noi (0 = tat).
fn idle_fail_step(
    cur_latest: &[String],
    state: &PollState,
    limits: &Limits,
    now: Instant,
) -> Result<(), DolaError> {
    let idle_fail = u64::from(limits.agent_idle_fail_s);
    if idle_fail == 0 || cur_latest.is_empty() {
        return Ok(());
    }
    let latest_text = cur_latest.join("\n");
    let latest_text = latest_text.trim();
    // tin moi nhat la chinh lenh cua tool -> agent chua tra loi
    let own_prompt = latest_text.contains("DIRECTIVE:") && latest_text.contains("Content:");
    let idle = now.duration_since(state.last_activity);
    if !latest_text.is_empty()
        && !own_prompt
        && idle.as_secs() >= idle_fail
        && !is_generating(latest_text)
    {
        // `#[error("AGENT_NO_VIDEO: {0}")]` da them ma -> KHONG lap (fix vong 2, I-1)
        return Err(DolaError::AgentNoVideo(format!(
            "agent tra loi bang chu, khong tao video ({}s im lang): {}",
            idle.as_secs(),
            head(latest_text, MSG_HEAD_400)
        )));
    }
    Ok(())
}

/// Buoc 11 (`:1394-1428`): chi co LINK trong cau tra loi (ban co watermark) -> thu raw dung video do
/// (`models`), doi `link_raw_grace_s` cho khoi video toi, het han moi tai link (`LINK_PREVIEW_ASK` =
/// false: khong port `:1414-1425`).
fn link_step(
    link: &str,
    video_models: &[String],
    state: &mut PollState,
    limits: &Limits,
    now: Instant,
) -> PollDecision {
    let models = raw_models_for_link(link, video_models);
    let grace = Duration::from_secs(u64::from(limits.link_raw_grace_s));
    let first_seen = *state.link_first_seen.get_or_insert(now); // :1407 setdefault
    if now.duration_since(first_seen) < grace {
        if !state.link_wait_logged {
            state.link_wait_logged = true;
            info!(
                grace_s = limits.link_raw_grace_s,
                link = %url_host_path(link),
                "thấy link video trong câu trả lời nhưng chưa có khối video (bản raw) → đợi thêm"
            );
        }
        state.last_activity = now;
        return PollDecision::NeedRaw {
            models,
            fallback: RawFallback::Wait,
        };
    }
    // Log "tải link" o `act` (nhanh `RawFallback::Link`) SAU khi thu raw that bai — nhu Python :1427
    // (fix vong 2, Minor #4).
    PollDecision::NeedRaw {
        models,
        fallback: RawFallback::Link(link.to_string()),
    }
}

/// `_handle_poll` `:1275-1429` — THUAN. `dom_text` = ket qua [`DOM_TAIL_JS`] do vong doc (`None`
/// khi loi/timeout — Python `except Exception: pass`). Thu tu xem bang trong doc module.
///
/// # Errors
/// `AccountLimited` / `PromptViolation` / `Credit` (buoc 3, 4, 6), `AgentQuestion` (8),
/// `AgentNoVideo` (9) — thong diep nhu Python.
pub(crate) fn handle_poll(
    poll: &ChainPoll,
    state: &mut PollState,
    opts: &PollOpts,
    limits: &Limits,
    now: Instant,
    dom_text: Option<&str>,
) -> Result<PollDecision, DolaError> {
    // 1. :1280-1283 activity
    let cur_latest = &poll.latest;
    if !cur_latest.is_empty() && *cur_latest != state.last_latest {
        state.last_activity = now;
    }
    if !cur_latest.is_empty() {
        state.last_latest.clone_from(cur_latest);
    }
    // 2. :1285-1288 balance — BO (R7)
    // 3. :1289-1299
    classify_latest(cur_latest)?;
    // 4. :1300-1320 quet DOM <main> (bo khi chat dung lai :1301-1302; loi doc DOM -> None -> bo)
    if let Some(dom) = dom_text.filter(|_| !opts.reuses_chat()) {
        classify_dom(dom)?;
    }
    // 5. :1321-1343 loc video/link cu — tin MOI NHAT dung truoc -> [0] la moi nhat
    let videos: Vec<&String> = poll
        .videos
        .iter()
        .filter(|v| !state.is_old(v, opts))
        .collect();
    let links: Vec<&String> = poll
        .links
        .iter()
        .filter(|l| !state.is_old(l, opts))
        .collect();
    let no_media = videos.is_empty() && links.is_empty();
    if no_media {
        // 6. :1344-1353
        policy_step(cur_latest)?;
        // 7. :1354-1368 — MOI mode (Python khong gate theo mode; fix vong 1)
        if let Some(decision) = confirm_step(cur_latest, state, opts, limits, now) {
            return Ok(decision);
        }
    }
    // 8. :1369-1375 agent hoi nguoi dung -> sau grace khong ai tra loi -> fail
    let question = if no_media {
        agent_question(cur_latest)
    } else {
        None
    };
    question_grace(
        state,
        question.as_deref(),
        now,
        Duration::from_secs(u64::from(limits.agent_question_grace_s)),
    )?;
    // 9. :1376-1384
    if no_media {
        idle_fail_step(cur_latest, state, limits, now)?;
    }
    // 10. :1385-1393 khoi video 2074 -> raw qua fallback_api, khong duoc thi main_url/download_url.
    // Python lay `video_models[0]` (danh sach CHUA loc) — giu nguyen.
    if let Some(video_url) = videos.first() {
        let video_model = poll.video_models.first().cloned().unwrap_or_default();
        return Ok(PollDecision::NeedRaw {
            models: vec![video_model.clone()],
            fallback: RawFallback::Unwatermarked {
                video_model,
                video_url: (*video_url).clone(),
            },
        });
    }
    // 11. :1394-1428
    if let Some(link) = links.first() {
        return Ok(link_step(link, &poll.video_models, state, limits, now));
    }
    // 12. :1429
    Ok(PollDecision::Continue)
}

/// `unwatermark.resolve_raw_video` `:243-251`: ≤ 3 `fallback_api` trong `video_model`, cai dau tien
/// [`DolaHttp::fetch_raw_video`] tra `Some` thang. `Err` (mang/timeout) → `warn` + thu cai ke
/// (ruling #4). Log chi `host/path`.
pub(crate) async fn resolve_raw_video(http: &DolaHttp, video_model: &str) -> Option<String> {
    for api in find_fallback_apis(video_model)
        .iter()
        .take(MAX_FALLBACK_APIS)
    {
        match http.fetch_raw_video(api, FETCH_TIMEOUT).await {
            Ok(Some(url)) => return Some(url),
            Ok(None) => {}
            Err(e) => warn!(
                api = %url_host_path(api),
                error = %e,
                "fetch_raw_video loi mang -> coi nhu None, thu fallback_api ke"
            ),
        }
    }
    None
}

/// `_download_result` `:1265-1272`: stage `downloading`, [`output_path`] (UTC), tai, tra artifact.
///
/// # Errors
/// [`DolaError::Http`] tu [`DolaHttp::download`].
async fn download_result(
    ctx: &LoopCtx<'_>,
    url: &str,
    note: &'static str,
) -> Result<VideoArtifact, DolaError> {
    let account = ctx.account;
    info!(account, note, url = %url_host_path(url), "Completed! Downloading...");
    ctx.hooks.on_stage(STAGE_DOWNLOADING);
    let local_path = output_path(ctx.out_dir, account, SystemTime::now());
    let bytes = ctx.http.download(url, &local_path).await?;
    // :1271 `{size / 1e6:.1f} MB` — so nguyen, khong ep f64 (tranh cast_precision_loss).
    info!(
        account,
        out = %local_path.display(),
        mb = format_args!("{}.{}", bytes / 1_000_000, bytes % 1_000_000 / 100_000),
        "Downloaded"
    );
    Ok(VideoArtifact {
        video_url: url.to_string(),
        local_path,
        conversation_id: ctx.conv.clone(),
        account: account.to_string(),
        watermarked: is_watermarked_url(url),
        bytes,
        note,
    })
}

/// `poll_conversation` `:1475-1525` tren [`Page`] that ([`POLL_URL`]).
///
/// # Errors
/// Xem [`poll_and_download_at`].
// PF-3: chu ky 8 tham so do brief Task 11 quy dinh (Task 14 `BrowserDriver::poll_and_download` goi
// cung dang); gom lai thanh struct la doi giao dien plan — de controller quyet.
#[allow(clippy::too_many_arguments)]
pub async fn poll_and_download(
    page: &Page,
    http: &DolaHttp,
    account: &str,
    conv: &ConversationId,
    out_dir: &Path,
    opts: &PollOpts,
    limits: &Limits,
    hooks: &dyn PollHooks,
) -> Result<VideoArtifact, DolaError> {
    poll_and_download_at(
        page, http, POLL_URL, account, conv, out_dir, opts, limits, hooks,
    )
    .await
}

/// Nhu [`poll_and_download`] voi bat ky [`PollPage`] (test/driver gia).
///
/// # Errors
/// Xem [`poll_and_download_at`].
// PF-3: nhu `poll_and_download`.
#[allow(clippy::too_many_arguments)]
pub async fn poll_and_download_with<P: PollPage + ?Sized>(
    page: &P,
    http: &DolaHttp,
    account: &str,
    conv: &ConversationId,
    out_dir: &Path,
    opts: &PollOpts,
    limits: &Limits,
    hooks: &dyn PollHooks,
) -> Result<VideoArtifact, DolaError> {
    poll_and_download_at(
        page, http, POLL_URL, account, conv, out_dir, opts, limits, hooks,
    )
    .await
}

/// Ngu canh co dinh cua mot vong poll (gom tham so de cac ham phu khong qua 7 doi so).
struct LoopCtx<'a> {
    http: &'a DolaHttp,
    account: &'a str,
    conv: &'a ConversationId,
    out_dir: &'a Path,
    hooks: &'a dyn PollHooks,
}

/// Vong poll tren endpoint `poll_url` tuy y (test wiremock). Xem doc module.
///
/// # Errors
/// - [`DolaError::RiskControl`] captcha (moi nhip);
/// - [`DolaError::BrowserClosed`] loi CDP "closed" khi doc cookie/captcha;
/// - [`DolaError::LoggedOut`] poll HTTP 401/403 (`revoked: false`);
/// - loi cua [`handle_poll`] (limit/credit/refusal/policy/question/idle);
/// - [`DolaError::Browser`] khi gui tra loi xac nhan that bai (Python: ngoai le lan ra);
/// - [`DolaError::Http`] tai file that bai;
/// - [`DolaError::Timeout`] het gio (`:1522-1524`).
// PF-3: nhu `poll_and_download` + `poll_url` (9 tham so).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn poll_and_download_at<P: PollPage + ?Sized>(
    page: &P,
    http: &DolaHttp,
    poll_url: &str,
    account: &str,
    conv: &ConversationId,
    out_dir: &Path,
    opts: &PollOpts,
    limits: &Limits,
    hooks: &dyn PollHooks,
) -> Result<VideoArtifact, DolaError> {
    let ctx = LoopCtx {
        http,
        account,
        conv,
        out_dir,
        hooks,
    };
    let start = Instant::now();
    let mut state = PollState::new(start, opts);
    let mut last_callback: Option<Instant> = None; // :1488 `last_callback = 0.0` -> lan dau goi ngay
    let mut cookie_overrides: Vec<(String, String)> = Vec::new();
    let interval = Duration::from_secs(u64::from(limits.poll_interval_s));
    let max_timeout = Duration::from_secs(u64::from(limits.pro_max_timeout_s));
    let window = Duration::from_secs(u64::from(limits.pro_activity_window_s));
    hooks.on_stage(STAGE_GENERATING); // :1493-1494
    info!(account, conv = %conv, timeout_s = opts.timeout.as_secs(), "bat dau poll");
    loop {
        // :1496-1500 het gio (tru khi gia han)
        let now = Instant::now();
        let elapsed = now.duration_since(start);
        let silence = now.duration_since(state.last_activity);
        if elapsed >= opts.timeout
            && !should_extend(elapsed, opts.timeout, max_timeout, silence, window)
        {
            break;
        }
        tokio::time::sleep(interval).await; // :1499
        // Ruling #2 captcha + ruling #3 cookie (loi CDP: closed -> Err, khac -> continue)
        let Some(cookie) = read_cookie(page, account, &cookie_overrides).await? else {
            continue;
        };
        // :1500-1509 poll; loi HTTP -> "Polling exception" + continue
        let outcome = match http.pull_chain_at(&cookie, conv, poll_url).await {
            Ok(o) => o,
            Err(e) => {
                warn!(account, error = %e, "Polling exception");
                continue;
            }
        };
        upsert_cookies(&mut cookie_overrides, outcome.set_cookies);
        let poll = outcome.poll;
        // :1510-1512 mat phien giua chung
        if !poll.ok && is_auth_status(poll.status) {
            return Err(DolaError::LoggedOut {
                account: account.to_string(),
                why: format!("conversation API HTTP {} while polling", poll.status),
                revoked: false,
            });
        }
        // :1513-1517 on_poll <= 1 lan / 30 s
        let now = Instant::now();
        if last_callback.is_none_or(|t| now.duration_since(t) >= ON_POLL_EVERY) {
            hooks.on_poll(now.duration_since(start));
            last_callback = Some(now);
        }
        // brief: latest doi -> on_agent_reply (cung dieu kien buoc 1)
        if !poll.latest.is_empty() && poll.latest != state.last_latest {
            hooks.on_agent_reply(&poll.latest.join("\n"));
        }
        // Buoc 4: doc DOM truoc khi quyet dinh (bo khi chat dung lai)
        let dom_text = if opts.reuses_chat() {
            None
        } else {
            read_dom_tail(page).await
        };
        // `now` lay lai SAU khi doc DOM (<= 5 s) — Python `time.time()` tai moi cho dung (fix vong 2, Minor #2)
        let now = Instant::now();
        let decision = handle_poll(&poll, &mut state, opts, limits, now, dom_text.as_deref())?;
        if let Some(artifact) = act(&ctx, page, decision).await? {
            return Ok(artifact);
        }
        debug!(
            account,
            elapsed_s = start.elapsed().as_secs(),
            "...Generating"
        );
    }
    Err(timeout_error(start, conv, &state.last_latest))
}

/// Ruling #2 (Task 9 I-1): captcha moi nhip TRUOC `pull_chain`; ruling #3: cookie moi tu CDP + cac
/// `Set-Cookie` truoc do. `Ok(None)` = loi CDP khong phai "closed" -> vong `continue`
/// (`Polling exception` `:1508-1509`).
///
/// # Errors
/// [`DolaError::RiskControl`] (captcha), [`DolaError::BrowserClosed`].
async fn read_cookie<P: PollPage + ?Sized>(
    page: &P,
    account: &str,
    overrides: &[(String, String)],
) -> Result<Option<String>, DolaError> {
    match page.has_captcha().await {
        Ok(true) => {
            warn!(
                account,
                "Captcha detected (bdcaptcha.html) trong luc poll → RISK_CONTROL"
            );
            return Err(captcha_error());
        }
        Ok(false) => {}
        Err(e) => {
            closed_or_warn(account, "has_captcha", &e)?;
            return Ok(None);
        }
    }
    match page.cookie_header().await {
        Ok(base) => Ok(Some(merge_cookie_header(&base, overrides))),
        Err(e) => {
            closed_or_warn(account, "cookie_header", &e)?;
            Ok(None)
        }
    }
}

/// `Set-Cookie` moi -> ghi de theo ten trong `overrides` (moi nhat thang), ten moi -> them.
fn upsert_cookies(overrides: &mut Vec<(String, String)>, incoming: Vec<(String, String)>) {
    for (name, value) in incoming {
        match overrides.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => overrides.push((name, value)),
        }
    }
}

/// `:1305-1308` `wait_for(page.evaluate(DOM_TAIL_JS), timeout=5)` — loi/timeout -> `None`
/// (`except Exception: pass` `:1319-1320`).
async fn read_dom_tail<P: PollPage + ?Sized>(page: &P) -> Option<String> {
    match tokio::time::timeout(DOM_TAIL_TIMEOUT, page.dom_tail()).await {
        Ok(Ok(t)) => Some(t),
        Ok(Err(e)) => {
            debug!(error = %e, "khong doc duoc DOM <main> -> bo qua");
            None
        }
        Err(_) => {
            debug!("doc DOM <main> qua 5 s -> bo qua");
            None
        }
    }
}

/// Thuc hien [`PollDecision`] (phan CO mang): gui tra loi, lay raw (≤ 3 `fallback_api` moi model),
/// tai file. `Ok(None)` = poll tiep.
///
/// # Errors
/// [`DolaError::Browser`] (gui tra loi), [`DolaError::Http`] (tai file).
async fn act<P: PollPage + ?Sized>(
    ctx: &LoopCtx<'_>,
    page: &P,
    decision: PollDecision,
) -> Result<Option<VideoArtifact>, DolaError> {
    let (models, fallback) = match decision {
        PollDecision::Continue => return Ok(None),
        PollDecision::ConfirmReply { text, nth } => {
            ctx.hooks.on_stage(STAGE_CONFIRM); // :1362 set_stage("confirm", f"#{n}")
            info!(
                account = ctx.account,
                nth,
                chars = text.chars().count(),
                "gui tra loi xac nhan"
            );
            page.send_reply(&text).await?; // :1363 — loi lan ra nhu Python
            return Ok(None);
        }
        PollDecision::NeedRaw { models, fallback } => (models, fallback),
    };
    debug!(account = ctx.account, models = models.len(), fallback = %fallback, "can ban raw");
    for model in &models {
        if let Some(raw) = resolve_raw_video(ctx.http, model).await {
            info!(account = ctx.account, raw = %url_host_path(&raw), "raw stream without logo");
            return download_result(ctx, &raw, NOTE_RAW).await.map(Some);
        }
    }
    match fallback {
        RawFallback::Unwatermarked {
            video_model,
            video_url,
        } => {
            let url = extract_unwatermarked_url(&video_model, &video_url);
            download_result(ctx, &url, NOTE_UNWATERMARKED)
                .await
                .map(Some)
        }
        RawFallback::Link(url) => {
            // :1427 — chi log khi `_raw_for_link` da tra None (sau vong `models` o tren)
            info!(
                account = ctx.account,
                link = %url_host_path(&url),
                "chỉ có link trong câu trả lời, không khớp được bản raw sau grace → tải link (có thể còn watermark)"
            );
            download_result(ctx, &url, NOTE_LINK).await.map(Some)
        }
        RawFallback::Wait => Ok(None),
    }
}

/// `:1522-1524` `TimeoutError(f"No video generated within {int(elapsed)}s (conversation_id=…)" +
/// (f" | agent: {tail}" if tail else ""))`.
fn timeout_error(start: Instant, conv: &ConversationId, last_latest: &[String]) -> DolaError {
    let elapsed_s = start.elapsed().as_secs();
    let tail = latest_tail(last_latest);
    let mut msg = format!("No video generated within {elapsed_s}s (conversation_id={conv})");
    if !tail.is_empty() {
        msg.push_str(" | agent: ");
        msg.push_str(&tail);
    }
    DolaError::Timeout(msg)
}

/// Loi CDP khi doc trang moi nhip: "closed" → [`DolaError::BrowserClosed`] (`:1503-1506`); khac →
/// `warn` + `Ok` de vong `continue` (`:1508-1509`).
fn closed_or_warn(account: &str, what: &str, e: &BrowserError) -> Result<(), DolaError> {
    if is_browser_closed_msg(&e.to_string()) {
        warn!(account, what, "Browser or page was closed while polling");
        return Err(DolaError::BrowserClosed(format!(
            "browser or tab was closed while generating ({account})"
        )));
    }
    warn!(account, what, error = %e, "Polling exception");
    Ok(())
}

/// `text[:n]` Python — cat theo KY TU.
fn head(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

impl fmt::Display for RawFallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unwatermarked { video_url, .. } => {
                write!(f, "unwatermarked {}", url_host_path(video_url))
            }
            Self::Link(u) => write!(f, "link {}", url_host_path(u)),
            Self::Wait => f.write_str("wait"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;
    use chromiumoxide::error::CdpError;
    use serde_json::{Value, json};
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    use super::*;

    const ACC: &str = "acc1";
    const CONV: &str = "123456789";
    const COOKIE_BASE: &str = "sessionid=s1; msToken=old";
    const CONFIRM_REPLY: &str = "Yes, please proceed. Có, hãy tiếp tục tạo video. Đúng rồi, hãy tạo video theo các thông số này. はい、確認しました。生成開始してください。";
    const VID_A: &str = "v186a3aaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VID_B: &str = "v186a3bbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn conv() -> ConversationId {
        ConversationId(CONV.to_string())
    }

    fn opts(mode: Mode) -> PollOpts {
        PollOpts {
            timeout: Duration::from_secs(300),
            mode,
            duration: 10,
            ignore_videos: HashSet::new(),
            ignore_links: HashSet::new(),
            accept_links: mode.is_pro_like(),
        }
    }

    fn poll_with(latest: &[&str]) -> ChainPoll {
        ChainPoll {
            ok: true,
            status: 200,
            latest: latest.iter().map(ToString::to_string).collect(),
            ..ChainPoll::default()
        }
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(ToString::to_string).collect()
    }

    fn t0() -> Instant {
        Instant::now()
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// `handle_poll` voi `dom_text = None`.
    fn hp(
        poll: &ChainPoll,
        state: &mut PollState,
        opts: &PollOpts,
        limits: &Limits,
        now: Instant,
    ) -> Result<PollDecision, DolaError> {
        handle_poll(poll, state, opts, limits, now, None)
    }

    // ===== Buoc 1: activity (:1280-1283) =====

    #[test]
    fn hp_activity_cap_nhat_last_activity_khi_latest_doi() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        assert_eq!(st.last_activity, start);
        assert!(st.last_latest.is_empty());
        // latest moi -> last_activity = now, last_latest = latest
        let now1 = start + secs(5);
        let p = poll_with(&["猫の動画を作ります"]);
        assert_eq!(
            hp(&p, &mut st, &o, &l, now1).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(st.last_activity, now1);
        assert_eq!(st.last_latest, s(&["猫の動画を作ります"]));
        // cung latest -> khong doi
        let now2 = start + secs(10);
        assert_eq!(
            hp(&p, &mut st, &o, &l, now2).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(st.last_activity, now1);
        // latest rong -> giu last_latest, khong doi activity (:1283 `cur_latest or state["last_latest"]`)
        let now3 = start + secs(15);
        let empty = poll_with(&[]);
        assert_eq!(
            hp(&empty, &mut st, &o, &l, now3).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(st.last_activity, now1);
        assert_eq!(st.last_latest, s(&["猫の動画を作ります"]));
    }

    // ===== Buoc 3: limit/refusal/credit tren latest (:1289-1299) =====

    #[test]
    fn hp_latest_limit_bao_account_limited() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        let long = format!("動画生成の1日あたりの上限に達しました{}", "。".repeat(200));
        let p = poll_with(&[&long]);
        let err = hp(&p, &mut st, &o, &l, start).unwrap_err();
        let want = format!("Account daily limit reached: {}", head(&long, 120));
        assert!(
            matches!(&err, DolaError::AccountLimited(m) if *m == want),
            "{err}"
        );
    }

    #[test]
    fn hp_latest_refusal_bao_prompt_violation_voi_refusal_note() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        let text = "I am temporarily unable to generate the content you requested. Please try entering other requirements.";
        let p = poll_with(&[text]);
        let err = hp(&p, &mut st, &o, &l, start).unwrap_err();
        let want = format!("{REFUSAL_NOTE}{}", head(text, 200));
        assert!(
            matches!(&err, DolaError::PromptViolation(m) if *m == want),
            "{err}"
        );
        assert!(err.is_dead_chat());
    }

    #[test]
    fn hp_latest_credit_bao_credit() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        // texts (tin cu) KHONG duoc xet (:1289 chi cur_latest) — tin cu het credit khong bao.
        let mut p = poll_with(&["残高不足のため生成できません"]);
        p.texts = s(&["動画生成の1日あたりの上限に達しました"]);
        let err = hp(&p, &mut st, &o, &l, start).unwrap_err();
        assert!(
            matches!(&err, DolaError::Credit(m) if m == "Insufficient quota: 残高不足のため生成できません"),
            "{err}"
        );
        assert!(err.rotates_account());
    }

    /// `looks_like_json` bi bo qua o buoc 3 (:1290-1291) nhung buoc 6 `policy_rejection` KHONG bo
    /// (:1348 `list(cur_latest)`): JSON chua "content policy" -> `PromptViolation` KHONG co `REFUSAL_NOTE`.
    #[test]
    fn hp_latest_json_bo_qua_buoc_3_nhung_buoc_6_van_bat() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        // JSON khop "daily limit" -> bo qua buoc 3; buoc 6 khong phai policy -> Continue.
        let p = poll_with(&[r#"{"limit_type":"PerDay","msg":"daily limit reached"}"#]);
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::Continue
        );
        // JSON khop policy -> buoc 6 bao, ly do = chinh chuoi (trim, <= 300 ky tu), khong REFUSAL_NOTE.
        let text = r#"{"note": "content policy violation"}"#;
        let p = poll_with(&[text]);
        let err = hp(&p, &mut st, &o, &l, start).unwrap_err();
        assert!(
            matches!(&err, DolaError::PromptViolation(m) if m == text),
            "{err}"
        );
    }

    // ===== Buoc 4: quet DOM (:1300-1320) =====

    #[test]
    fn hp_dom_scan_limit_refusal_credit_va_bo_qua_khi_reuse() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        let p = poll_with(&["少々お待ちください"]);
        // limit (DOM)
        let dom = format!("{}動画生成の1日あたりの上限に達しました", "x".repeat(100));
        let err = handle_poll(&p, &mut st, &o, &l, start, Some(&dom)).unwrap_err();
        let want = format!("Account daily limit reached (DOM): {}", head(&dom, 120));
        assert!(
            matches!(&err, DolaError::AccountLimited(m) if *m == want),
            "{err}"
        );
        // refusal (DOM): REFUSAL_NOTE + doan quanh cho khop GENERIC_REFUSAL_RE (40 truoc / 160 sau
        // m.end()). Oracle Python: khop "temporarily unable to generate" (60..90) -> snippet 230 ky
        // tu = 40 'a' + khop + " the video" (10) + 150 'b'.
        let dom = format!(
            "{}temporarily unable to generate the video{}",
            "a".repeat(60),
            "b".repeat(200)
        );
        let err = handle_poll(&p, &mut st, &o, &l, start, Some(&dom)).unwrap_err();
        let want = format!(
            "{REFUSAL_NOTE}{}temporarily unable to generate the video{}",
            "a".repeat(40),
            "b".repeat(150)
        );
        assert!(
            matches!(&err, DolaError::PromptViolation(m) if *m == want),
            "{err}"
        );
        // refusal (DOM) CHI khop policy_rejection (khong GENERIC_REFUSAL_RE): Python nuot
        // AttributeError -> poll tiep.
        let dom = "ポリシーに違反する可能性があります";
        assert_eq!(
            classify_dola_text(dom),
            Some(DolaTextClass::Refusal),
            "tien de: DOM la refusal"
        );
        assert_eq!(
            handle_poll(&p, &mut st, &o, &l, start, Some(dom)).unwrap(),
            PollDecision::Continue
        );
        // credit (DOM)
        let dom = "sidebar\n残高不足のため生成できません";
        let err = handle_poll(&p, &mut st, &o, &l, start, Some(dom)).unwrap_err();
        assert!(
            matches!(&err, DolaError::Credit(m) if m == "Insufficient quota (DOM): sidebar\n残高不足のため生成できません"),
            "{err}"
        );
        // dom_text None (loi/timeout) -> bo qua
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::Continue
        );
        // chat dung lai (co ignore_*) -> KHONG quet DOM (:1301-1302)
        let mut o2 = opts(Mode::Fast);
        o2.ignore_links.insert("http://x/video/old.mp4".to_string());
        let mut st2 = PollState::new(start, &o2);
        assert_eq!(
            handle_poll(&p, &mut st2, &o2, &l, start, Some(dom)).unwrap(),
            PollDecision::Continue
        );
    }

    // ===== Buoc 5: loc ignore (:1321-1343) =====

    #[test]
    fn hp_loc_ignore_theo_chuoi_link_key_va_ma_video() {
        let start = t0();
        let mut o = opts(Mode::Fast);
        // link cu ky lai (signature/expire doi, cung /video/... path) -> link_key khop
        o.ignore_links.insert(
            "http://v16-dola.dola.com/sig1/exp1/video/tos/mya/tos-mya-v-50851/KEY1/?x-signature=a"
                .to_string(),
        );
        // video cu theo MA v186a3... (URL khac hoan toan)
        o.ignore_videos
            .insert(format!("http://cdn.example/{VID_A}/play?x=1"));
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        assert_eq!(st.ignore_ids, HashSet::from([VID_A.to_string()]));
        assert_eq!(
            st.ignore_keys,
            HashSet::from([
                "v16-dola.dola.com/video/tos/mya/tos-mya-v-50851/KEY1".to_string(),
                format!("cdn.example/{VID_A}/play"),
            ])
        );
        let mut p = poll_with(&["動画です"]);
        p.links = vec![
            "http://v16-dola.dola.com/sig2/exp2/video/tos/mya/tos-mya-v-50851/KEY1/?x-signature=b"
                .to_string(),
        ];
        p.videos = vec![format!("http://other.example/{VID_A}/x.mp4")];
        p.video_models = vec!["{}".to_string()];
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::Continue,
            "ca video lan link deu cu"
        );
        // video moi (ma khac) o vi tri 1: videos loc con [new]; video_models[0] (KHONG loc, :1387
        // Python) di kem — pin hanh vi Python (R5: P2 khong dung lai chat nen khong gap).
        p.videos.push(format!("http://other.example/{VID_B}/y.mp4"));
        p.video_models.push(r#"{"id":"b"}"#.to_string());
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::NeedRaw {
                models: vec!["{}".to_string()],
                fallback: RawFallback::Unwatermarked {
                    video_model: "{}".to_string(),
                    video_url: format!("http://other.example/{VID_B}/y.mp4"),
                },
            }
        );
    }

    // ===== Buoc 6: policy_rejection (:1344-1353) =====

    #[test]
    fn hp_policy_rejection_portrait_them_portrait_note() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        // Oracle Python: classify_dola_text -> '' (co chu credit -> khong refusal, khong CREDIT_FAIL),
        // policy_rejection -> khop, PORTRAIT_RE -> True.
        let text =
            "肖像保護のため、あなた自身が写っている動画のみ対応します。credit は消費されません。";
        assert_eq!(classify_dola_text(text), None, "tien de");
        let p = poll_with(&[text]);
        let err = hp(&p, &mut st, &o, &l, start).unwrap_err();
        let want = format!("{PORTRAIT_NOTE}{text}");
        assert!(
            matches!(&err, DolaError::PromptViolation(m) if *m == want),
            "{err}"
        );
        // Co video -> KHONG xet policy (:1345 `if not videos and not links`)
        let mut p2 = poll_with(&[text]);
        p2.videos = vec!["http://h/video/a.mp4".to_string()];
        p2.video_models = vec![String::new()];
        assert!(matches!(
            hp(&p2, &mut st, &o, &l, start).unwrap(),
            PollDecision::NeedRaw { .. }
        ));
    }

    // ===== Buoc 7: xac nhan (:1354-1368) =====

    #[test]
    fn hp_xac_nhan_pro_tra_loi_toi_da_roi_bo_qua() {
        let start = t0();
        let o = opts(Mode::Pro);
        let l = Limits::default();
        assert_eq!(l.confirm_max_replies, 3);
        let mut st = PollState::new(start, &o);
        let now1 = start + secs(5);
        let p1 = poll_with(&["ご確認ください"]);
        assert_eq!(
            hp(&p1, &mut st, &o, &l, now1).unwrap(),
            PollDecision::ConfirmReply {
                text: CONFIRM_REPLY.to_string(),
                nth: 1
            }
        );
        assert!(st.confirmed);
        assert_eq!(st.last_activity, now1);
        assert_eq!(st.confirm_keys.len(), 1);
        // Cung tin -> da tra loi -> khong tra loi lai (:1358 `key not in keys`) -> roi xuong Continue
        let now2 = start + secs(10);
        assert_eq!(
            hp(&p1, &mut st, &o, &l, now2).unwrap(),
            PollDecision::Continue
        );
        // Tin khac -> #2, #3
        let p2 = poll_with(&["パラメータをご確認ください 2"]);
        assert!(matches!(
            hp(&p2, &mut st, &o, &l, now2).unwrap(),
            PollDecision::ConfirmReply { nth: 2, .. }
        ));
        let p3 = poll_with(&["パラメータをご確認ください 3"]);
        assert!(matches!(
            hp(&p3, &mut st, &o, &l, now2).unwrap(),
            PollDecision::ConfirmReply { nth: 3, .. }
        ));
        // Tin thu 4 -> CONFIRM_MAX_REPLIES=3 da dat -> khong tra loi (:1367-1368)
        let p4 = poll_with(&["パラメータをご確認ください 4"]);
        assert_eq!(
            hp(&p4, &mut st, &o, &l, now2).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(st.confirm_keys.len(), 3);
        // Mode Nhanh CUNG tra loi (fix vong 1: Python :1354 khong gate theo mode) — toi da 3 roi roi xuong
        let of = opts(Mode::Fast);
        let mut sf = PollState::new(start, &of);
        assert_eq!(
            hp(&p1, &mut sf, &of, &l, now1).unwrap(),
            PollDecision::ConfirmReply {
                text: CONFIRM_REPLY.to_string(),
                nth: 1
            }
        );
        assert!(sf.confirmed);
        assert_eq!(sf.last_activity, now1);
        assert!(matches!(
            hp(&p2, &mut sf, &of, &l, now2).unwrap(),
            PollDecision::ConfirmReply { nth: 2, .. }
        ));
        assert!(matches!(
            hp(&p3, &mut sf, &of, &l, now2).unwrap(),
            PollDecision::ConfirmReply { nth: 3, .. }
        ));
        assert_eq!(
            hp(&p4, &mut sf, &of, &l, now2).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(sf.confirm_keys.len(), 3);
        // Co video -> khong xet xac nhan (:1354)
        let mut pv = poll_with(&["ご確認ください"]);
        pv.videos = vec!["http://h/video/a.mp4".to_string()];
        pv.video_models = vec![String::new()];
        let mut sp = PollState::new(start, &o);
        assert!(matches!(
            hp(&pv, &mut sp, &o, &l, now1).unwrap(),
            PollDecision::NeedRaw { .. }
        ));
    }

    #[test]
    fn hp_xac_nhan_hoi_so_giay_tra_thoi_luong_task() {
        let start = t0();
        let mut o = opts(Mode::Agent);
        o.duration = 15;
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        let p = poll_with(&["秒数を指定してください"]);
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::ConfirmReply {
                text: "15秒 (15 seconds). 15秒の動画を生成してください。".to_string(),
                nth: 1
            }
        );
    }

    // ===== Buoc 8: agent_question grace (:1369-1375) =====

    /// `lvc_core::agent_question` khong bao gio `Some` (parity Python — doc o do) nen nhanh nay
    /// KHONG the kich qua `handle_poll`: cau hoi "?" khop `needs_confirmation` (Question) → buoc 7 tra
    /// loi 1 lan (fix vong 1: moi mode), roi cung tin do → buoc 8 `agent_question` = None → Continue
    /// sau grace, `question_since` khong duoc dat. Logic grace test rieng qua `question_grace`.
    #[test]
    fn hp_agent_question_grace() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        let p = poll_with(&["どの比率にしますか？"]);
        assert!(matches!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::ConfirmReply { nth: 1, .. }
        ));
        assert_eq!(st.question_since, None);
        assert_eq!(
            hp(&p, &mut st, &o, &l, start + secs(5)).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(st.question_since, None);
        assert_eq!(
            hp(&p, &mut st, &o, &l, start + secs(61)).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(st.question_since, None);
        // question_grace: lan dau ghi moc, truoc grace Ok, tu grace -> AGENT_QUESTION; None -> xoa moc.
        let grace = Duration::from_secs(u64::from(l.agent_question_grace_s));
        let qtext = "どの比率にしますか？";
        question_grace(&mut st, Some(qtext), start, grace).unwrap();
        assert_eq!(st.question_since, Some(start));
        question_grace(&mut st, Some(qtext), start + secs(59), grace).unwrap();
        let err = question_grace(&mut st, Some(qtext), start + secs(60), grace).unwrap_err();
        assert!(matches!(err, DolaError::AgentQuestion(_)), "{err}");
        // Fix vong 2 (I-1): `Display` = dung chuoi Python :1373 — ma loi MOT lan (`#[error]` da them).
        assert_eq!(
            err.to_string(),
            "AGENT_QUESTION: Dola asks the user, no one answers -> fix the prompt: どの比率にしますか？"
        );
        assert!(err.is_dead_chat());
        question_grace(&mut st, None, start + secs(61), grace).unwrap();
        assert_eq!(st.question_since, None);
        // question[:300]
        let long = "?".repeat(400);
        question_grace(&mut st, Some(long.as_str()), start, grace).unwrap();
        let err =
            question_grace(&mut st, Some(long.as_str()), start + secs(60), grace).unwrap_err();
        assert!(err.to_string().ends_with(&"?".repeat(300)), "{err}");
    }

    // ===== Buoc 9: idle fail (:1376-1384) =====

    #[test]
    fn hp_idle_fail_agent_no_video_voi_ngoai_le() {
        let start = t0();
        let o = opts(Mode::Agent);
        let l = Limits::default();
        assert_eq!(l.agent_idle_fail_s, 120);
        let mut st = PollState::new(start, &o);
        let p = poll_with(&["猫の動画はこちらです", "以上です"]);
        // t=0: activity -> Continue; t=119: Continue; t=120: AGENT_NO_VIDEO
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::Continue
        );
        assert_eq!(
            hp(&p, &mut st, &o, &l, start + secs(119)).unwrap(),
            PollDecision::Continue
        );
        let err = hp(&p, &mut st, &o, &l, start + secs(120)).unwrap_err();
        assert!(matches!(err, DolaError::AgentNoVideo(_)), "{err}");
        // Fix vong 2 (I-1): `Display` = dung chuoi Python :1384 — ma loi MOT lan.
        assert_eq!(
            err.to_string(),
            "AGENT_NO_VIDEO: agent tra loi bang chu, khong tao video (120s im lang): 猫の動画はこちらです\n以上です"
        );
        // Lenh cua tool (DIRECTIVE: + Content:) la tin moi nhat -> agent chua tra loi -> Continue
        let mut s2 = PollState::new(start, &o);
        let own = poll_with(&["DIRECTIVE: make video\nContent: a cat"]);
        hp(&own, &mut s2, &o, &l, start).unwrap();
        assert_eq!(
            hp(&own, &mut s2, &o, &l, start + secs(500)).unwrap(),
            PollDecision::Continue
        );
        // Dang tao (DOLA_TEXT["generating"]) -> Continue
        let mut s3 = PollState::new(start, &o);
        let generating = poll_with(&["動画を生成しています"]);
        hp(&generating, &mut s3, &o, &l, start).unwrap();
        assert_eq!(
            hp(&generating, &mut s3, &o, &l, start + secs(500)).unwrap(),
            PollDecision::Continue
        );
        // Tat (agent_idle_fail_s = 0) -> Continue
        let l0 = Limits {
            agent_idle_fail_s: 0,
            ..Limits::default()
        };
        let mut s4 = PollState::new(start, &o);
        hp(&p, &mut s4, &o, &l0, start).unwrap();
        assert_eq!(
            hp(&p, &mut s4, &o, &l0, start + secs(500)).unwrap(),
            PollDecision::Continue
        );
        // latest rong -> khong xet (:1379 `and cur_latest`)
        let mut s5 = PollState::new(start, &o);
        assert_eq!(
            hp(&poll_with(&[]), &mut s5, &o, &l, start + secs(500)).unwrap(),
            PollDecision::Continue
        );
        // latest_text[:400]
        let long = "あ".repeat(500);
        let mut s6 = PollState::new(start, &o);
        let pl = poll_with(&[&long]);
        hp(&pl, &mut s6, &o, &l, start).unwrap();
        let err = hp(&pl, &mut s6, &o, &l, start + secs(200)).unwrap_err();
        let m = err.to_string();
        assert!(m.contains("(200s im lang): "), "{m}");
        assert_eq!(m.chars().filter(|&c| c == 'あ').count(), 400);
    }

    // ===== Buoc 10: videos (:1385-1393) =====

    #[test]
    fn hp_videos_need_raw_voi_unwatermarked_fallback() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        let mut p = poll_with(&["動画を生成しました"]);
        p.videos = s(&["http://h/video/a.mp4", "http://h/video/b.mp4"]);
        p.video_models = s(&[r#"{"fallback_api":"http://api/a"}"#, r#"{"id":"b"}"#]);
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::NeedRaw {
                models: vec![r#"{"fallback_api":"http://api/a"}"#.to_string()],
                fallback: RawFallback::Unwatermarked {
                    video_model: r#"{"fallback_api":"http://api/a"}"#.to_string(),
                    video_url: "http://h/video/a.mp4".to_string(),
                },
            }
        );
        // video_models rong -> model "" (:1387 `video_models[0] if video_models else ""`)
        p.video_models.clear();
        assert_eq!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::NeedRaw {
                models: vec![String::new()],
                fallback: RawFallback::Unwatermarked {
                    video_model: String::new(),
                    video_url: "http://h/video/a.mp4".to_string(),
                },
            }
        );
        // videos uu tien hon links (:1385 truoc :1394)
        p.links = s(&["http://h/video/link.mp4"]);
        assert!(matches!(
            hp(&p, &mut st, &o, &l, start).unwrap(),
            PollDecision::NeedRaw {
                fallback: RawFallback::Unwatermarked { .. },
                ..
            }
        ));
    }

    // ===== Buoc 11: links (:1394-1428) =====

    #[test]
    fn hp_links_grace_roi_tai_link() {
        let start = t0();
        let o = opts(Mode::Pro);
        let l = Limits::default();
        assert_eq!(l.link_raw_grace_s, 45);
        let mut st = PollState::new(start, &o);
        let link = format!("https://cdn/{VID_B}/video/x.mp4?sig=1");
        let model_a = format!(r#"{{"id":"{VID_A}"}}"#);
        let model_b = format!(r#"{{"id":"{VID_B}"}}"#);
        let mut p = poll_with(&["こちらです"]);
        p.links = vec![link.clone()];
        p.video_models = vec![model_a.clone(), model_b.clone()];
        let now1 = start + secs(3);
        // trong grace: thu raw cho model khop (B), khong co -> doi; ghi link_first_seen, last_activity
        assert_eq!(
            hp(&p, &mut st, &o, &l, now1).unwrap(),
            PollDecision::NeedRaw {
                models: vec![model_b.clone()],
                fallback: RawFallback::Wait,
            }
        );
        assert_eq!(st.link_first_seen, Some(now1));
        assert_eq!(st.last_activity, now1);
        assert!(st.link_wait_logged);
        // 44 s sau lan dau thay: van doi (last_activity = now — :1411)
        let now2 = now1 + secs(44);
        assert_eq!(
            hp(&p, &mut st, &o, &l, now2).unwrap(),
            PollDecision::NeedRaw {
                models: vec![model_b.clone()],
                fallback: RawFallback::Wait,
            }
        );
        assert_eq!(st.last_activity, now2);
        // 45 s: het grace -> tai link (LINK_PREVIEW_ASK = false)
        let now3 = now1 + secs(45);
        assert_eq!(
            hp(&p, &mut st, &o, &l, now3).unwrap(),
            PollDecision::NeedRaw {
                models: vec![model_b.clone()],
                fallback: RawFallback::Link(link.clone()),
            }
        );
        // Link khong co ma video / khong model khop -> models rong (Python `_raw_for_link` -> None)
        let only_a = std::slice::from_ref(&model_a);
        assert!(raw_models_for_link("http://h/video/plain.mp4", only_a).is_empty());
        assert!(raw_models_for_link(&link, only_a).is_empty());
        assert!(raw_models_for_link(&link, &[]).is_empty());
        // Nhieu model khop -> reversed (moi nhat truoc)
        let model_b2 = format!(r#"{{"id":"{VID_B}","v":2}}"#);
        assert_eq!(
            raw_models_for_link(&link, &[model_b.clone(), model_a.clone(), model_b2.clone()]),
            vec![model_b2, model_b]
        );
        // links[0] = MOI NHAT (:1339-1341)
        let mut st2 = PollState::new(start, &o);
        let mut p2 = poll_with(&["x"]);
        p2.links = s(&["http://h/video/new.mp4", "http://h/video/old.mp4"]);
        let l0 = Limits {
            link_raw_grace_s: 0,
            ..Limits::default()
        };
        assert_eq!(
            hp(&p2, &mut st2, &o, &l0, start).unwrap(),
            PollDecision::NeedRaw {
                models: vec![],
                fallback: RawFallback::Link("http://h/video/new.mp4".to_string()),
            }
        );
    }

    // ===== Buoc 12: Continue =====

    #[test]
    fn hp_continue_khi_khong_co_gi() {
        let start = t0();
        let o = opts(Mode::Fast);
        let l = Limits::default();
        let mut st = PollState::new(start, &o);
        assert_eq!(
            hp(&ChainPoll::default(), &mut st, &o, &l, start).unwrap(),
            PollDecision::Continue
        );
        let ok = ChainPoll {
            ok: true,
            status: 200,
            ..ChainPoll::default()
        };
        assert_eq!(
            hp(&ok, &mut st, &o, &l, start + secs(1000)).unwrap(),
            PollDecision::Continue
        );
    }

    // ===== Thu tu uu tien giua cac buoc (fix vong 2, Minor #3) =====

    /// Buoc truoc thang buoc sau: 3 (latest) truoc 4 (DOM); 3 duyet `latest` theo thu tu, tin dau
    /// khop thang; 5 (loc ignore) truoc 6 (policy) — video CU bi loc thi van xet policy; 6 (policy)
    /// truoc 7 (xac nhan) khi mot tin khop ca hai.
    #[test]
    fn hp_thu_tu_uu_tien_giua_cac_buoc() {
        let start = t0();
        let o = opts(Mode::Pro);
        let l = Limits::default();
        // 3 truoc 4: latest = credit, DOM = limit -> Credit (khong phai "(DOM)")
        let mut st = PollState::new(start, &o);
        let p = poll_with(&["残高不足のため生成できません"]);
        let err = handle_poll(
            &p,
            &mut st,
            &o,
            &l,
            start,
            Some("動画生成の1日あたりの上限に達しました"),
        )
        .unwrap_err();
        assert!(
            matches!(&err, DolaError::Credit(m) if m == "Insufficient quota: 残高不足のため生成できません"),
            "{err}"
        );
        // 3 duyet theo thu tu: latest[0] = refusal, latest[1] = limit -> PromptViolation (tin dau)
        let mut st = PollState::new(start, &o);
        let p = poll_with(&[
            "I am temporarily unable to generate the content you requested.",
            "動画生成の1日あたりの上限に達しました",
        ]);
        let err = hp(&p, &mut st, &o, &l, start).unwrap_err();
        assert!(matches!(err, DolaError::PromptViolation(_)), "{err}");
        // 5 truoc 6: video duy nhat bi ignore (theo ma) + latest policy -> PromptViolation
        let mut o5 = opts(Mode::Pro);
        o5.ignore_videos
            .insert(format!("http://cdn.example/{VID_A}/old.mp4"));
        let mut st = PollState::new(start, &o5);
        let mut p = poll_with(&[r#"{"note": "content policy violation"}"#]);
        p.videos = vec![format!("http://other.example/{VID_A}/again.mp4")];
        p.video_models = vec![String::new()];
        let err = hp(&p, &mut st, &o5, &l, start).unwrap_err();
        assert!(matches!(err, DolaError::PromptViolation(_)), "{err}");
        // 6 truoc 7: mot tin khop ca policy (chan dung, co chu credit -> classify None) lan
        // needs_confirmation ("ご確認ください") -> PromptViolation, KHONG ConfirmReply (oracle Python:
        // classify '' / policy True / needs_confirmation True)
        let mut st = PollState::new(start, &o);
        let text = "肖像保護のため、あなた自身が写っている動画のみ対応します。credit は消費されません。ご確認ください";
        assert_eq!(classify_dola_text(text), None, "tien de");
        assert!(needs_confirmation(&[text.to_string()]).is_some(), "tien de");
        let p = poll_with(&[text]);
        let err = hp(&p, &mut st, &o, &l, start).unwrap_err();
        assert!(
            matches!(&err, DolaError::PromptViolation(m) if m.starts_with(PORTRAIT_NOTE)),
            "{err}"
        );
        assert!(st.confirm_keys.is_empty());
    }

    // ===== Ham thuan phu tro =====

    #[test]
    fn should_extend_theo_modes_py() {
        let (t, mx, w) = (secs(300), secs(3600), secs(600));
        assert!(
            !should_extend(secs(299), t, mx, secs(0), w),
            "chua qua timeout"
        );
        assert!(should_extend(secs(300), t, mx, secs(599), w));
        assert!(
            !should_extend(secs(300), t, mx, secs(600), w),
            "im >= window"
        );
        assert!(!should_extend(secs(3600), t, mx, secs(0), w), "cham tran");
        assert!(should_extend(secs(3599), t, mx, secs(0), w));
    }

    #[test]
    fn is_auth_status_401_403() {
        assert!(is_auth_status(401));
        assert!(is_auth_status(403));
        assert!(!is_auth_status(200));
        assert!(!is_auth_status(500));
        assert!(!is_auth_status(0));
    }

    #[test]
    fn is_browser_closed_msg_cac_cach_dien_dat() {
        for m in [
            "Page has been closed",
            "Target closed",
            "loi CDP: Session with given id not found.",
            "No target with given id found",
            "Connection closed normally",
            "send failed because receiver is gone",
            "oneshot canceled",
            "WebSocket protocol error: Connection reset without closing handshake",
        ] {
            assert!(is_browser_closed_msg(m), "{m}");
        }
        for m in [
            "Request timed out.",
            "Execution context was destroyed",
            "target navigated or closed", // dieu huong (context_gone), khong phai dong
            "loi JavaScript trong trang: TypeError",
        ] {
            assert!(!is_browser_closed_msg(m), "{m}");
        }
    }

    #[test]
    fn latest_tail_noi_bang_pipe_bo_xuong_dong_400_ky_tu() {
        assert_eq!(latest_tail(&[]), "");
        assert_eq!(latest_tail(&s(&["a\nb", "c"])), "a b | c");
        let long = "あ".repeat(500);
        let got = latest_tail(&[long, "end".to_string()]);
        assert_eq!(got.chars().count(), 400);
        assert!(got.ends_with(" | end"));
    }

    #[test]
    fn merge_cookie_header_ghi_de_theo_ten_them_cuoi() {
        let ov = vec![
            ("msToken".to_string(), "new".to_string()),
            ("ttwid".to_string(), "t1".to_string()),
            ("msToken".to_string(), "newer".to_string()),
        ];
        assert_eq!(
            merge_cookie_header(COOKIE_BASE, &ov),
            "sessionid=s1; msToken=newer; ttwid=t1"
        );
        assert_eq!(merge_cookie_header(COOKIE_BASE, &[]), COOKIE_BASE);
        assert_eq!(merge_cookie_header("", &ov[..1]), "msToken=new");
        assert_eq!(merge_cookie_header("", &[]), "");
    }

    #[test]
    fn confirm_key_theo_latest_da_strip() {
        assert_eq!(confirm_key(&s(&["a ", " b"])), confirm_key(&s(&["a", "b"])));
        assert_ne!(confirm_key(&s(&["a", "b"])), confirm_key(&s(&["a b"])));
        assert_eq!(confirm_key(&[]), "");
    }

    /// `DOM_TAIL_JS` chep nguyen van `video_worker_ui.py:1306` (inline, khong phai hang co ten).
    #[test]
    fn dom_tail_js_khop_python() {
        let Some(src) = super::super::py_src::python_source("src/video/video_worker_ui.py") else {
            return;
        };
        let needle = format!("page.evaluate(\"{DOM_TAIL_JS}\")");
        assert!(
            src.contains(&needle),
            "khong thay {needle:?} trong nguon Python"
        );
    }

    // ===== Vong poll (wiremock + FakePollPage) =====

    /// Trang gia: cookie co dinh, captcha/DOM/loi tuy chinh, ghi lai tra loi da gui.
    struct FakePollPage {
        cookie: String,
        captcha: bool,
        dom: String,
        cookie_err: Option<&'static str>,
        replies: Mutex<Vec<String>>,
    }

    impl FakePollPage {
        fn new() -> Self {
            Self {
                cookie: COOKIE_BASE.to_string(),
                captcha: false,
                dom: String::new(),
                cookie_err: None,
                replies: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl PollPage for FakePollPage {
        async fn cookie_header(&self) -> Result<String, BrowserError> {
            if let Some(m) = self.cookie_err {
                return Err(BrowserError::Cdp(CdpError::msg(m)));
            }
            Ok(self.cookie.clone())
        }
        async fn has_captcha(&self) -> Result<bool, BrowserError> {
            Ok(self.captcha)
        }
        async fn dom_tail(&self) -> Result<String, BrowserError> {
            Ok(self.dom.clone())
        }
        async fn send_reply(&self, text: &str) -> Result<(), BrowserError> {
            self.replies.lock().unwrap().push(text.to_string());
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecHooks {
        polls: AtomicUsize,
        stages: Mutex<Vec<String>>,
        replies: Mutex<Vec<String>>,
    }

    impl PollHooks for RecHooks {
        fn on_poll(&self, _elapsed: Duration) {
            self.polls.fetch_add(1, Ordering::SeqCst);
        }
        fn on_stage(&self, stage: &str) {
            self.stages.lock().unwrap().push(stage.to_string());
        }
        fn on_agent_reply(&self, text: &str) {
            self.replies.lock().unwrap().push(text.to_string());
        }
    }

    /// Tra lan luot cac response; het danh sach -> lap lai cai cuoi.
    struct Seq {
        calls: AtomicUsize,
        responses: Vec<ResponseTemplate>,
    }

    impl Seq {
        fn new(responses: Vec<ResponseTemplate>) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                responses,
            }
        }
    }

    impl Respond for Seq {
        fn respond(&self, _req: &Request) -> ResponseTemplate {
            let i = self.calls.fetch_add(1, Ordering::SeqCst);
            self.responses[i.min(self.responses.len() - 1)].clone()
        }
    }

    fn http() -> DolaHttp {
        DolaHttp::new("UA-TEST/1.0".to_string(), None).expect("client")
    }

    /// Limits cho test vong: khong ngu giua cac nhip; khong gia han.
    fn fast_limits() -> Limits {
        Limits {
            poll_interval_s: 0,
            pro_activity_window_s: 0,
            ..Limits::default()
        }
    }

    /// Body poll: 1 tin bot voi `latest` (+ khoi video 2074 khi `video` co).
    fn chain_body(latest: &str, video: Option<(&str, &str)>) -> Value {
        let mut content = vec![json!({ "content": { "text_block": { "text": latest } } })];
        if let Some((url, model)) = video {
            content.push(
                json!({ "block_type": 2074, "content": { "creation_block": { "creations": [
                { "type": 2, "video": { "download_url": url, "video_model": model } }
            ] } } }),
            );
        }
        json!({ "downlink_body": { "pull_singe_chain_downlink_body": { "messages": [
            { "user_type": 2, "content": content }
        ] } } })
    }

    fn raw_body(url: &str) -> Value {
        json!({ "video_info": { "data": { "video_list": {
            "v1": {"main_url": B64.encode(url), "bitrate": 2000, "vwidth": 1280, "vheight": 720}
        } } } })
    }

    fn poll_path(server: &MockServer) -> String {
        format!("{}/im/chain/single", server.uri())
    }

    async fn requests_to(server: &MockServer, p: &str) -> Vec<Request> {
        server
            .received_requests()
            .await
            .expect("recording")
            .into_iter()
            .filter(|r| r.url.path() == p)
            .collect()
    }

    #[tokio::test]
    async fn vong_tra_video_sau_2_lan_poll_raw_no_logo() {
        let server = MockServer::start().await;
        let raw_api = format!("{}/video/fplay/1/sig/vid?key_seed=abc", server.uri());
        let raw_mp4 = format!("{}/video/raw.mp4?x-signature=SECRET", server.uri());
        let model = json!({
            "fallback_api": raw_api,
            "video_list": {"v1": {"main_url": B64.encode(format!("{}/video/main.mp4", server.uri())), "bitrate": 100}}
        })
        .to_string();
        Mock::given(method("POST"))
            .and(path("/im/chain/single"))
            .respond_with(Seq::new(vec![
                ResponseTemplate::new(200).set_body_json(chain_body("生成しています", None)),
                ResponseTemplate::new(200).set_body_json(chain_body(
                    "できました",
                    Some(("http://v16-dola.dola.com/video/wm.mp4", &model)),
                )),
            ]))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/fplay/1/sig/vid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(raw_body(&raw_mp4)))
            .mount(&server)
            .await;
        let body: Vec<u8> = (0..70_000u32)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect();
        Mock::given(method("GET"))
            .and(path("/video/raw.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;

        let dir = tempdir().unwrap();
        let page = FakePollPage::new();
        let hooks = RecHooks::default();
        let art = poll_and_download_at(
            &page,
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &hooks,
        )
        .await
        .expect("artifact");
        assert_eq!(art.note, NOTE_RAW);
        assert_eq!(art.video_url, raw_mp4);
        assert!(!art.watermarked);
        assert_eq!(art.bytes, 70_000);
        assert_eq!(art.account, ACC);
        assert_eq!(art.conversation_id, conv());
        assert_eq!(std::fs::read(&art.local_path).unwrap(), body);
        assert_eq!(art.local_path.parent(), Some(dir.path()));
        let name = art.local_path.file_name().unwrap().to_string_lossy();
        assert!(
            name.starts_with("acc1_") && name.ends_with(".mp4"),
            "{name}"
        );
        // 2 lan poll, cookie tu CDP
        let polls = requests_to(&server, "/im/chain/single").await;
        assert_eq!(polls.len(), 2);
        for r in &polls {
            assert_eq!(
                r.headers.get("cookie").and_then(|v| v.to_str().ok()),
                Some(COOKIE_BASE)
            );
        }
        // hooks: on_poll lan dau ngay, lan 2 (< 30 s) bi tiet luu -> dung 1 (ON_POLL_EVERY);
        // stage generating -> downloading; agent_reply moi khi latest doi
        assert_eq!(hooks.polls.load(Ordering::SeqCst), 1);
        assert_eq!(
            *hooks.stages.lock().unwrap(),
            vec![STAGE_GENERATING.to_string(), STAGE_DOWNLOADING.to_string()]
        );
        assert_eq!(
            *hooks.replies.lock().unwrap(),
            vec!["生成しています".to_string(), "できました".to_string()]
        );
        assert!(page.replies.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn vong_raw_500_roi_ve_unwatermarked_priority() {
        let server = MockServer::start().await;
        let main_mp4 = format!("{}/video/main.mp4", server.uri());
        let model = json!({
            "fallback_api": format!("{}/video/fplay/1/sig/vid", server.uri()),
            "video_list": {"v1": {"main_url": B64.encode(&main_mp4), "bitrate": 100}}
        })
        .to_string();
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(chain_body(
                "できました",
                Some(("http://v16-dola.dola.com/video/wm.mp4", &model)),
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/fplay/1/sig/vid"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/main.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 1000]))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let art = poll_and_download_at(
            &FakePollPage::new(),
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .expect("artifact");
        assert_eq!(art.note, NOTE_UNWATERMARKED);
        assert_eq!(art.video_url, main_mp4);
        assert_eq!(art.bytes, 1000);
        assert_eq!(
            requests_to(&server, "/video/fplay/1/sig/vid").await.len(),
            1
        );
    }

    /// Ruling #4: `fetch_raw_video` `Err` (khong ket noi duoc) coi nhu `None` -> thu `fallback_api`
    /// ke tiep (<= 3) -> raw. Cong "chet" = cong vua bind roi tha (fix vong 2, Minor #7: chac chan
    /// RST ngay, khong phu thuoc firewall DROP o cong 9).
    #[tokio::test]
    async fn vong_raw_loi_mang_coi_nhu_none_thu_api_ke() {
        let server = MockServer::start().await;
        let raw_mp4 = format!("{}/video/raw2.mp4", server.uri());
        let dead_port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind cong 0");
            l.local_addr().expect("addr").port()
        };
        let model = json!({
            "fallback_api": format!("http://127.0.0.1:{dead_port}/video/dead"),
            "nested": {"fallback_api": format!("{}/video/fplay/2/sig/vid", server.uri())},
            "video_list": {"v1": {"main_url": B64.encode(format!("{}/video/main.mp4", server.uri()))}}
        })
        .to_string();
        assert_eq!(
            find_fallback_apis(&model).len(),
            2,
            "tien de: 2 fallback_api theo thu tu"
        );
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(chain_body(
                "できました",
                Some(("http://v16-dola.dola.com/video/wm.mp4", &model)),
            )))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/fplay/2/sig/vid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(raw_body(&raw_mp4)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/raw2.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1u8; 500]))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let art = poll_and_download_at(
            &FakePollPage::new(),
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .expect("artifact");
        assert_eq!(art.note, NOTE_RAW);
        assert_eq!(art.video_url, raw_mp4);
    }

    #[tokio::test]
    async fn vong_401_bao_logged_out() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let err = poll_and_download_at(
            &FakePollPage::new(),
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&err, DolaError::LoggedOut { account, why, revoked: false }
                if account == ACC && why == "conversation API HTTP 401 while polling"),
            "{err}"
        );
        assert_eq!(
            err.to_string(),
            "LOGGED_OUT: acc1: conversation API HTTP 401 while polling"
        );
        assert!(err.rotates_account());
    }

    /// Loi HTTP khac (500) -> `continue` (:1508-1509), khong bao loi; het gio -> Timeout voi
    /// thong diep dung (`:1522-1524`), `last_latest` tu lan poll ok truoc do.
    #[tokio::test]
    async fn vong_timeout_bao_dung_thong_diep() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(Seq::new(vec![
                ResponseTemplate::new(200).set_body_json(chain_body("まだ生成中\nです", None)),
                ResponseTemplate::new(500),
            ]))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let o = PollOpts {
            timeout: Duration::from_secs(1),
            ..opts(Mode::Fast)
        };
        let started = Instant::now();
        let err = poll_and_download_at(
            &FakePollPage::new(),
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &o,
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .unwrap_err();
        let took = started.elapsed();
        assert!(took >= Duration::from_secs(1), "{took:?}");
        let DolaError::Timeout(m) = &err else {
            panic!("{err}");
        };
        // N = int(elapsed) tinh BEN TRONG ham (co the nho hon `took` mot don vi neu vua qua moc giay).
        let n: u64 = m
            .strip_prefix("No video generated within ")
            .and_then(|r| r.split('s').next())
            .and_then(|d| d.parse().ok())
            .unwrap_or_else(|| panic!("khong doc duoc N trong {m:?}"));
        assert!(n >= 1 && n <= took.as_secs(), "N={n} took={took:?}");
        let want = format!(
            "No video generated within {n}s (conversation_id={CONV}) | agent: まだ生成中 です"
        );
        assert_eq!(*m, want);
        assert_eq!(err.failure_code(), None);
        assert!(requests_to(&server, "/im/chain/single").await.len() >= 2);
    }

    /// Ruling Task 9 I-1 / #2: captcha moi nhip TRUOC `pull_chain` -> `RISK_CONTROL`, 0 request HTTP.
    #[tokio::test]
    async fn vong_captcha_bao_risk_control_truoc_khi_poll() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let mut page = FakePollPage::new();
        page.captcha = true;
        let err = poll_and_download_at(
            &page,
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DolaError::RiskControl(_)), "{err}");
        assert_eq!(err.to_string(), captcha_error().to_string());
        assert!(requests_to(&server, "/im/chain/single").await.is_empty());
    }

    #[tokio::test]
    async fn vong_trang_dong_bao_browser_closed() {
        let server = MockServer::start().await;
        let dir = tempdir().unwrap();
        let mut page = FakePollPage::new();
        page.cookie_err = Some("Target closed");
        let err = poll_and_download_at(
            &page,
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&err, DolaError::BrowserClosed(m)
                if m == "browser or tab was closed while generating (acc1)"),
            "{err}"
        );
    }

    /// Buoc 7 qua vong: agent xin xac nhan -> gui tra loi (stage `confirm`) -> lan poll sau co video.
    /// Chay cho CA Pro lan Nhanh (fix vong 1: Python `:1354` khong gate theo mode).
    async fn vong_tra_loi_xac_nhan_roi_tai(mode: Mode) {
        let server = MockServer::start().await;
        let raw_mp4 = format!("{}/video/raw.mp4", server.uri());
        let model =
            json!({"fallback_api": format!("{}/video/fplay/1/sig/vid", server.uri())}).to_string();
        Mock::given(method("POST"))
            .respond_with(Seq::new(vec![
                ResponseTemplate::new(200).set_body_json(chain_body("ご確認ください", None)),
                ResponseTemplate::new(200).set_body_json(chain_body(
                    "できました",
                    Some(("http://v16-dola.dola.com/video/wm.mp4", &model)),
                )),
            ]))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/fplay/1/sig/vid"))
            .respond_with(ResponseTemplate::new(200).set_body_json(raw_body(&raw_mp4)))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/raw.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![3u8; 100]))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let page = FakePollPage::new();
        let hooks = RecHooks::default();
        let art = poll_and_download_at(
            &page,
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(mode),
            &fast_limits(),
            &hooks,
        )
        .await
        .expect("artifact");
        assert_eq!(art.note, NOTE_RAW, "{mode:?}");
        assert_eq!(
            *page.replies.lock().unwrap(),
            vec![CONFIRM_REPLY.to_string()],
            "{mode:?}"
        );
        assert_eq!(
            *hooks.stages.lock().unwrap(),
            vec![
                STAGE_GENERATING.to_string(),
                STAGE_CONFIRM.to_string(),
                STAGE_DOWNLOADING.to_string()
            ],
            "{mode:?}"
        );
    }

    #[tokio::test]
    async fn vong_pro_tra_loi_xac_nhan_roi_tai() {
        vong_tra_loi_xac_nhan_roi_tai(Mode::Pro).await;
    }

    #[tokio::test]
    async fn vong_nhanh_cung_tra_loi_xac_nhan_roi_tai() {
        vong_tra_loi_xac_nhan_roi_tai(Mode::Fast).await;
    }

    /// Ruling #3: `Set-Cookie` cua poll truoc ghi de len chuoi CDP o lan poll sau.
    #[tokio::test]
    async fn vong_set_cookie_gop_vao_cookie_lan_sau() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(Seq::new(vec![
                ResponseTemplate::new(200)
                    .set_body_json(json!({}))
                    .insert_header("set-cookie", "msToken=new; Path=/")
                    .append_header("set-cookie", "ttwid=t1; Path=/"),
                ResponseTemplate::new(401),
            ]))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let err = poll_and_download_at(
            &FakePollPage::new(),
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DolaError::LoggedOut { .. }), "{err}");
        let polls = requests_to(&server, "/im/chain/single").await;
        assert_eq!(polls.len(), 2);
        let cookie = |i: usize| {
            polls[i]
                .headers
                .get("cookie")
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string)
        };
        assert_eq!(cookie(0).as_deref(), Some(COOKIE_BASE));
        assert_eq!(
            cookie(1).as_deref(),
            Some("sessionid=s1; msToken=new; ttwid=t1")
        );
    }

    /// Chi co link trong text (khong khoi 2074), grace 0 -> tai link, `watermarked` theo URL.
    #[tokio::test]
    async fn vong_link_trong_text_tai_link_sau_grace() {
        let server = MockServer::start().await;
        let link = format!("{}/video/link_watermark.mp4", server.uri());
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chain_body(&format!("動画はこちら: {link}"), None)),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/video/link_watermark.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![9u8; 300]))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let l = Limits {
            link_raw_grace_s: 0,
            ..fast_limits()
        };
        let art = poll_and_download_at(
            &FakePollPage::new(),
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Pro),
            &l,
            &NoopHooks,
        )
        .await
        .expect("artifact");
        assert_eq!(art.note, NOTE_LINK);
        assert_eq!(art.video_url, link);
        assert!(art.watermarked, "URL co 'watermark' -> is_watermarked_url");
        assert_eq!(art.bytes, 300);
    }

    /// Buoc 4 qua vong: DOM `<main>` co chu het credit -> CREDIT (DOM), khong tai.
    #[tokio::test]
    async fn vong_dom_scan_bao_credit() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(chain_body("お待ちください", None)),
            )
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let mut page = FakePollPage::new();
        page.dom = "…\n残高不足のため生成できません".to_string();
        let err = poll_and_download_at(
            &page,
            &http(),
            &poll_path(&server),
            ACC,
            &conv(),
            dir.path(),
            &opts(Mode::Fast),
            &fast_limits(),
            &NoopHooks,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(&err, DolaError::Credit(m) if m.starts_with("Insufficient quota (DOM): ")),
            "{err}"
        );
    }
}
