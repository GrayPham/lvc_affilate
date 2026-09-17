//! Gui prompt Dola -> `conversation_id` (Task 9, P2) — port `video_worker_ui.py` (HEAD 3d07c19):
//!
//! | Rust | Python |
//! |---|---|
//! | [`precheck_duration`] | `generate_video` `:2399-2403` (`ValueError` -> `DURATION_UNAVAILABLE`) |
//! | [`submit_timeout`] | `:2398` `video_timeout(...)` + `:2404-2405` (`lvc_core::video_timeout`) |
//! | [`submit`] / [`submit_at`] | `generate_video` `:2444-2494` (chat MOI — R5; bo balance `:2473` — R7) |
//! | [`open_new_chat`] | `:2461-2469` (goto `/chat` + 5 s, `assert_logged_in`) |
//! | — `known` | `:2472` `known_conv_ids = set(await recent_conv_ids(...))` |
//! | — `select_mode` | `:2475-2485` (`target_mode`; Agent nuot loi — Task 12) |
//! | [`submit_fast`] | `_submit_fast` `:2264-2301` |
//! | [`select_duration_or_hook`] | `:2274-2288` (15/30 khong co tren UI -> 10 s + hook ADR-044) |
//! | [`hook_not_patched_msg`] | `:2296-2299` |
//! | [`after_submit`] | `_after_submit` `:2533-2600` (captcha + conv id; KHONG poll — Task 11) |
//! | [`has_captcha`] | `find_captcha_frame` `:692-696` |
//! | [`first_fresh`] / [`is_recent_tick`] | `pick_new_conv_id(tail, known, recent)` `:678-689`, `:2578` |
//! | [`no_conversation_msg`] | `:2600` |
//!
//! # Khac Python (ruling R3 — captcha song song voi cho conv id)
//! Python `_after_submit` cho **tuan tu**: 20 x 1 s tim iframe `bdcaptcha.html` (`:2540-2548`, roi
//! `solve_slider` 3 lan `:2549-2561`) — het 20 s khong thay moi sang vong 30 x 1 s doc URL lay
//! conversation id (`:2563-2600`) => luon mat >= 20 s. O day chi co **MOT** vong
//! `Limits::no_conversation_timeout_s` (30) x 1 s; MOI nhip: (a) frame nao chua `bdcaptcha.html` ->
//! [`DolaError::RiskControl`] ngay (R6: khong giai slider o P2, pool cooldown 1800 s); (b) doc URL
//! nhu Python. Ket qua: khong captcha -> co id sau ~1-2 s thay vi ~21 s; co captcha -> bao trong 1 s.
//! Cai gia: captcha xuat hien SAU khi da co conversation id thi Task 11 (poll) moi thay — Python
//! cung vay sau 20 s.
//!
//! # Khac Python — dieu huong (ruling #5)
//! Python `:2461-2464`: `is_reused and page.url == chat_url` -> 1 s; khong thi `goto` + (3 s neu reused
//! / 5 s). P2 khong co warm session nen luon coi la KHONG reused (5 s sau goto); nhung URL da la
//! `/chat` (bo query/`/` cuoi) thi chi cho 1 s nhu nhanh reused — trang da mo san.
//!
//! Log qua `tracing`; KHONG BAO GIO log noi dung prompt (chi so ky tu) hay cookie. `SubmitRequest`
//! tu viet `Debug` de che prompt.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::ScreenshotParams;
use lvc_core::{ConvPick, Limits, Mode, Model, Ratio, pick_new_conv_id, video_timeout};

use super::compose;
use super::error::DolaError;
use super::labels::{CAPTCHA_FRAME_KEY, DOLA_CHAT_URL};
use super::refs;
use super::session;
use crate::BrowserError;
use crate::ops;

/// `:2462` `wait_for_timeout(1000)` — trang da o `/chat`.
const CHAT_ALREADY_OPEN_SETTLE: Duration = Duration::from_secs(1);
/// `:2465` `wait_for_timeout(3000 if is_reused else 5000)` — P2 luon "khong reused" (xem doc module).
const CHAT_GOTO_SETTLE: Duration = Duration::from_secs(5);
/// `:2566` `wait_for_timeout(1000)` moi nhip cho conversation id.
const CONV_TICK: Duration = Duration::from_secs(1);
/// `:2583` `wait_for_timeout(2000)` sau khi mo lai dung chat moi.
const CONV_JUMP_SETTLE: Duration = Duration::from_secs(2);
/// `:2591` `Path("logs")`.
const DEFAULT_LOGS_DIR: &str = "logs";
/// `:2600` `page.url.split('?')[0][:80]`.
const URL_SNIPPET: usize = 80;
/// `:2594-2595` `except Exception: shot = "(khong chup duoc)"`.
const SHOT_FAILED: &str = "(khong chup duoc)";

/// Yeu cau gui mot video. `Debug` tu viet: in `prompt_chars` thay cho `prompt`.
#[derive(Clone)]
pub struct SubmitRequest {
    pub account: String,
    pub prompt: String,
    pub mode: Mode,
    pub ratio: Option<Ratio>,
    pub duration: u16,
    pub model: Model,
    pub reference_images: Vec<PathBuf>,
}

impl fmt::Debug for SubmitRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubmitRequest")
            .field("account", &self.account)
            .field("prompt_chars", &self.prompt.chars().count())
            .field("mode", &self.mode)
            .field("ratio", &self.ratio)
            .field("duration", &self.duration)
            .field("model", &self.model)
            .field("reference_images", &self.reference_images)
            .finish()
    }
}

/// Id hoi thoai Dola (chuoi toan chu so trong URL `/chat/<id>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConversationId(pub String);

impl fmt::Display for ConversationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Ngu canh gui: trang da mo (mot Chrome/tai khoan), gioi han cau hinh, thu muc ghi anh chan doan
/// (`no_conv_<account>_<ts>.png`; ruling: cau hinh duoc, mac dinh `logs` nhu Python `:2591`).
pub struct SubmitCtx<'a> {
    pub page: &'a Page,
    pub limits: &'a Limits,
    pub logs_dir: PathBuf,
}

impl<'a> SubmitCtx<'a> {
    /// `logs_dir` mac dinh `logs` (tuong doi thu muc lam viec, nhu Python).
    #[must_use]
    pub fn new(page: &'a Page, limits: &'a Limits) -> Self {
        Self {
            page,
            limits,
            logs_dir: default_logs_dir(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Ham thuan (co unit test)
// ---------------------------------------------------------------------------------------------

fn default_logs_dir() -> PathBuf {
    PathBuf::from(DEFAULT_LOGS_DIR)
}

/// Timeout cho video cua yeu cau nay — `lvc_core::video_timeout(mode, duration, co anh, limits)`
/// (`:2398` + `:2404-2405`). Chon timeout la viec cua caller (Task 11/17); `submit` khong dung.
#[must_use]
pub fn submit_timeout(req: &SubmitRequest, limits: &Limits) -> Duration {
    Duration::from_secs(video_timeout(
        req.mode,
        req.duration,
        !req.reference_images.is_empty(),
        limits,
    ))
}

/// Kiem tra thuan TRUOC moi I/O (`:2399-2403`): `duration` phai thuoc
/// `Mode::allowed_durations(model)` (ADR-044). Thong diep = `ValueError` cua Python: ngoai
/// {10, 15, 30} -> "Dola supports durations of 10s, 15s and 30s"; 30 s ma khong phai Nhanh + 2.5 ->
/// "30s is only available with mode=fast and model seedance-2.5 (...)".
///
/// # Errors
/// [`DolaError::DurationUnavailable`].
pub fn precheck_duration(mode: Mode, model: Model, duration: u16) -> Result<(), DolaError> {
    if mode.allowed_durations(model).contains(&duration) {
        return Ok(());
    }
    let msg = if duration == 30 {
        "30s is only available with mode=fast and model seedance-2.5 \
         (Dola caps agent/pro and Seedance 2.0 at 15s)"
    } else {
        "Dola supports durations of 10s, 15s and 30s"
    };
    Err(DolaError::DurationUnavailable(msg.to_string()))
}

/// Bo `#fragment`, `?query` va `/` cuoi de so URL.
fn url_path_only(url: &str) -> &str {
    let no_frag = url.split('#').next().unwrap_or("");
    let no_query = no_frag.split('?').next().unwrap_or("");
    no_query.trim_end_matches('/')
}

/// `:2461` `page.url.rstrip("/") == chat_url` — ruling #5: bo query/fragment va `/` cuoi o ca hai ve.
#[must_use]
pub fn is_chat_url(current: &str, chat_url: &str) -> bool {
    let cur = url_path_only(current);
    !cur.is_empty() && cur == url_path_only(chat_url)
}

/// `:2567` `page.url.rstrip("/").split("/")[-1]` — giu nguyen query nhu Python (khong phai so ->
/// `ConvPick::None`).
#[must_use]
pub fn url_tail(url: &str) -> &str {
    url.trim_end_matches('/').rsplit('/').next().unwrap_or("")
}

/// `:2582` `f"https://www.dola.com/chat/{conv_id}"` tren `chat_url` bat ky (fixture cuc bo).
fn conv_url(chat_url: &str, id: &str) -> String {
    format!("{}/{id}", url_path_only(chat_url))
}

/// `:2600` phan sau ma `NO_CONVERSATION: ` — `ticks` = so giay da cho (30 mac dinh -> khop Python
/// tung byte); `url.split('?')[0][:80]`.
fn no_conversation_msg(ticks: u32, url: &str, shot: &str) -> String {
    let url_no_query: String = url
        .split('?')
        .next()
        .unwrap_or("")
        .chars()
        .take(URL_SNIPPET)
        .collect();
    format!("Dola khong tao hoi thoai sau {ticks}s (url={url_no_query}, anh={shot})")
}

/// `:2296-2299` phan sau ma `DURATION_UNAVAILABLE: ` (`{state!r}`).
fn hook_not_patched_msg(state: &str) -> String {
    format!(
        "hook không đổi được duration trong body /chat/completion (state={}) — \
         Dola có thể đã đổi định dạng request; video sẽ là 10s nếu tiếp tục",
        lvc_core::error::py_repr(state)
    )
}

/// R6: chi phat hien, khong giai; pool cooldown 1800 s (`rotates_account`).
fn captcha_error() -> DolaError {
    DolaError::RiskControl(format!(
        "captcha detected ({CAPTCHA_FRAME_KEY}) — khong giai o P2"
    ))
}

/// `find_captcha_frame` `:692-696`: frame nao co `bdcaptcha.html` trong URL.
fn has_captcha(frame_urls: &[String]) -> bool {
    frame_urls.iter().any(|u| u.contains(CAPTCHA_FRAME_KEY))
}

/// `:684-687` `fresh = [x for x in (recent or []) if x and x not in known]` -> `fresh[0]`.
fn first_fresh(recent: &[String], known: &HashSet<String>) -> Option<String> {
    recent
        .iter()
        .find(|x| !x.is_empty() && !known.contains(x.as_str()))
        .cloned()
}

/// `:2578` `i % 3 == 2` — chi hoi `recent_conv` moi 3 nhip (nhip thu 3, 6, 9...).
fn is_recent_tick(i: u32) -> bool {
    i % 3 == 2
}

/// Task 12 se thay bang `skill::submit_pro/_pro_max/_agent`.
fn unsupported_mode(mode: Mode) -> DolaError {
    DolaError::ModeUnavailable(format!("mode {} chưa hỗ trợ (Task 12)", mode.as_str()))
}

/// `compose`/`refs` goi `raise_if_logged_out(page, "", ...)` nhu Python (`:887`, `:1953`...) -> dien
/// ten tai khoan vao `LoggedOut` con rong; loi khac giu nguyen.
fn with_account(e: DolaError, account: &str) -> DolaError {
    match e {
        DolaError::LoggedOut {
            account: a,
            why,
            revoked,
        } if a.is_empty() => DolaError::LoggedOut {
            account: account.to_string(),
            why,
            revoked,
        },
        other => other,
    }
}

/// `:2592` `Path("logs") / f"no_conv_{account}_{ts}.png"`.
fn no_conv_shot_path(logs_dir: &Path, account: &str, ts: &str) -> PathBuf {
    logs_dir.join(format!("no_conv_{account}_{ts}.png"))
}

// ---------------------------------------------------------------------------------------------
// Luong gui
// ---------------------------------------------------------------------------------------------

/// Gui prompt tren trang Dola that ([`DOLA_CHAT_URL`]) va tra `conversation_id` cua chat MOI (R5).
/// Xem [`submit_at`].
///
/// # Errors
/// Nhu [`submit_at`].
pub async fn submit(ctx: &SubmitCtx<'_>, req: &SubmitRequest) -> Result<ConversationId, DolaError> {
    submit_at(ctx, req, DOLA_CHAT_URL).await
}

/// Nhu [`submit`] nhung trang chat la `chat_url` (smoke tren fixture cuc bo — KHONG goi dola.com).
/// Thu tu (`generate_video` `:2397-2494`):
/// 1. [`precheck_duration`] (thuan, truoc moi I/O);
/// 2. [`open_new_chat`]: URL da la `chat_url` -> 1 s, khong thi `goto` + 5 s; `assert_logged_in`;
/// 3. `known = recent_conv_ids()` (`:2472`) — bo `_preflight_balance` (`:2473`, R7);
/// 4. `select_mode(target_mode(mode))` (`:2475-2485`; cookie banner nam trong `select_mode`);
/// 5. re nhanh theo `mode` (`:2486-2490`): Fast -> [`submit_fast`]; con lai -> `MODE_UNAVAILABLE`
///    (Task 12);
/// 6. [`after_submit`] (`:2491-2494`).
///
/// Moi `LoggedOut` co `account` rong (tu `compose`/`refs`) duoc dien `req.account`.
///
/// # Errors
/// `DurationUnavailable` (buoc 1, 15/30 khong chon/hook duoc), `LoggedOut`, `ModeUnavailable`,
/// `ModeSwitch`, `RatioUnavailable`, `RefUpload`, `RiskControl` (captcha), `NoConversation`,
/// `Browser` (CDP/timeout).
pub async fn submit_at(
    ctx: &SubmitCtx<'_>,
    req: &SubmitRequest,
    chat_url: &str,
) -> Result<ConversationId, DolaError> {
    precheck_duration(req.mode, req.model, req.duration)?;
    submit_inner(ctx, req, chat_url)
        .await
        .map_err(|e| with_account(e, &req.account))
}

async fn submit_inner(
    ctx: &SubmitCtx<'_>,
    req: &SubmitRequest,
    chat_url: &str,
) -> Result<ConversationId, DolaError> {
    let page = ctx.page;
    let account = req.account.as_str();
    tracing::info!(
        account,
        mode = req.mode.as_str(),
        model = req.model.as_str(),
        ratio = req.ratio.map(Ratio::as_str),
        duration = req.duration,
        refs = req.reference_images.len(),
        prompt_chars = req.prompt.chars().count(),
        "submit"
    );
    open_new_chat(page, account, chat_url).await?;
    // :2472 — nho cac chat DANG CO truoc khi gui de sau khi gui khong nhan nham chat cu
    let known: HashSet<String> = session::recent_conv_ids(page).await?.into_iter().collect();
    tracing::debug!(account, known = known.len(), "chat gan day truoc khi gui");
    // :2475-2485 — pro_max/agent dung chung nut プロ; chi Agent moi nuot loi (Task 12)
    let target = compose::target_mode(req.mode);
    let label = compose::select_mode(page, target).await?;
    tracing::info!(account, label = %label, mode = req.mode.as_str(), "Dola mode");
    // :2486-2490
    match req.mode {
        Mode::Fast => submit_fast(ctx, req).await?,
        Mode::Pro | Mode::ProMax | Mode::Agent => return Err(unsupported_mode(req.mode)),
    }
    // :2491-2494
    after_submit(ctx, account, &known, chat_url).await
}

/// `:2461-2469` (chat moi, R5): URL hien tai da la `chat_url` -> cho 1 s; khong thi `goto` (cho
/// `DOMContentLoaded`, 60 s) + 5 s; roi `assert_logged_in_at` (REQ-002: truoc moi thao tac, khong
/// mat luot).
///
/// # Errors
/// `LoggedOut`, `Browser` (dieu huong/CDP).
pub async fn open_new_chat(page: &Page, account: &str, chat_url: &str) -> Result<(), DolaError> {
    let current = ops::url(page).await?;
    if is_chat_url(&current, chat_url) {
        tracing::debug!(account, "trang da o /chat -> cho 1 s");
        tokio::time::sleep(CHAT_ALREADY_OPEN_SETTLE).await;
    } else {
        tracing::info!(account, "mo trang chat moi (+5 s)");
        ops::goto(page, chat_url, CHAT_GOTO_SETTLE).await?;
    }
    session::assert_logged_in_at(page, account, chat_url).await
}

/// Port `_submit_fast` (`:2264-2301`): `open_video_composer_fast` -> `clear_composer_attachments`
/// -> (anh tham chieu: `attach_reference_images`, timeout `Limits::reference_upload_timeout_s`) ->
/// `select_model` -> `close_open_menus` -> (`select_ratio`) -> [`select_duration_or_hook`] ->
/// `type_and_send(prompt, append=false)` -> neu hook: `verify_hook_patched` (20 x 250 ms) sai ->
/// `DurationUnavailable` voi [`compose::hook_state`] cuoi (`:2296-2299`).
///
/// # Errors
/// `ModeUnavailable` (composer), `RefUpload`, `RatioUnavailable`, `DurationUnavailable`, `LoggedOut`
/// (account rong — [`submit_at`] dien), `Browser`.
pub async fn submit_fast(ctx: &SubmitCtx<'_>, req: &SubmitRequest) -> Result<(), DolaError> {
    let page = ctx.page;
    let account = req.account.as_str();
    compose::open_video_composer_fast(page).await?; // :2267
    // :2268 — anh dinh kem cu con trong composer -> nhan doi / gui nham
    let removed = compose::clear_composer_attachments(page).await?;
    if removed > 0 {
        tracing::info!(account, removed, "đã xoá ảnh đính kèm cũ trong composer");
    }
    // :2269 `_attach_refs` -> `attach_reference_images` (:1688-1689) — SAU khi mo composer
    if !req.reference_images.is_empty() {
        let timeout = Duration::from_secs(u64::from(ctx.limits.reference_upload_timeout_s));
        let out = refs::attach_reference_images(page, &req.reference_images, timeout).await?;
        tracing::info!(
            account,
            expected = out.expected,
            tos = out.tos,
            thumbs = out.thumbs,
            way = ?out.way,
            extra_uploads = out.extra_uploads,
            "ảnh tham chiếu đã đính"
        );
    }
    compose::select_model(page, req.model).await?; // :2270
    compose::close_open_menus(page).await?; // :2271
    if let Some(ratio) = req.ratio {
        compose::select_ratio(page, ratio).await?; // :2272-2273
    }
    let hooked = select_duration_or_hook(page, account, req.duration).await?; // :2274-2288
    compose::type_and_send(page, &req.prompt, false).await?; // :2289
    if hooked {
        // :2290-2300 — 20 x 250 ms cho `patched:N`
        if !compose::verify_hook_patched(page, req.duration).await? {
            let state = compose::hook_state(page).await?;
            return Err(DolaError::DurationUnavailable(hook_not_patched_msg(&state)));
        }
        tracing::info!(
            account,
            duration = req.duration,
            "body /chat/completion đã đổi duration trước khi ký (ADR-044)"
        );
    }
    // :2301 — Python in `prompt[:40]`; o day KHONG log noi dung
    tracing::info!(
        account,
        prompt_chars = req.prompt.chars().count(),
        "UI submitted prompt"
    );
    Ok(())
}

/// `:2274-2288`: `select_fast_duration(d)`; `DurationUnavailable` ma `d ∈ {15, 30}` ->
/// `close_open_menus` -> `select_fast_duration(10)` -> `install_duration_hook(d)` (ADR-044) ->
/// `Ok(true)` (da hook); loi khac / d khac -> nem. `Ok(false)` = UI co option that.
async fn select_duration_or_hook(
    page: &Page,
    account: &str,
    duration: u16,
) -> Result<bool, DolaError> {
    match compose::select_fast_duration(page, duration).await {
        Ok(()) => Ok(false),
        Err(DolaError::DurationUnavailable(why)) if matches!(duration, 15 | 30) => {
            tracing::info!(
                account,
                duration,
                why = %why,
                "Dola UI không có option thời lượng → giữ 10s, hook đổi ability_param.duration (ADR-044)"
            );
            compose::close_open_menus(page).await?; // :2283
            compose::select_fast_duration(page, 10).await?; // :2284
            compose::install_duration_hook(page, duration).await?; // :2285-2287
            Ok(true)
        }
        Err(e) => Err(e),
    }
}

/// `:2591-2596` — `logs/no_conv_<account>_<ts>.png`; loi (ke ca tao thu muc) -> `(khong chup duoc)`.
async fn screenshot_no_conv(page: &Page, logs_dir: &Path, account: &str) -> String {
    let shot = async {
        tokio::fs::create_dir_all(logs_dir).await?;
        let path = no_conv_shot_path(logs_dir, account, &compose::timestamp_utc());
        page.save_screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .build(),
            &path,
        )
        .await
        .map_err(BrowserError::Cdp)?;
        Ok::<String, BrowserError>(path.display().to_string())
    }
    .await;
    match shot {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(account, error = %e, "khong chup duoc anh no_conv");
            SHOT_FAILED.to_string()
        }
    }
}

/// Port `_after_submit` phan captcha + conversation id (`:2533-2600`, R3 — xem doc module): moi
/// nhip 1 s (`Limits::no_conversation_timeout_s` nhip): (a) [`has_captcha`] -> `RiskControl`;
/// (b) `pick_new_conv_id(url_tail, known)`: `New(id)` -> tra; `Known(id)` va [`is_recent_tick`] ->
/// `recent_conv_ids` (`:2579`), id dau tien ∉ `known` -> `goto <chat_url>/<id>` + 2 s (loi chi
/// warn, `:2581-2587`) -> tra. Het gio: anh `logs/no_conv_<account>_<ts>.png` ->
/// `raise_if_logged_out(account, "ngay sau khi gửi (ảnh <shot>)")` (`:2599`, `revoked`) ->
/// `NoConversation` (`:2600`).
///
/// # Errors
/// `RiskControl`, `LoggedOut { revoked: true }`, `NoConversation`, `Browser` (CDP: URL/frame/cookie).
// PF-3: `known` la `HashSet<String>` cu the — cung kieu voi `lvc_core::pick_new_conv_id(&HashSet<String>)`
// (interface da chot o Task 2); khong generic hoa theo BuildHasher.
#[allow(clippy::implicit_hasher)]
pub async fn after_submit(
    ctx: &SubmitCtx<'_>,
    account: &str,
    known: &HashSet<String>,
    chat_url: &str,
) -> Result<ConversationId, DolaError> {
    let page = ctx.page;
    let ticks = ctx.limits.no_conversation_timeout_s;
    let mut warned = String::new();
    for i in 0..ticks {
        tokio::time::sleep(CONV_TICK).await; // :2566
        // (a) R3/R6: captcha -> bao ngay, khong giai
        if has_captcha(&ops::frame_urls(page).await?) {
            tracing::warn!(
                account,
                tick = i,
                "Captcha detected (bdcaptcha.html) → RISK_CONTROL"
            );
            return Err(captcha_error());
        }
        // (b) :2567-2588
        let url = ops::url(page).await?;
        let tail = url_tail(&url);
        match pick_new_conv_id(tail, known) {
            ConvPick::New(id) => {
                tracing::info!(account, conversation_id = %id, tick = i, "conversation_id");
                return Ok(ConversationId(id));
            }
            ConvPick::Known(old) if is_recent_tick(i) => {
                // 2026-09-17 toi: trang nhay ve chat CU sau khi gui -> hoi recent_conv lay chat moi
                if warned != old {
                    warned.clone_from(&old);
                    tracing::info!(
                        account,
                        old = %old,
                        "URL đang ở chat cũ sau khi gửi → tìm chat mới qua recent_conv"
                    );
                }
                let recent = session::recent_conv_ids(page).await?;
                if let Some(fresh) = first_fresh(&recent, known) {
                    tracing::info!(
                        account,
                        conversation_id = %fresh,
                        old = %old,
                        "chat mới (trang đang ở chat cũ) → mở lại đúng chat"
                    );
                    let target = conv_url(chat_url, &fresh);
                    if let Err(e) = ops::goto(page, &target, CONV_JUMP_SETTLE).await {
                        tracing::warn!(
                            account,
                            conversation_id = %fresh,
                            error = %e,
                            "mở lại chat lỗi (vẫn poll theo id)"
                        );
                    }
                    return Ok(ConversationId(fresh));
                }
            }
            ConvPick::Known(_) | ConvPick::None => {}
        }
    }
    // :2589-2600 — co ma loi + anh chup rieng tung lan de dashboard goi y duoc
    let shot = screenshot_no_conv(page, &ctx.logs_dir, account).await;
    // 2026-09-17 (do that 13 tai khoan moi): anh no_conv_* hien nut ログイン vi server tra STREAM_ERROR
    // 710022002 roi front-end tu logout (from_logout=1) -> LOGGED_OUT(revoked), KHONG auto-verify.
    session::raise_if_logged_out(page, account, &format!("ngay sau khi gửi (ảnh {shot})")).await?;
    let url = ops::url(page).await.unwrap_or_default();
    Err(DolaError::NoConversation(no_conversation_msg(
        ticks, &url, &shot,
    )))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::Duration;

    use lvc_core::{Limits, Mode, Model, Ratio};

    use super::*;
    use crate::dola::DolaError;

    fn req(mode: Mode, model: Model, duration: u16, refs: usize) -> SubmitRequest {
        SubmitRequest {
            account: "acc1".into(),
            prompt: "BI MAT prompt".into(),
            mode,
            ratio: Some(Ratio::R16x9),
            duration,
            model,
            reference_images: (0..refs)
                .map(|i| PathBuf::from(format!("a{i}.png")))
                .collect(),
        }
    }

    /// `Debug` tu viet: KHONG in noi dung prompt, chi `prompt_chars` (chinh sach log P2).
    #[test]
    fn submit_request_debug_khong_lo_prompt() {
        let r = req(Mode::Fast, Model::Seedance20, 10, 1);
        let s = format!("{r:?}");
        assert!(!s.contains("BI MAT"), "{s}");
        assert!(s.contains("prompt_chars: 13"), "{s}");
        assert!(s.contains("account: \"acc1\""), "{s}");
        assert!(s.contains("a0.png"), "{s}");
        let c = r.clone();
        assert_eq!(c.prompt, r.prompt);
    }

    /// `submit_timeout` = `lvc_core::video_timeout(mode, duration, co anh, limits)` (`modes.py:278-285`
    /// + `video_worker_ui.py:2404-2405`).
    #[test]
    fn submit_timeout_theo_video_timeout() {
        let l = Limits::default();
        let t = |m, model, d, refs| submit_timeout(&req(m, model, d, refs), &l);
        assert_eq!(
            t(Mode::Fast, Model::Seedance20, 10, 0),
            Duration::from_secs(300)
        );
        assert_eq!(
            t(Mode::Fast, Model::Seedance25, 30, 0),
            Duration::from_secs(1500)
        );
        assert_eq!(
            t(Mode::Fast, Model::Seedance20, 10, 2),
            Duration::from_secs(900)
        );
        assert_eq!(
            t(Mode::Pro, Model::Seedance20, 10, 0),
            Duration::from_secs(1800)
        );
    }

    /// Kiem tra thuan `:2399-2403` — hai thong diep Python (oracle `ValueError(...)`), ma
    /// `DURATION_UNAVAILABLE`; hop le -> `Ok`.
    #[test]
    fn precheck_duration_thong_diep_python() {
        for (m, model, d) in [
            (Mode::Fast, Model::Seedance20, 10),
            (Mode::Fast, Model::Seedance20, 15),
            (Mode::Fast, Model::Seedance25, 30),
            (Mode::Agent, Model::Seedance25, 15),
        ] {
            precheck_duration(m, model, d).unwrap_or_else(|e| panic!("{m:?} {model:?} {d}: {e}"));
        }
        let e = precheck_duration(Mode::Fast, Model::Seedance25, 20).unwrap_err();
        assert_eq!(
            e.to_string(),
            "DURATION_UNAVAILABLE: Dola supports durations of 10s, 15s and 30s"
        );
        assert!(matches!(e, DolaError::DurationUnavailable(_)));
        for (m, model) in [
            (Mode::Fast, Model::Seedance20),
            (Mode::Pro, Model::Seedance25),
            (Mode::Agent, Model::Seedance10),
        ] {
            let e = precheck_duration(m, model, 30).unwrap_err();
            assert_eq!(
                e.to_string(),
                "DURATION_UNAVAILABLE: 30s is only available with mode=fast and model seedance-2.5 \
                 (Dola caps agent/pro and Seedance 2.0 at 15s)",
                "{m:?} {model:?}"
            );
        }
        let e = precheck_duration(Mode::Fast, Model::Seedance25, 0).unwrap_err();
        assert!(e.to_string().ends_with("10s, 15s and 30s"), "{e}");
    }

    /// `:2461-2462` `page.url.rstrip("/") == chat_url` — ruling Task 9 #5: bo query/fragment va `/`
    /// cuoi o CA HAI ve.
    #[test]
    fn is_chat_url_bo_query_va_gach_cheo_cuoi() {
        let c = "https://www.dola.com/chat";
        assert!(is_chat_url("https://www.dola.com/chat", c));
        assert!(is_chat_url("https://www.dola.com/chat/", c));
        assert!(is_chat_url("https://www.dola.com/chat?x=1", c));
        assert!(is_chat_url("https://www.dola.com/chat#frag", c));
        assert!(is_chat_url(
            "https://www.dola.com/chat",
            "https://www.dola.com/chat/?y=2"
        ));
        assert!(!is_chat_url("https://www.dola.com/chat/123", c));
        assert!(!is_chat_url("https://www.dola.com/", c));
        assert!(!is_chat_url("", c));
        assert!(is_chat_url(
            "http://127.0.0.1:1/chat?noconv=1",
            "http://127.0.0.1:1/chat?noconv=1"
        ));
    }

    /// `:2567` `page.url.rstrip("/").split("/")[-1]` (oracle Python: query KHONG bi bo).
    #[test]
    fn url_tail_theo_python() {
        assert_eq!(url_tail("https://www.dola.com/chat/123/"), "123");
        assert_eq!(url_tail("https://www.dola.com/chat"), "chat");
        assert_eq!(
            url_tail("http://127.0.0.1:1/chat?noconv=1"),
            "chat?noconv=1"
        );
        assert_eq!(url_tail(""), "");
        assert_eq!(url_tail("///"), "");
    }

    /// `:2582` `f"https://www.dola.com/chat/{conv_id}"` — tren `chat_url` bat ky (bo query, bo `/`).
    #[test]
    fn conv_url_ghep_id() {
        assert_eq!(
            conv_url("https://www.dola.com/chat", "123"),
            "https://www.dola.com/chat/123"
        );
        assert_eq!(
            conv_url("http://127.0.0.1:1/chat?jump=1", "9"),
            "http://127.0.0.1:1/chat/9"
        );
        assert_eq!(
            conv_url("https://www.dola.com/chat/", "1"),
            "https://www.dola.com/chat/1"
        );
    }

    /// `:2600` `url={page.url.split('?')[0][:80]}` (oracle Python) + `anh=`.
    #[test]
    fn no_conversation_msg_cat_80_ky_tu_bo_query() {
        let long = format!("https://www.dola.com/chat/{}?a=1", "x".repeat(100));
        let m = no_conversation_msg(30, &long, r"logs\no_conv_acc_20260918_010203.png");
        assert_eq!(
            m,
            format!(
                "Dola khong tao hoi thoai sau 30s (url=https://www.dola.com/chat/{}, \
                 anh=logs\\no_conv_acc_20260918_010203.png)",
                "x".repeat(54)
            )
        );
        assert_eq!(
            no_conversation_msg(3, "https://www.dola.com/chat", "(khong chup duoc)"),
            "Dola khong tao hoi thoai sau 3s (url=https://www.dola.com/chat, anh=(khong chup duoc))"
        );
        assert_eq!(
            DolaError::NoConversation(no_conversation_msg(30, "u", "s")).to_string(),
            "NO_CONVERSATION: Dola khong tao hoi thoai sau 30s (url=u, anh=s)"
        );
    }

    /// `:2296-2299` (oracle Python, `{state!r}`).
    #[test]
    fn hook_not_patched_msg_theo_python() {
        assert_eq!(
            DolaError::DurationUnavailable(hook_not_patched_msg("")).to_string(),
            "DURATION_UNAVAILABLE: hook không đổi được duration trong body /chat/completion \
             (state='') — Dola có thể đã đổi định dạng request; video sẽ là 10s nếu tiếp tục"
        );
        assert!(hook_not_patched_msg("installed:15").contains("(state='installed:15')"));
        assert!(hook_not_patched_msg("it's").contains("(state=\"it's\")"));
    }

    /// Captcha: `RiskControl` khong ma, thong diep co dinh (ruling #3).
    #[test]
    fn captcha_error_la_risk_control() {
        let e = captcha_error();
        assert_eq!(e.failure_code(), None);
        assert!(e.rotates_account());
        assert_eq!(
            e.to_string(),
            "RISK_CONTROL: captcha detected (bdcaptcha.html) — khong giai o P2"
        );
        assert!(has_captcha(&[
            "http://x/chat".into(),
            "https://verify.example/bdcaptcha.html?x=1".into()
        ]));
        assert!(!has_captcha(&["http://x/chat".into()]));
        assert!(!has_captcha(&[]));
    }

    /// Bien the `LoggedOut` tu `compose`/`refs` (`raise_if_logged_out(page, "", ...)`) mang `account`
    /// rong -> dien ten tai khoan; co san thi giu; loi khac khong doi.
    #[test]
    fn with_account_chi_dien_khi_rong() {
        let e = with_account(
            DolaError::LoggedOut {
                account: String::new(),
                why: "w".into(),
                revoked: true,
            },
            "acc1",
        );
        assert!(
            matches!(&e, DolaError::LoggedOut { account, why, revoked: true } if account == "acc1" && why == "w"),
            "{e:?}"
        );
        let e = with_account(
            DolaError::LoggedOut {
                account: "khac".into(),
                why: "w".into(),
                revoked: false,
            },
            "acc1",
        );
        assert!(matches!(&e, DolaError::LoggedOut { account, .. } if account == "khac"));
        let e = with_account(DolaError::Http("h".into()), "acc1");
        assert!(matches!(e, DolaError::Http(_)));
    }

    /// `pick_new_conv_id(tail, known, recent)` `:684-687`: `fresh = [x for x in recent if x and x not in
    /// known]` -> `fresh[0]` (oracle: C -> 777..., D -> khong co).
    #[test]
    fn first_fresh_theo_python() {
        let known: HashSet<String> = ["3841796000000000689", "12345"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let recent = vec![
            "777000000000000001".to_string(),
            "3841796000000000689".to_string(),
            "12345".to_string(),
        ];
        assert_eq!(
            first_fresh(&recent, &known).as_deref(),
            Some("777000000000000001")
        );
        assert_eq!(
            first_fresh(
                &["3841796000000000689".to_string(), "12345".to_string()],
                &known
            ),
            None
        );
        assert_eq!(
            first_fresh(&[String::new(), "9".into()], &known).as_deref(),
            Some("9")
        );
        assert_eq!(first_fresh(&[], &known), None);
    }

    /// `:2578` `i % 3 == 2` (oracle: [2, 5, 8, 11, 14]).
    #[test]
    fn recent_tick_moi_3_nhip_tu_nhip_thu_3() {
        let ticks: Vec<u32> = (0..30).filter(|i| is_recent_tick(*i)).collect();
        assert_eq!(&ticks[..5], &[2, 5, 8, 11, 14]);
        assert_eq!(ticks.len(), 10);
    }

    /// Che do ngoai Fast (Task 12) -> `MODE_UNAVAILABLE`, thong diep noi ro.
    #[test]
    fn unsupported_mode_bao_mode_unavailable() {
        let e = unsupported_mode(Mode::ProMax);
        assert_eq!(
            e.to_string(),
            "MODE_UNAVAILABLE: mode pro_max chưa hỗ trợ (Task 12)"
        );
        assert_eq!(
            e.failure_code(),
            Some(lvc_core::FailureCode::ModeUnavailable)
        );
    }

    /// `SubmitCtx::new` mac dinh `logs_dir = "logs"` (Python `Path("logs")` `:2591`).
    #[test]
    fn submit_ctx_logs_dir_mac_dinh() {
        assert_eq!(default_logs_dir(), PathBuf::from("logs"));
        assert_eq!(
            no_conv_shot_path(&PathBuf::from("logs"), "acc1", "20260918_010203"),
            PathBuf::from("logs").join("no_conv_acc1_20260918_010203.png")
        );
    }

    #[test]
    fn conversation_id_display_va_eq() {
        let id = ConversationId("123".into());
        assert_eq!(id.to_string(), "123");
        assert_eq!(id, ConversationId("123".into()));
        assert_eq!(id.0, "123");
    }
}
