//! HTTP "nua nhan" cua luong Dola (Task 10, P2): poll `/im/chain/single` bang cookie trinh duyet,
//! lay ban KHONG logo qua `fallback_api`, tai mp4 ve dia — bang `reqwest` (Python: `aiohttp`).
//! Task 11 (vong poll) goi cac ham nay; o day KHONG co vong lap/quyet dinh.
//!
//! | Rust | Python | Ghi chu |
//! |---|---|---|
//! | [`LOGIN_INVALID_CODES`] (re-export) | `hub_browser.py:746` | `lvc_core::chain` |
//! | [`POLL_UA`] | `hub_browser.py:747` | CHI de tham khao — R2: UA THAT cua trinh duyet (`ops::user_agent`) |
//! | [`POLL_PARAMS`] | `hub_browser.py:748-750` | chep nguyen van, DUNG THU TU (test so byte voi nguon) |
//! | [`DolaHttp::new`] | `HttpSession.__init__` `:801-810` + `_sess` `:817-823` | timeout total 40 s, proxy `aiohttp_proxy_parts` (`proxies.py:105-110`) |
//! | [`DolaHttp::pull_chain`] / [`DolaHttp::pull_chain_at`] | `HttpSession.poll` `:853-873` | POST + query + body `POLL_JS` (`video_worker.py:19-57`) |
//! | [`poll_body`] | `:856-860` / `POLL_JS` dong 37-55 | JSON `cmd 3100` |
//! | [`PullOutcome::set_cookies`] | `:867-868` (`cookie_jar` -> `_cookies`) | cap `(ten, gia tri)` tu `Set-Cookie` |
//! | [`DolaHttp::fetch_raw_video`] | `unwatermark.fetch_raw_video` `:225-240` | 1 lan GET; vong <= 3 `fallback_api` (`resolve_raw_video` `:243-251`) o Task 11 |
//! | [`DolaHttp::download`] | `video_worker._download` `:170-187` | stream -> `<out>.part` -> rename |
//! | [`output_path`] | `:176` `f"{account}_{time.strftime('%Y%m%d_%H%M%S')}.mp4"` | UTC (ruling #4, `util::timestamp_utc_at`) |
//!
//! # Header (so voi Python)
//! | Header | poll (`_sess` `:820-822` + `:861`) | `fetch_raw_video` (`:236`) | `download` (`:181-182`) |
//! |---|---|---|---|
//! | `User-Agent` | `POLL_UA` -> **UA that** (R2) | aiohttp mac dinh -> **UA that** (ruling) | aiohttp mac dinh -> **UA that** (ruling) |
//! | `Accept` | `*/*` | `application/json,text/plain,*/*` | `*/*` (reqwest mac dinh = aiohttp mac dinh) |
//! | `Accept-Language` | `ja-JP,ja;q=0.9` | — | — |
//! | `Origin` | `https://www.dola.com` | — | — |
//! | `agw-js-conv` | `str` | — | — |
//! | `Content-Type` | `application/json; encoding=utf-8` | — | — |
//! | `Referer` | `https://www.dola.com/chat/<id>` | — | — |
//! | `Cookie` | chuoi `ops::cookie_header` (chi poll — Python `HttpSession` la cookie-only) | **KHONG** | **KHONG** |
//! | `Accept-Encoding` | `gzip, br` (reqwest tu them/giai nen khi bat feature; aiohttp cung tu giai nen) | nt | nt |
//!
//! # Khac Python (co chu y)
//! - **UA**: Python poll dung `POLL_UA` co dinh (Chrome/153); Rust dung UA THAT cua trinh duyet dang
//!   dang nhap (R2 — cookie + UA cung mot trinh duyet, giam nguy co bi coi la bot). Tai file/fetch cung
//!   gui UA do (Python de aiohttp gui `Python/3.x aiohttp/3.x`) — vo hai, dong nhat.
//! - **Proxy**: `None` -> `no_proxy()` (aiohttp `trust_env=False` mac dinh: KHONG doc `HTTP_PROXY`/registry);
//!   `Some` -> `Proxy::all(server)` (+ `basic_auth` khi co user) — cung 1 proxy cho ca 3 loai request
//!   (Python: `resolve_proxy` cung ra 1 proxy hieu luc cua tai khoan).
//! - **Cookie**: khong co `cookie_store` (feature `cookies` KHONG bat -> reqwest khong co jar) — header
//!   `Cookie` dat tuong minh moi lan poll; `Set-Cookie` tra ve cho caller tu gop (Python gop vao dict
//!   `_cookies` de sync ve file). Cookie HET HAN (`Max-Age <= 0`, hoac `Expires <= now` khi khong co
//!   `Max-Age` hop le — parser ngay RFC 6265 `util::cookie_date_unix`) bo qua nhu jar aiohttp xoa
//!   cookie do (fix vong 1, I-2; chi tiet + 2 diem lech aiohttp o [`parse_set_cookie_at`]).
//! - **`fetch_raw_video`**: Python `except Exception: return None` cho MOI loi; o day loi mang/timeout
//!   -> `Err(Http)` (caller — Task 11 — coi nhu `None`, thu `fallback_api` ke tiep), HTTP != 200 hay
//!   JSON hong -> `Ok(None)` nhu Python.
//! - **`download`**: ghi `<out>.part` roi `rename` (Python ghi thang); 0 byte -> `Http("empty body")`
//!   (Python de lai file rong); loi I/O -> `DolaError::Http("io: ...")` (khong co bien the Io).
//! - **Ten file**: UTC thay vi gio may (xem `util`).
//!
//! # Log
//! KHONG BAO GIO log gia tri `Cookie`/`Set-Cookie`, KHONG log URL day du (URL CDN mang `x-signature`)
//! — chi `host/path` ([`url_host_path`]) va so byte. Thong diep `DolaError::Http` cung chi chua
//! `host/path` (`reqwest::Error::without_url`).

use std::error::Error as StdError;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use futures::StreamExt;
pub use lvc_core::LOGIN_INVALID_CODES;
use lvc_core::{
    ChainPoll, ProxySpec, RAW_PARAMS, chain_http_status, parse_chain, pick_raw_video, replace_query,
};
use reqwest::header::{
    ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE, COOKIE, HeaderMap, HeaderValue, ORIGIN, REFERER,
    SET_COOKIE,
};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

use super::error::DolaError;
use super::submit::ConversationId;
use super::util;

/// `hub_browser.py:862` — endpoint pull hoi thoai.
pub const POLL_URL: &str = "https://www.dola.com/im/chain/single";
/// `hub_browser.py:822` `Origin` / `:861` tien to `Referer`.
pub const DOLA_ORIGIN: &str = "https://www.dola.com";
/// `hub_browser.py:747` `POLL_UA` — CHI DE THAM KHAO (chep nguyen van). Ruling R2: KHONG dung; UA
/// gui di la UA THAT cua trinh duyet (`ops::user_agent`) do caller truyen vao [`DolaHttp::new`].
pub const POLL_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36";
/// `hub_browser.py:748-750` `POLL_PARAMS` — chep nguyen van, DUNG THU TU dict Python (= `POLL_JS`
/// `video_worker.py:23-29`); `web_tab_id=uuid4` noi them luc goi (`:856`).
pub const POLL_PARAMS: [(&str, &str); 14] = [
    ("version_code", "20800"),
    ("language", "ja"),
    ("device_platform", "web"),
    ("doubao_device_platform", "web"),
    ("aid", "495671"),
    ("real_aid", "495671"),
    ("pkg_type", "release_version"),
    ("pc_version", "3.32.61"),
    ("doubao_pc_version", "3.32.61"),
    ("region", "JP"),
    ("sys_region", "JP"),
    ("samantha_web", "1"),
    ("web_platform", "browser"),
    ("use-olympus-account", "1"),
];
/// `hub_browser.py:821` `Accept-Language`.
pub const ACCEPT_LANGUAGE_JA: &str = "ja-JP,ja;q=0.9";
/// `unwatermark.py:236` header `accept` khi GET `fallback_api`.
pub const RAW_ACCEPT: &str = "application/json,text/plain,*/*";
/// `hub_browser.py:820` `ClientTimeout(total=40)` — timeout MAC DINH cua client (poll).
pub const POLL_TIMEOUT: Duration = Duration::from_secs(40);
/// `unwatermark.py:27` `FETCH_TIMEOUT = 30` — gia tri Task 11 nen truyen cho `fetch_raw_video`.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// `video_worker.py:177` `ClientTimeout(total=300)`.
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
/// `hub_browser.py:822` / `POLL_JS` dong 34.
const AGW_JS_CONV: &str = "str";
/// `hub_browser.py:861` / `POLL_JS` dong 33.
const POLL_CONTENT_TYPE: &str = "application/json; encoding=utf-8";
/// `hub_browser.py:872` `str(status_desc)[:120]`.
const DESC_MAX_CHARS: usize = 120;

/// Ket qua mot lan poll: `ChainPoll` (nhu dict Python) + cookie moi tu `Set-Cookie`
/// (`hub_browser.py:867-868`: Dola lam moi `msToken`/... -> caller cap nhat chuoi `Cookie` cua minh
/// cho lan poll sau, va sync ve file neu muon). Tra ve CA KHI HTTP != 200 (Python cap nhat jar ngay
/// khi nhan header, chi cham copy sang dict — ket qua cuoi nhu nhau). `Debug` tu viet (fix vong 1,
/// I-1): in `poll` binh thuong, `set_cookies` CHI TEN (`[("msToken", "***"), ...]`) — gia tri la
/// msToken/sessionid, khong bao gio duoc ra log du caller `{:?}` ca struct.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PullOutcome {
    pub poll: ChainPoll,
    /// `(ten, gia tri)` theo thu tu header `Set-Cookie`; caller UPSERT vao map cookie cua minh.
    pub set_cookies: Vec<(String, String)>,
}

impl std::fmt::Debug for PullOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let masked: Vec<(&str, &str)> = self
            .set_cookies
            .iter()
            .map(|(name, _)| (name.as_str(), "***"))
            .collect();
        f.debug_struct("PullOutcome")
            .field("poll", &self.poll)
            .field("set_cookies", &masked)
            .finish()
    }
}

/// Client HTTP dung chung cho ca ba loai request (poll/fetch/download) cua MOT tai khoan: cung UA,
/// cung proxy (`HttpSession` `:801-826` + `aiohttp_proxy_parts` `proxies.py:105-110`).
/// `Debug` tu viet: khong in client, proxy da tu che mat khau.
pub struct DolaHttp {
    client: reqwest::Client,
    ua: String,
    proxy: Option<ProxySpec>,
}

impl std::fmt::Debug for DolaHttp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DolaHttp")
            .field("ua", &self.ua)
            .field("proxy", &self.proxy)
            .finish_non_exhaustive()
    }
}

impl DolaHttp {
    /// `HttpSession._sess` `:818-823`: mot session, timeout total 40 s, UA co dinh; proxy
    /// `aiohttp_proxy_parts` -> `proxy=server, proxy_auth=BasicAuth(user, pw)` (`:807-808`).
    /// `user_agent` = UA THAT cua trinh duyet (R2). Khong co cookie jar (xem doc module).
    ///
    /// # Errors
    /// `DolaError::Http` khi UA khong phai header hop le, proxy khong parse duoc, hoac reqwest
    /// khong khoi tao duoc TLS. Thong diep KHONG chua mat khau proxy (`ProxySpec::server()` khong co
    /// user/pass).
    pub fn new(user_agent: String, proxy: Option<ProxySpec>) -> Result<Self, DolaError> {
        // `no_proxy()` truoc: aiohttp `trust_env=False` — khong doc HTTP(S)_PROXY/registry.
        let mut builder = reqwest::Client::builder()
            .user_agent(&user_agent)
            .timeout(POLL_TIMEOUT)
            .no_proxy();
        if let Some(spec) = &proxy {
            let server = spec.server();
            let mut p = reqwest::Proxy::all(&server).map_err(|e| {
                DolaError::Http(format!("proxy {server}: {}", error_chain(&e.without_url())))
            })?;
            if spec.has_auth() {
                p = p.basic_auth(
                    spec.username.as_deref().unwrap_or(""),
                    spec.password.as_deref().unwrap_or(""),
                );
            }
            builder = builder.proxy(p);
        }
        let client = builder
            .build()
            .map_err(|e| DolaError::Http(format!("reqwest client: {}", error_chain(&e))))?;
        Ok(Self {
            client,
            ua: user_agent,
            proxy,
        })
    }

    /// UA dang dung (de log/kiem tra — khong phai bi mat).
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.ua
    }

    #[must_use]
    pub fn proxy(&self) -> Option<&ProxySpec> {
        self.proxy.as_ref()
    }

    /// `HttpSession.poll` `:853-873` tren [`POLL_URL`]. `cookie_header` = chuoi `ops::cookie_header`
    /// (`name=value; ...`) — KHONG BAO GIO duoc log.
    ///
    /// # Errors
    /// Xem [`Self::pull_chain_at`].
    pub async fn pull_chain(
        &self,
        cookie_header: &str,
        conv: &ConversationId,
    ) -> Result<PullOutcome, DolaError> {
        self.pull_chain_at(cookie_header, conv, POLL_URL).await
    }

    /// Nhu [`Self::pull_chain`] nhung endpoint tuy y (test tren server cuc bo). `Origin`/`Referer`
    /// VAN la `https://www.dola.com...` nhu Python (header, khong phu thuoc endpoint).
    ///
    /// Ket qua (theo dung thu tu kiem cua Python `:864-873`):
    /// - HTTP != 200 -> `ChainPoll { ok: false, status, .. }` (`:864-865`; KHONG doc body);
    /// - body `status_code` khac 0/None -> `ok: false`, `status` = 401 neu trong
    ///   [`LOGIN_INVALID_CODES`] else 500 (`:869-872`, [`chain_http_status`]); `code`/`desc[:120]`
    ///   chi ghi log (`ChainPoll` khong co truong do — `_handle_poll` Python chi doc `ok`/`status`);
    /// - con lai [`parse_chain`] (`:873`).
    ///
    /// # Errors
    /// `DolaError::Http`: loi mang/timeout (40 s), `cookie_header` khong phai header hop le, body
    /// 200 khong phai JSON (Python `json.loads` nem — `:866`).
    pub async fn pull_chain_at(
        &self,
        cookie_header: &str,
        conv: &ConversationId,
        poll_url: &str,
    ) -> Result<PullOutcome, DolaError> {
        let cookie = HeaderValue::from_str(cookie_header).map_err(|_| {
            DolaError::Http("Cookie header khong hop le (ky tu ngoai ASCII hien thi)".to_string())
        })?;
        // :856 `params = {**POLL_PARAMS, "web_tab_id": str(uuid.uuid4())}`
        let web_tab_id = uuid::Uuid::new_v4().to_string();
        let mut params: Vec<(&str, &str)> = POLL_PARAMS.to_vec();
        params.push(("web_tab_id", &web_tab_id));
        // :857-860 body; :861 header rieng; :820-822 header session
        let body = poll_body(conv, &uuid::Uuid::new_v4().to_string());
        let referer = format!("{DOLA_ORIGIN}/chat/{conv}");
        let host_path = url_host_path(poll_url);
        let resp = self
            .client
            .post(poll_url)
            .query(&params)
            .header(ACCEPT, "*/*")
            .header(ACCEPT_LANGUAGE, ACCEPT_LANGUAGE_JA)
            .header(ORIGIN, DOLA_ORIGIN)
            .header("agw-js-conv", AGW_JS_CONV)
            .header(CONTENT_TYPE, POLL_CONTENT_TYPE)
            .header(REFERER, referer)
            .header(COOKIE, cookie)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| http_err("poll", poll_url, e))?;
        let status = resp.status().as_u16();
        let set_cookies = set_cookie_pairs(resp.headers());
        if status != 200 {
            // :864-865 `{"ok": False, "status": resp.status, "texts": [], "videos": []}`
            warn!(status, host_path, "poll HTTP != 200");
            return Ok(PullOutcome {
                poll: ChainPoll {
                    ok: false,
                    status,
                    ..ChainPoll::default()
                },
                set_cookies,
            });
        }
        // :866 `json.loads(await resp.text())`
        let text = resp
            .text()
            .await
            .map_err(|e| http_err("poll", poll_url, e))?;
        let data: Value = serde_json::from_str(&text).map_err(|e| {
            DolaError::Http(format!(
                "poll {host_path}: body khong phai JSON ({} byte): {e}",
                text.len()
            ))
        })?;
        // :869-872 — 712012001 "登录校验不合法" = mat phien -> 401, ma khac -> 500
        if let Some(status) = chain_http_status(&data) {
            let desc: String = data
                .get("status_desc")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(DESC_MAX_CHARS)
                .collect();
            let code = data.get("status_code").map(Value::to_string);
            warn!(
                status,
                code, desc, "poll: HTTP 200 nhung status_code bao loi"
            );
            return Ok(PullOutcome {
                poll: ChainPoll {
                    ok: false,
                    status,
                    ..ChainPoll::default()
                },
                set_cookies,
            });
        }
        let poll = parse_chain(&data);
        debug!(
            videos = poll.videos.len(),
            texts = poll.texts.len(),
            set_cookies = set_cookies.len(),
            "poll ok"
        );
        Ok(PullOutcome { poll, set_cookies })
    }

    /// `unwatermark.fetch_raw_video` `:225-240`: GET `replace_query(fallback_api, RAW_PARAMS)` voi
    /// `accept: application/json,text/plain,*/*`, KHONG cookie (URL da co chu ky), `timeout` total
    /// (Task 11 truyen [`FETCH_TIMEOUT`] = 30 s) -> [`pick_raw_video`] voi `api_url` = URL da
    /// `replace_query` (`:240`, `key_seed` co the nam trong query). HTTP != 200 (`:237-238`) hay body
    /// khong phai JSON -> `Ok(None)`.
    ///
    /// # Errors
    /// `DolaError::Http` khi loi mang/timeout — Python nuot thanh `None` (`:239`); caller (Task 11,
    /// vong <= 3 `fallback_api`) nen coi `Err` nhu `None` (ghi log) roi thu URL ke tiep.
    pub async fn fetch_raw_video(
        &self,
        fallback_api: &str,
        timeout: Duration,
    ) -> Result<Option<String>, DolaError> {
        let url = replace_query(fallback_api, &RAW_PARAMS);
        let host_path = url_host_path(&url);
        let resp = self
            .client
            .get(&url)
            .timeout(timeout)
            .header(ACCEPT, RAW_ACCEPT)
            .send()
            .await
            .map_err(|e| http_err("fetch_raw_video", &url, e))?;
        let status = resp.status().as_u16();
        if status != 200 {
            debug!(status, host_path, "fetch_raw_video: HTTP != 200 -> None");
            return Ok(None);
        }
        let text = resp
            .text()
            .await
            .map_err(|e| http_err("fetch_raw_video", &url, e))?;
        let Ok(payload) = serde_json::from_str::<Value>(&text) else {
            debug!(
                host_path,
                bytes = text.len(),
                "fetch_raw_video: body khong phai JSON -> None"
            );
            return Ok(None);
        };
        let picked = pick_raw_video(&payload, &url);
        debug!(host_path, found = picked.is_some(), "fetch_raw_video");
        Ok(picked)
    }

    /// `video_worker._download` `:170-187`: `mkdir -p` thu muc cha (`:175`), GET thuan (khong cookie,
    /// khong Referer; UA = UA trinh duyet — ruling), timeout total 300 s (`:177`), status != 2xx ->
    /// loi (`raise_for_status` `:182`; 3xx da duoc reqwest/aiohttp tu theo redirect), stream body vao
    /// `<out>.part` (Python `iter_chunked(1<<16)` ghi thang `out`) roi `rename` -> `out`. Tra so byte
    /// da ghi; 0 byte -> `Http("empty body")` (ruling #4; Python de lai file rong). Moi duong loi
    /// deu xoa `.part`.
    ///
    /// # Errors
    /// `DolaError::Http`: loi mang/timeout/`HTTP <status> <host/path>`, body rong, hoac loi I/O
    /// (`io: ...`) — thong diep chi chua `host/path`, KHONG chua query co chu ky.
    pub async fn download(&self, url: &str, out: &Path) -> Result<u64, DolaError> {
        let host_path = url_host_path(url);
        if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| io_err("mkdir", parent, &e))?;
        }
        let resp = self
            .client
            .get(url)
            .timeout(DOWNLOAD_TIMEOUT)
            .send()
            .await
            .map_err(|e| http_err("download", url, e))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(DolaError::Http(format!(
                "HTTP {} {host_path}",
                status.as_u16()
            )));
        }
        let part = part_path(out);
        let written = match stream_to_file(resp, url, &part).await {
            Ok(n) => n,
            Err(e) => {
                remove_part(&part).await;
                return Err(e);
            }
        };
        if written == 0 {
            remove_part(&part).await;
            return Err(DolaError::Http("empty body".to_string()));
        }
        tokio::fs::rename(&part, out)
            .await
            .map_err(|e| io_err("rename", out, &e))?;
        info!(host_path, bytes = written, out = %out.display(), "download xong");
        Ok(written)
    }
}

/// Body JSON cua `POST /im/chain/single` — `hub_browser.py:857-860` == `POLL_JS` `video_worker.py:37-55`
/// (`anchor_index = 2**53-1 = Number.MAX_SAFE_INTEGER`, SO nguyen). Thu tu khoa giu nhu Python
/// (`serde_json` `preserve_order` bat toan workspace tu lvc-core).
#[must_use]
pub fn poll_body(conv: &ConversationId, sequence_id: &str) -> Value {
    json!({
        "cmd": 3100,
        "uplink_body": { "pull_singe_chain_uplink_body": {
            "conversation_id": conv.0,
            "anchor_index": 9_007_199_254_740_991_u64,
            "conversation_type": 3,
            "direction": 1,
            "limit": 20,
            "ext": {},
            "filter": { "index_list": [] },
            "evaluate_ab_params": "",
            "evaluate_common_params": "",
        } },
        "sequence_id": sequence_id,
        "channel": 2,
        "version": "1",
    })
}

/// `video_worker.py:176` `dl_dir / f"{account}_{time.strftime('%Y%m%d_%H%M%S')}.mp4"` — `now` UTC
/// (ruling #4; Python gio may). `out_dir` = thu muc `downloads` cua dot (Task 11/17 quyet).
#[must_use]
pub fn output_path(out_dir: &Path, account: &str, now: SystemTime) -> PathBuf {
    out_dir.join(format!("{account}_{}.mp4", util::timestamp_utc_at(now)))
}

/// Mot header `Set-Cookie` -> `(ten, gia tri)` tai thoi diem hien tai — xem [`parse_set_cookie_at`].
#[must_use]
pub fn parse_set_cookie(line: &str) -> Option<(String, String)> {
    parse_set_cookie_at(line, SystemTime::now())
}

/// Mot header `Set-Cookie` -> `(ten, gia tri)`: phan truoc `;` dau tien, `name=value` (cat khoang
/// trang, bo dau `"` bao quanh nhu `http.cookies` Python); thieu `=`/ten rong -> `None`.
/// Het han = XOA cookie -> `None` (jar aiohttp khong giu -> `_cookies` Python giu gia tri CU; caller
/// o day cung vay vi khong nhan cap nao) theo RFC 6265 §5.2.1/§5.2.2/§5.3 buoc 5 (fix vong 1, I-2):
/// - `Max-Age` HOP LE (so nguyen) thang `Expires`: `<= 0` -> xoa; `> 0` -> giu. Khong hop le -> bo qua
///   thuoc tinh do (nhu khong co).
/// - Khong co `Max-Age` hop le: `Expires` parse duoc ([`util::cookie_date_unix`], RFC 1123 / RFC 850 /
///   asctime nhu trinh duyet) va `<= now` -> xoa; tuong lai -> giu; khong parse duoc -> bo qua thuoc
///   tinh (cookie phien -> giu).
/// - Lap lai thuoc tinh: lan HOP LE cuoi thang (RFC 6265 §5.3 "the last attribute").
///
/// `Domain`/`Path`/... bo qua (caller giu MOT chuoi Cookie cho `www.dola.com`).
///
/// Lech aiohttp (co chu y, deu theo RFC/trinh duyet — oracle jar aiohttp 3.14.3 cho cac ca khac):
/// aiohttp `if expire_time:` coi epoch 0 (`Thu, 01 Jan 1970 00:00:00 GMT` — idiom xoa pho bien) la
/// "khong parse duoc" -> GIU; va `Max-Age` khong hop le lam aiohttp bo luon `Expires` -> GIU. Ca hai
/// o day -> xoa (ruling fix vong 1: `Expires <= now` = xoa).
#[must_use]
pub fn parse_set_cookie_at(line: &str, now: SystemTime) -> Option<(String, String)> {
    let mut parts = line.split(';');
    let (name, value) = parts.next()?.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let value = value.trim();
    let value = value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value);
    let mut max_age: Option<i64> = None;
    let mut expires: Option<i64> = None;
    for attr in parts {
        let (k, v) = attr
            .split_once('=')
            .map_or((attr.trim(), ""), |(k, v)| (k.trim(), v.trim()));
        if k.eq_ignore_ascii_case("max-age") {
            if let Ok(n) = v.parse::<i64>() {
                max_age = Some(n);
            }
        } else if k.eq_ignore_ascii_case("expires") {
            if let Some(t) = util::cookie_date_unix(v) {
                expires = Some(t);
            }
        }
    }
    let is_expired = match (max_age, expires) {
        (Some(n), _) => n <= 0,
        (None, Some(t)) => t <= util::unix_secs(now),
        (None, None) => false,
    };
    if is_expired {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}

/// Moi `Set-Cookie` trong response (thu tu giu nguyen) -> [`parse_set_cookie`]; header khong phai
/// ASCII hien thi -> bo qua.
fn set_cookie_pairs(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(parse_set_cookie)
        .collect()
}

/// `host/path` cua URL (bo scheme, query, fragment) — dung cho log/thong diep loi vi URL CDN mang
/// `x-signature`/`key_seed` (chinh sach: khong log URL day du).
#[must_use]
pub fn url_host_path(url: &str) -> String {
    let rest = url.find("://").map_or(url, |i| &url[i + 3..]);
    let end = rest.find(['?', '#']).unwrap_or(rest.len());
    rest[..end].to_string()
}

/// `<out>.part` — cung thu muc, them duoi (`acc_x.mp4` -> `acc_x.mp4.part`).
fn part_path(out: &Path) -> PathBuf {
    let mut s = out.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

async fn remove_part(part: &Path) {
    if let Err(e) = tokio::fs::remove_file(part).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(part = %part.display(), error = %e, "khong xoa duoc .part");
        }
    }
}

/// Stream body -> file `part` (`iter_chunked(1<<16)` `:185`: reqwest tra chunk theo hyper, kich thuoc
/// khong co dinh — chi la buffer, khong anh huong noi dung). Tra so byte ghi. Loi -> caller xoa `part`.
async fn stream_to_file(resp: reqwest::Response, url: &str, part: &Path) -> Result<u64, DolaError> {
    let mut file = tokio::fs::File::create(part)
        .await
        .map_err(|e| io_err("create", part, &e))?;
    let mut stream = resp.bytes_stream();
    let mut written: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| http_err("download", url, e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| io_err("write", part, &e))?;
        written += chunk.len() as u64;
    }
    file.flush().await.map_err(|e| io_err("flush", part, &e))?;
    drop(file); // dong TRUOC rename (Windows khong rename duoc file dang mo)
    Ok(written)
}

/// `reqwest::Error` -> `DolaError::Http("<what> <host/path> (<loai>): <chuoi nguyen nhan>")` —
/// `without_url()` de thong diep KHONG chua URL day du (`Display` cua reqwest in ` for url (...)`
/// ke ca query co chu ky); loai loi tinh TRUOC khi bo URL (`is_timeout` doc chain `source()`).
fn http_err(what: &str, url: &str, e: reqwest::Error) -> DolaError {
    let kind = if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_redirect() {
        "redirect"
    } else if e.is_body() || e.is_decode() {
        "body"
    } else {
        "request"
    };
    let e = e.without_url();
    DolaError::Http(format!(
        "{what} {} ({kind}): {}",
        url_host_path(url),
        error_chain(&e)
    ))
}

/// Chuoi `Display` + chain `source()` cua mot loi bat ky (dung cho loi builder/proxy — khong co URL ky).
fn error_chain(e: &(dyn StdError + 'static)) -> String {
    let mut s = e.to_string();
    let mut cur = e.source();
    while let Some(src) = cur {
        s.push_str(": ");
        s.push_str(&src.to_string());
        cur = src.source();
    }
    s
}

fn io_err(what: &str, path: &Path, e: &std::io::Error) -> DolaError {
    DolaError::Http(format!("io: {what} {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as B64;
    use serde_json::{Value, json};
    use tempfile::tempdir;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const UA: &str = "UA-TEST/1.0 (Chromium 140 that)";
    const COOKIE: &str = "sessionid=s1; msToken=old";

    fn http() -> DolaHttp {
        DolaHttp::new(UA.to_string(), None).expect("client")
    }

    fn conv() -> ConversationId {
        ConversationId("123456789".to_string())
    }

    /// Fixture T2 (`lvc-core::chain` test `latest_la_tin_bot_moi_nhat_khong_phai_messages_0`):
    /// 1 creation `block_type 2074` type 2 -> `videos.len() == 1`.
    fn chain_fixture() -> Value {
        json!({
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
        })
    }

    fn is_uuid4(s: &str) -> bool {
        s.len() == 36
            && s.split('-').map(str::len).collect::<Vec<_>>() == [8, 4, 4, 4, 12]
            && s.chars().all(|c| c == '-' || c.is_ascii_hexdigit())
            && s.as_bytes()[14] == b'4'
    }

    fn header<'a>(req: &'a wiremock::Request, name: &str) -> Option<&'a str> {
        req.headers.get(name).and_then(|v| v.to_str().ok())
    }

    async fn only_request(server: &MockServer) -> wiremock::Request {
        let mut reqs = server.received_requests().await.expect("recording bat");
        assert_eq!(reqs.len(), 1, "dung 1 request");
        reqs.remove(0)
    }

    // ===== pull_chain (hub_browser.py:853-873) =====

    #[tokio::test]
    async fn pull_chain_gui_dung_header_va_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/im/chain/single"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chain_fixture())
                    .insert_header("set-cookie", "msToken=abc; Path=/; Domain=.dola.com"),
            )
            .mount(&server)
            .await;
        let url = format!("{}/im/chain/single", server.uri());

        let out = http()
            .pull_chain_at(COOKIE, &conv(), &url)
            .await
            .expect("pull_chain");
        assert!(out.poll.ok);
        assert_eq!(out.poll.status, 200);
        assert_eq!(out.poll.videos.len(), 1);
        assert_eq!(out.poll.videos[0], "http://v16-dola.dola.com/video/abc.mp4");
        assert_eq!(out.poll.latest, vec!["Đây là video của bạn".to_string()]);
        assert_eq!(
            out.set_cookies,
            vec![("msToken".to_string(), "abc".to_string())]
        );

        let req = only_request(&server).await;
        // Header phien (hub_browser.py:820-822) + header rieng poll (:861)
        assert_eq!(header(&req, "cookie"), Some(COOKIE));
        assert_eq!(header(&req, "agw-js-conv"), Some("str"));
        assert_eq!(header(&req, "origin"), Some("https://www.dola.com"));
        assert_eq!(
            header(&req, "referer"),
            Some("https://www.dola.com/chat/123456789")
        );
        assert_eq!(
            header(&req, "user-agent"),
            Some(UA),
            "UA THAT (R2), khong POLL_UA"
        );
        assert_eq!(
            header(&req, "content-type"),
            Some("application/json; encoding=utf-8")
        );
        assert_eq!(header(&req, "accept"), Some("*/*"));
        assert_eq!(header(&req, "accept-language"), Some("ja-JP,ja;q=0.9"));

        // Body = POLL_JS / hub_browser.py:856-860
        let body: Value = req.body_json().expect("body JSON");
        assert_eq!(body["cmd"], json!(3100));
        let up = &body["uplink_body"]["pull_singe_chain_uplink_body"];
        assert_eq!(up["conversation_id"], json!("123456789"));
        assert_eq!(up["anchor_index"], json!(9_007_199_254_740_991_u64));
        assert!(up["anchor_index"].is_u64(), "anchor_index phai la SO");
        assert_eq!(up["conversation_type"], json!(3));
        assert_eq!(up["direction"], json!(1));
        assert_eq!(up["limit"], json!(20));
        assert_eq!(up["ext"], json!({}));
        assert_eq!(up["filter"], json!({"index_list": []}));
        assert_eq!(up["evaluate_ab_params"], json!(""));
        assert_eq!(up["evaluate_common_params"], json!(""));
        assert!(
            is_uuid4(body["sequence_id"].as_str().unwrap_or("")),
            "{body}"
        );
        assert_eq!(body["channel"], json!(2));
        assert_eq!(body["version"], json!("1"));

        // Query = POLL_PARAMS + web_tab_id=uuid4 (:856)
        let pairs: Vec<(String, String)> = req
            .url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        for (k, v) in POLL_PARAMS {
            assert!(
                pairs.iter().any(|(pk, pv)| pk == k && pv == v),
                "thieu {k}={v} trong {pairs:?}"
            );
        }
        let tab = pairs
            .iter()
            .find(|(k, _)| k == "web_tab_id")
            .map_or("", |(_, v)| v.as_str());
        assert!(is_uuid4(tab), "web_tab_id={tab:?}");
        assert_eq!(pairs.len(), POLL_PARAMS.len() + 1);
    }

    #[tokio::test]
    async fn pull_chain_status_712012001_tra_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status_code": 712_012_001, "status_desc": "登录校验不合法"
            })))
            .mount(&server)
            .await;
        let url = format!("{}/im/chain/single", server.uri());
        let out = http().pull_chain_at(COOKIE, &conv(), &url).await.unwrap();
        assert!(!out.poll.ok);
        assert_eq!(out.poll.status, 401);
        assert!(out.poll.videos.is_empty());
        assert!(out.poll.texts.is_empty());
    }

    #[tokio::test]
    async fn pull_chain_status_code_khac_tra_500() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"status_code": 5, "status_desc": "x"})),
            )
            .mount(&server)
            .await;
        let url = format!("{}/im/chain/single", server.uri());
        let out = http().pull_chain_at(COOKIE, &conv(), &url).await.unwrap();
        assert!(!out.poll.ok);
        assert_eq!(out.poll.status, 500);
    }

    #[tokio::test]
    async fn pull_chain_http_500_ok_false() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;
        let url = format!("{}/im/chain/single", server.uri());
        let out = http().pull_chain_at(COOKIE, &conv(), &url).await.unwrap();
        assert_eq!(
            out.poll,
            ChainPoll {
                ok: false,
                status: 500,
                ..ChainPoll::default()
            }
        );
    }

    #[tokio::test]
    async fn pull_chain_body_khong_phai_json_loi_http() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>"))
            .mount(&server)
            .await;
        let url = format!("{}/im/chain/single", server.uri());
        let err = http()
            .pull_chain_at(COOKIE, &conv(), &url)
            .await
            .unwrap_err();
        assert!(matches!(err, DolaError::Http(_)), "{err}");
    }

    /// `for c in s.cookie_jar: self._cookies[c.key] = c.value` (:867-868): moi Set-Cookie -> cap
    /// (ten, gia tri), theo thu tu; `Max-Age=0` / `Expires` qua khu = xoa -> bo qua (jar aiohttp
    /// khong giu; fix vong 1 them `Expires`), `Expires` tuong lai -> giu.
    #[tokio::test]
    async fn pull_chain_gom_nhieu_set_cookie() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({}))
                    .append_header("set-cookie", "msToken=t1; Max-Age=86400; Path=/")
                    .append_header("set-cookie", "ttwid=\"quoted\"; Path=/; HttpOnly")
                    .append_header("set-cookie", "gone=; Max-Age=0; Path=/")
                    .append_header(
                        "set-cookie",
                        "dead=; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/",
                    )
                    .append_header(
                        "set-cookie",
                        "alive=v; Expires=Mon, 01 Jan 2035 00:00:00 GMT; Path=/",
                    ),
            )
            .mount(&server)
            .await;
        let url = format!("{}/im/chain/single", server.uri());
        let out = http().pull_chain_at(COOKIE, &conv(), &url).await.unwrap();
        assert!(out.poll.ok);
        assert!(out.poll.videos.is_empty());
        assert_eq!(
            out.set_cookies,
            vec![
                ("msToken".to_string(), "t1".to_string()),
                ("ttwid".to_string(), "quoted".to_string()),
                ("alive".to_string(), "v".to_string()),
            ]
        );
    }

    /// Fix vong 1 (I-1): `Debug` che GIA TRI cookie — chi in ten + `***`.
    #[test]
    fn pull_outcome_debug_che_gia_tri_cookie() {
        let out = PullOutcome {
            poll: ChainPoll {
                ok: true,
                status: 200,
                ..ChainPoll::default()
            },
            set_cookies: vec![
                ("msToken".to_string(), "SECRETVAL1".to_string()),
                ("sessionid".to_string(), "SECRETVAL2".to_string()),
            ],
        };
        let s = format!("{out:?}");
        assert!(s.contains("PullOutcome"), "{s}");
        assert!(s.contains("ok: true"), "{s}");
        assert!(s.contains("(\"msToken\", \"***\")"), "{s}");
        assert!(s.contains("(\"sessionid\", \"***\")"), "{s}");
        assert!(!s.contains("SECRETVAL"), "lo gia tri cookie: {s}");
        let alt = format!("{out:#?}");
        assert!(
            !alt.contains("SECRETVAL"),
            "lo gia tri cookie ({{:#?}}): {alt}"
        );
    }

    #[tokio::test]
    async fn pull_chain_may_chu_khong_ton_tai_loi_http_khong_lo_query() {
        // Cong 9 (discard) — khong co gi nghe -> loi ket noi.
        let err = http()
            .pull_chain_at(COOKIE, &conv(), "http://127.0.0.1:9/im/chain/single")
            .await
            .unwrap_err();
        let s = err.to_string();
        assert!(s.starts_with("HTTP: "), "{s}");
        assert!(s.contains("127.0.0.1:9/im/chain/single"), "{s}");
        assert!(!s.contains("web_tab_id"), "khong lo query: {s}");
    }

    // ===== fetch_raw_video (unwatermark.py:225-240) =====

    #[tokio::test]
    async fn fetch_raw_video_chon_bitrate_cao() {
        let low = B64.encode("https://low/720p.mp4");
        let high = B64.encode("https://high/1080p.mp4");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/video/fplay/1/sig/vid"))
            .and(query_param("channel", "no"))
            .and(query_param("codec_type", "8"))
            .and(query_param("logo_type", "unwatermarked"))
            .and(query_param("key_seed", "abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "video_info": { "data": { "video_list": {
                    "v1": {"main_url": low, "bitrate": 500, "vwidth": 640, "vheight": 480},
                    "v2": {"main_url": high, "bitrate": 2000, "vwidth": 1280, "vheight": 720}
                } } }
            })))
            .mount(&server)
            .await;
        // channel=yes bi RAW_PARAMS ghi de; key_seed giu nguyen (replace_query).
        let api = format!(
            "{}/video/fplay/1/sig/vid?channel=yes&key_seed=abc",
            server.uri()
        );
        let got = http().fetch_raw_video(&api, FETCH_TIMEOUT).await.unwrap();
        assert_eq!(got.as_deref(), Some("https://high/1080p.mp4"));

        let req = only_request(&server).await;
        assert!(header(&req, "cookie").is_none(), "KHONG cookie");
        assert_eq!(
            header(&req, "accept"),
            Some("application/json,text/plain,*/*")
        );
        assert_eq!(header(&req, "user-agent"), Some(UA));
        assert_eq!(
            req.url.query(),
            Some("channel=no&key_seed=abc&codec_type=8&logo_type=unwatermarked")
        );
    }

    #[tokio::test]
    async fn fetch_raw_video_http_404_tra_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let api = format!("{}/video/x", server.uri());
        let got = http().fetch_raw_video(&api, FETCH_TIMEOUT).await.unwrap();
        assert_eq!(got, None);
    }

    #[tokio::test]
    async fn fetch_raw_video_json_hong_tra_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("nope"))
            .mount(&server)
            .await;
        let api = format!("{}/video/x", server.uri());
        let got = http().fetch_raw_video(&api, FETCH_TIMEOUT).await.unwrap();
        assert_eq!(got, None);
    }

    // ===== download (video_worker.py:170-187) =====

    #[tokio::test]
    async fn download_stream_ghi_dung_byte() {
        let body: Vec<u8> = (0..200u32 * 1024)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/video/abc.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let out = dir.path().join("sub").join("dir").join("acc_x.mp4");
        let url = format!("{}/video/abc.mp4?x-signature=SECRET", server.uri());

        let n = http().download(&url, &out).await.expect("download");
        assert_eq!(n, 200 * 1024);
        assert_eq!(std::fs::read(&out).unwrap(), body);
        let part = dir.path().join("sub").join("dir").join("acc_x.mp4.part");
        assert!(!part.exists(), ".part khong con");

        let req = only_request(&server).await;
        assert!(header(&req, "cookie").is_none(), "KHONG cookie");
        assert!(header(&req, "referer").is_none(), "KHONG referer");
        assert_eq!(
            header(&req, "user-agent"),
            Some(UA),
            "ruling: cung UA trinh duyet"
        );
    }

    #[tokio::test]
    async fn download_404_loi_khong_de_lai_part() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404).set_body_string("nf"))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let out = dir.path().join("v.mp4");
        let url = format!("{}/video/abc.mp4?x-signature=SECRET", server.uri());
        let err = http().download(&url, &out).await.unwrap_err();
        let s = err.to_string();
        assert!(matches!(err, DolaError::Http(_)), "{s}");
        assert!(s.contains("HTTP 404"), "{s}");
        assert!(s.contains("/video/abc.mp4"), "{s}");
        assert!(!s.contains("SECRET"), "khong lo query ky: {s}");
        assert!(!out.exists());
        assert!(!dir.path().join("v.mp4.part").exists());
    }

    #[tokio::test]
    async fn download_body_rong_loi() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let dir = tempdir().unwrap();
        let out = dir.path().join("v.mp4");
        let url = format!("{}/video/abc.mp4", server.uri());
        let err = http().download(&url, &out).await.unwrap_err();
        assert_eq!(err.to_string(), "HTTP: empty body");
        assert!(!out.exists());
        assert!(!dir.path().join("v.mp4.part").exists());
    }

    /// Proxy HTTP co xac thuc: reqwest gui request dang tuyet doi toi proxy kem
    /// `Proxy-Authorization: Basic` (aiohttp `proxy=server, proxy_auth=BasicAuth(u, p)`).
    #[tokio::test]
    async fn download_qua_proxy_http_gui_proxy_authorization() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/f.mp4"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![7u8; 10]))
            .mount(&server)
            .await;
        let addr = *server.address();
        let spec = ProxySpec {
            scheme: "http".into(),
            host: addr.ip().to_string(),
            port: addr.port(),
            username: Some("u".into()),
            password: Some("p".into()),
        };
        let http = DolaHttp::new(UA.to_string(), Some(spec)).unwrap();
        let dir = tempdir().unwrap();
        let out = dir.path().join("p.mp4");
        // Dich la cong 9 (khong ai nghe): chi toi duoc neu di qua proxy = mock server.
        let n = http
            .download("http://127.0.0.1:9/f.mp4", &out)
            .await
            .unwrap();
        assert_eq!(n, 10);
        let req = only_request(&server).await;
        assert_eq!(req.url.as_str(), "http://127.0.0.1:9/f.mp4");
        let auth = header(&req, "proxy-authorization").unwrap_or("");
        assert_eq!(auth, format!("Basic {}", B64.encode("u:p")));
    }

    #[test]
    fn new_chap_nhan_socks5_va_khong_proxy() {
        let spec = ProxySpec {
            scheme: "socks5".into(),
            host: "127.0.0.1".into(),
            port: 1080,
            username: None,
            password: None,
        };
        let h = DolaHttp::new("ua".into(), Some(spec.clone())).unwrap();
        assert_eq!(h.user_agent(), "ua");
        assert_eq!(h.proxy(), Some(&spec));
        assert!(DolaHttp::new("ua".into(), None).unwrap().proxy().is_none());
    }

    // ===== ham thuan =====

    /// `downloads/<account>_<YYYYmmdd_HHMMSS>.mp4` (`video_worker.py:176`) — UTC (ruling #4).
    #[test]
    fn output_path_dinh_dang() {
        let re = regex::Regex::new(r"^acc_\d{8}_\d{6}\.mp4$").unwrap();
        let p = output_path(Path::new("downloads"), "acc", SystemTime::now());
        let name = p.file_name().unwrap().to_str().unwrap();
        assert!(re.is_match(name), "{name}");
        assert_eq!(p.parent(), Some(Path::new("downloads")));
        let fixed = UNIX_EPOCH + Duration::from_secs(1_789_693_323);
        assert_eq!(
            output_path(Path::new("d"), "acc", fixed),
            Path::new("d").join("acc_20260918_010203.mp4")
        );
    }

    #[test]
    fn parse_set_cookie_cac_dang() {
        assert_eq!(
            parse_set_cookie("msToken=abc; Path=/; Domain=.dola.com"),
            Some(("msToken".into(), "abc".into()))
        );
        assert_eq!(
            parse_set_cookie("a=\"q v\"; HttpOnly"),
            Some(("a".into(), "q v".into()))
        );
        assert_eq!(parse_set_cookie(" b = 1 "), Some(("b".into(), "1".into())));
        assert_eq!(
            parse_set_cookie("c=; Path=/"),
            Some(("c".into(), String::new()))
        );
        assert_eq!(parse_set_cookie("d=x; Max-Age=0"), None);
        assert_eq!(parse_set_cookie("d=x; max-age=-5"), None);
        assert_eq!(
            parse_set_cookie("d=x; Max-Age=10"),
            Some(("d".into(), "x".into()))
        );
        assert_eq!(parse_set_cookie("novalue"), None);
        assert_eq!(parse_set_cookie("=v"), None);
        assert_eq!(parse_set_cookie(""), None);
    }

    /// Fix vong 1 (I-2): `Expires` <= now = xoa (bo qua, caller giu gia tri cu); tuong lai = upsert;
    /// khong parse duoc = cookie phien (upsert); `Max-Age` HOP LE thang `Expires` (RFC 6265 §5.3
    /// buoc 5); `Max-Age` khong hop le bi bo qua -> xet `Expires`; nhieu `Expires`/`Max-Age`: lan
    /// HOP LE cuoi thang. Oracle jar aiohttp 3.14.3 (`update_cookies` roi liet ke) cho cac dong
    /// khong danh dau; 2 dong danh dau `(lech aiohttp)` theo RFC/trinh duyet — xem doc ham.
    #[test]
    fn parse_set_cookie_expires_theo_now() {
        // now = 2026-09-18 01:02:03 UTC
        let now = UNIX_EPOCH + Duration::from_secs(1_789_693_323);
        let at = |line: &str| parse_set_cookie_at(line, now);
        let keep = Some(("a".to_string(), "NEW".to_string()));
        assert_eq!(at("a=NEW; Expires=Wed, 21 Oct 2015 07:28:00 GMT"), None);
        assert_eq!(at("a=NEW; Expires=Mon, 01 Jan 2035 00:00:00 GMT"), keep);
        assert_eq!(
            at("a=NEW; Expires=Fri, 18 Sep 2026 01:02:03 GMT"),
            None,
            "== now"
        );
        assert_eq!(
            at("a=NEW; Expires=Fri, 18 Sep 2026 01:02:04 GMT"),
            keep,
            "now + 1"
        );
        // (lech aiohttp: `if expire_time:` coi 0 la falsy -> giu; RFC/trinh duyet: qua khu -> xoa)
        assert_eq!(at("a=NEW; Expires=Thu, 01 Jan 1970 00:00:00 GMT"), None);
        assert_eq!(at("a=NEW; Expires=Thu, 31 Dec 1969 23:59:59 GMT"), None);
        assert_eq!(
            at("a=NEW; Expires=garbage"),
            keep,
            "khong parse duoc = cookie phien"
        );
        assert_eq!(at("a=NEW; Expires="), keep);
        assert_eq!(at("a=NEW; Expires"), keep);
        // Ba dinh dang qua khu
        assert_eq!(at("a=NEW; Expires=Sunday, 06-Nov-94 08:49:37 GMT"), None);
        assert_eq!(at("a=NEW; Expires=Sun Nov  6 08:49:37 1994"), None);
        assert_eq!(
            at("a=NEW; expires=Sun, 06 Nov 1994 08:49:37 GMT; Path=/"),
            None
        );
        // Max-Age thang Expires
        assert_eq!(
            at("a=NEW; Max-Age=10; Expires=Wed, 21 Oct 2015 07:28:00 GMT"),
            keep
        );
        assert_eq!(
            at("a=NEW; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Max-Age=10"),
            keep
        );
        assert_eq!(
            at("a=NEW; Max-Age=0; Expires=Mon, 01 Jan 2035 00:00:00 GMT"),
            None
        );
        // (lech aiohttp: Max-Age khong hop le -> aiohttp bo luon Expires -> giu; RFC §5.2.2 bo
        // thuoc tinh do -> xet Expires -> xoa)
        assert_eq!(
            at("a=NEW; Max-Age=abc; Expires=Wed, 21 Oct 2015 07:28:00 GMT"),
            None
        );
        assert_eq!(at("a=NEW; Max-Age=abc"), keep);
        assert_eq!(at("a=NEW; Max-Age="), keep);
        // Lan hop le cuoi thang
        assert_eq!(
            at(
                "a=NEW; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Expires=Mon, 01 Jan 2035 00:00:00 GMT"
            ),
            keep
        );
        assert_eq!(
            at(
                "a=NEW; Expires=Mon, 01 Jan 2035 00:00:00 GMT; Expires=Wed, 21 Oct 2015 07:28:00 GMT"
            ),
            None
        );
        assert_eq!(
            at("a=NEW; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Expires=garbage"),
            None
        );
        assert_eq!(at("a=NEW; Max-Age=0; Max-Age=5"), keep);
        assert_eq!(at("a=NEW; Max-Age=5; Max-Age=0"), None);
        assert_eq!(at("a=NEW; Max-Age=5; Max-Age=abc"), keep);
        // Gia tri rong + Expires qua khu (idiom xoa pho bien) -> bo qua, KHONG ghi de bang chuoi rong
        assert_eq!(
            at("a=; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Path=/"),
            None
        );
        assert_eq!(at("a=NEW"), keep);
    }

    #[test]
    fn url_host_path_bo_query_va_fragment() {
        assert_eq!(
            url_host_path("https://v16.cdn.com/video/abc.mp4?x-signature=S&k=v#f"),
            "v16.cdn.com/video/abc.mp4"
        );
        assert_eq!(
            url_host_path("http://127.0.0.1:9/f.mp4"),
            "127.0.0.1:9/f.mp4"
        );
        assert_eq!(url_host_path("https://host"), "host");
        assert_eq!(url_host_path("garbage?x=1"), "garbage");
    }

    #[test]
    fn poll_body_dung_hinh_dang_poll_js() {
        let b = poll_body(&conv(), "seq-1");
        let want = json!({
            "cmd": 3100,
            "uplink_body": { "pull_singe_chain_uplink_body": {
                "conversation_id": "123456789",
                "anchor_index": 9_007_199_254_740_991_u64,
                "conversation_type": 3, "direction": 1, "limit": 20,
                "ext": {}, "filter": {"index_list": []},
                "evaluate_ab_params": "", "evaluate_common_params": ""
            } },
            "sequence_id": "seq-1", "channel": 2, "version": "1"
        });
        assert_eq!(b, want);
        // Thu tu khoa nhu Python dict (serde_json preserve_order bat o lvc-core).
        let s = b.to_string();
        assert!(s.starts_with("{\"cmd\":3100,\"uplink_body\":"), "{s}");
        assert!(s.contains("\"anchor_index\":9007199254740991,"), "{s}");
    }

    #[test]
    fn hang_so_theo_python() {
        assert_eq!(POLL_URL, "https://www.dola.com/im/chain/single");
        assert_eq!(DOLA_ORIGIN, "https://www.dola.com");
        assert_eq!(POLL_TIMEOUT.as_secs(), 40); // hub_browser.py:820
        assert_eq!(FETCH_TIMEOUT.as_secs(), 30); // unwatermark.py:27
        assert_eq!(DOWNLOAD_TIMEOUT.as_secs(), 300); // video_worker.py:177
        assert_eq!(POLL_PARAMS.len(), 14);
        assert_eq!(POLL_PARAMS[0], ("version_code", "20800"));
        assert_eq!(POLL_PARAMS[13], ("use-olympus-account", "1"));
        assert!(POLL_UA.contains("Chrome/153.0.0.0"));
    }

    /// So byte `POLL_UA` + tung cap `POLL_PARAMS` (dung THU TU) voi nguon Python
    /// (`hub_browser.py:747-750`); file vang -> bo qua (nhu `labels`).
    #[test]
    fn poll_params_va_ua_chep_nguyen_van_python() {
        use crate::dola::py_src::{python_source, same_bytes};
        let Some(src) = python_source(r"src\browser\hub_browser.py") else {
            return;
        };
        assert!(same_bytes(&src, "POLL_UA", POLL_UA));
        let start = src.find("\nPOLL_PARAMS = {").expect("POLL_PARAMS") + 1;
        let end = src[start..].find('}').expect("dong ngoac") + start;
        let re = regex::Regex::new(r#""([^"]+)":\s*"([^"]*)""#).unwrap();
        let py: Vec<(String, String)> = re
            .captures_iter(&src[start..end])
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .collect();
        let rust: Vec<(String, String)> = POLL_PARAMS
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        assert_eq!(rust, py);
    }
}
