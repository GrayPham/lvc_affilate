//! Composer Dola (Task 7, P2) — port CLICK-THEO-CLICK cua `video_worker_ui.py` (HEAD 3d07c19):
//!
//! | Rust | Python |
//! |---|---|
//! | [`close_open_menus`] | `_close_open_menus` `:1845-1851` |
//! | [`dismiss_cookie_banner`] | `_select_mode_locked` `:1987-1993` |
//! | [`select_mode`] | `select_mode`/`_select_mode_locked` `:1970-2050`, `_wait_mode_button` `:1925-1945`, `_mode_unavailable_message` `:1951-1967`, `_is_mode_match` `:1867-1871` |
//! | [`open_video_composer_fast`] | `open_video_composer` nhanh Nhanh `:2053-2089` |
//! | [`clear_composer_attachments`] | `_clear_composer_attachments` `:794-833` |
//! | [`select_model`] | `_select_model` `:2136-2169` |
//! | [`select_ratio`] | `_select_ratio` `:2176-2207` |
//! | [`select_fast_duration`] | `select_fast_duration` `:1879-1922`, `_is_duration_match` `:1874-1876` |
//! | [`install_duration_hook`], [`hook_state`], [`verify_hook_patched`] | `:2249-2261`; vong kiem `patched:N` trong `_submit_fast` `:2289-2300` |
//! | [`type_and_send`] | `_type_and_send` `:2117-2133` + `_enter_text` `:1231-1239` |
//! | [`clear_ability_tag`] | `_clear_ability_tag` `:2349-2367` |
//!
//! # Hai "the gioi" regex (ruling Task 7 #2)
//! - **CHON phan tu** — LUON qua finder JS (`labels::finder_*`: `RegExp` chay TRONG trang, `\b`/`\d`
//!   ASCII) vi Python dua `has_text=`/`get_by_text` cho Playwright chay trong trinh duyet. He qua da
//!   chung minh: `MODEL_BTN_SRC` `(モデル|Model)\b` KHONG khop "モデル 2.0高速" trong JS, nen Python
//!   `_select_model` roi xuong `get_by_text(label, exact=True)` (`model_current`) roi
//!   `get_by_text(re.compile(re.escape("モデル")), exact=False)` (`:2148` — locator REGEX = chuoi con
//!   khong neo tren text, phan tu sau nhat => `finder_text_lax("モデル")`, KHONG phai exact) —
//!   [`select_model`] giu DUNG thu tu do. KHONG bao gio ap `labels::*_RE` Rust len chu keo ve de "chon".
//! - **DOC LAI chu** — Python `inner_text()` roi `re`/`in` Python tren chuoi: [`is_mode_match`],
//!   [`is_duration_match`], kiem "nut ti le da chua `16:9`" (`:2184`), "nut model da chua `2.5`"
//!   (`:2154`). O day dung Rust (`str::contains`, regex Unicode) tren `innerText` — chinh la dieu
//!   Python lam.
//!
//! # Bam chuot
//! Playwright `locator.click(timeout=N)` cho phan tu visible toi da N roi bam; o day [`click_wait`]
//! lap `ops::click_js` (visible = rect != 0) toi N. `click_js` KHONG kiem actionability (Task 5
//! M-1 park). Truoc cu bam dau cua [`select_mode`] chi co [`dismiss_cookie_banner`] — DUNG NHU Python
//! `_select_mode_locked` (`:1987-1993`); Python KHONG `_close_open_menus` truoc cu bam dau (chi trong
//! nhanh reload `:2017` va sau khi chon muc menu `:2042`) — ruling review Task 7 I-1: parity thang, giu
//! nguyen. [`close_open_menus`] van duoc goi truoc [`type_and_send`] (`:2124`) va truoc/sau cac
//! menu nhu Python.
//!
//! Log qua `tracing` thay `print`; KHONG log noi dung prompt (chi do dai).

use std::path::Path;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::page::ScreenshotParams;
use lvc_core::{Mode, Model, Ratio};
use regex::Regex;

use super::error::DolaError;
use super::labels::{
    ABILITY_TAG_SEL, ABILITY_TAG_SRC, ALL_MODE_LABELS, ATTACH_CARD_SEL, BUTTONS_JS,
    CLEAR_ABILITY_TAG_JS, COOKIE_OK_FINDER, COUNT_ATTACH_JS, DRAFT_BOX_SEL, DUR_BTN_SRC,
    DUR_MENU_ITEM_SEL, FAST_ITEM_SRC, FAST_MODE_LABELS, MODE_BTN_LOOSE_SRC, MODE_BTN_SRC,
    MODE_HAS_TEXT_FINDER, MODEL_BTN_SRC, MODEL_OPTIONS, OPEN_MENU_JS, PRO_ITEM_SRC,
    PRO_MODE_LABELS, RATIO_BTN_SRC, UI_LABELS, VIDEO_BTN_SRC, VIDEO_TAG_SRC, dur_item_src,
    duration_hook_script, finder_button_text, finder_composer, finder_menu_item,
    finder_menu_item_in, finder_text_exact, finder_text_lax, ratio_item_src,
};
use super::session;
use crate::BrowserError;
use crate::ops;

// ---------------------------------------------------------------------------------------------
// Hang thoi gian — moi so la literal cua Python (dong ghi ben canh)
// ---------------------------------------------------------------------------------------------

/// Chu ky poll cua [`click_wait`]/[`inner_text_wait`]/`wait_until` (Playwright poll ~ raf/100 ms).
const POLL: Duration = Duration::from_millis(100);
/// `_close_open_menus(tries=3)` `:1845`; ngu 300 ms sau moi Escape `:1851`.
const MENU_CLOSE_TRIES: u32 = 3;
const MENU_ESC_SETTLE: Duration = Duration::from_millis(300);
/// Cookie banner: `wait_for_timeout(500)` `:1992`.
const COOKIE_SETTLE: Duration = Duration::from_millis(500);
/// `_wait_mode_button(page, 8000)` `:1995`; retry 5000 `:2011`, `:2025`; sau reload 15000 `:2023`.
const MODE_BTN_WAIT: Duration = Duration::from_secs(8);
const MODE_BTN_RETRY: Duration = Duration::from_secs(5);
const MODE_BTN_AFTER_RELOAD: Duration = Duration::from_secs(15);
/// `c.wait_for(state="attached", timeout=min(timeout_ms, 5000))` `:1941`.
const ATTACHED_WAIT_CAP: Duration = Duration::from_secs(5);
/// Sau khi go the ky nang + xoa nhap: `wait_for_timeout(800)` `:2010`, `:2024`.
const CLEAR_DRAFT_SETTLE: Duration = Duration::from_millis(800);
/// `box.click(timeout=3000)` `:2005`.
const DRAFT_CLICK_TIMEOUT: Duration = Duration::from_secs(3);
/// Sau `page.reload`: `wait_for_timeout(5000)` `:2022` (ngu ke ca khi reload loi).
const RELOAD_SETTLE: Duration = Duration::from_secs(5);
/// `_mode_unavailable_message`: `asyncio.wait_for(page.evaluate(_BUTTONS_JS), timeout=10)` `:1962`.
const BUTTONS_JS_TIMEOUT: Duration = Duration::from_secs(10);
/// Thu muc anh chup `logs/mode_fail_<ts>.png` `:1957-1958` (tuong doi cwd nhu Python).
const LOGS_DIR: &str = "logs";
/// Vong doc/bam nut che do: 2 vong `:2031`; `inner_text(timeout=8000)` `:2032`; `click(timeout=5000)`
/// `:2035`; 700 ms sau khi mo menu `:2036`; 1500 ms sau khi chon muc `:2041`.
const MODE_ROUNDS: u32 = 2;
const MODE_TEXT_TIMEOUT: Duration = Duration::from_secs(8);
const MODE_CLICK_TIMEOUT: Duration = Duration::from_secs(5);
const MENU_SETTLE: Duration = Duration::from_millis(700);
const ITEM_SETTLE: Duration = Duration::from_millis(1500);
/// `open_video_composer` Nhanh: 3 lan `:2066`; `click(timeout=5000*attempt)` `:2070`, `:2074`;
/// 1500 ms sau khi bam `:2076`; `wait_for_timeout(1500*attempt)` giua cac lan `:2085`.
const COMPOSER_TRIES: u32 = 3;
const COMPOSER_CLICK_BASE: Duration = Duration::from_secs(5);
const COMPOSER_SETTLE: Duration = Duration::from_millis(1500);
const COMPOSER_RETRY_BASE: Duration = Duration::from_millis(1500);
/// `_clear_composer_attachments`: hover roi 250 ms `:813`; 4 toa do goc phai `:819`; 350 ms sau moi
/// cu bam `:821`; toi da `n + 3` vong `:806`.
const ATTACH_HOVER_SETTLE: Duration = Duration::from_millis(250);
const ATTACH_CLICK_SETTLE: Duration = Duration::from_millis(350);
const ATTACH_OFFSETS: [(f64, f64); 4] = [(4.0, 4.0), (6.0, 6.0), (2.0, 2.0), (8.0, 8.0)];
const ATTACH_EXTRA_ROUNDS: usize = 3;
/// `_select_model`: `click(timeout=5000)` `:2159`, `:2164`; 500 ms sau khi mo menu `:2160` va sau
/// khi chon `:2165`.
const MODEL_CLICK_TIMEOUT: Duration = Duration::from_secs(5);
const MODEL_MENU_SETTLE: Duration = Duration::from_millis(500);
const MODEL_OPTION_SETTLE: Duration = Duration::from_millis(500);
/// `_select_ratio(tries=3)` `:2176`; `click(timeout=3000)` `:2186`, `:2190`, `:2192`; 600 ms sau khi
/// mo menu `:2191`; 400 ms sau khi chon `:2193`; `inner_text(timeout=2000)` `:2196`; 700 ms giua cac
/// lan `:2205`.
const RATIO_TRIES: u32 = 3;
const RATIO_CLICK_TIMEOUT: Duration = Duration::from_secs(3);
const RATIO_MENU_SETTLE: Duration = Duration::from_millis(600);
const RATIO_ITEM_SETTLE: Duration = Duration::from_millis(400);
const RATIO_TEXT_TIMEOUT: Duration = Duration::from_secs(2);
const RATIO_RETRY_PAUSE: Duration = Duration::from_millis(700);
/// `select_fast_duration`: 10 x 500 ms cho nhan khop `:1888-1894` (`inner_text(timeout=1000)`);
/// `click(timeout=3000)` `:1900`, `:1905`; 600 ms sau khi mo `:1901`; 400 ms sau khi chon `:1906`;
/// `inner_text(timeout=3000)` `:1907`.
const DUR_POLLS: u32 = 10;
const DUR_POLL: Duration = Duration::from_millis(500);
const DUR_TEXT_TIMEOUT: Duration = Duration::from_secs(1);
const DUR_CLICK_TIMEOUT: Duration = Duration::from_secs(3);
const DUR_MENU_SETTLE: Duration = Duration::from_millis(600);
const DUR_ITEM_SETTLE: Duration = Duration::from_millis(400);
const DUR_TEXT_AFTER: Duration = Duration::from_secs(3);
/// Vong kiem hook sau khi gui: 20 x 0.25 s `:2291-2295`.
const HOOK_POLLS: u32 = 20;
const HOOK_POLL: Duration = Duration::from_millis(250);
/// `_type_and_send`: `box.click(timeout=10000)` `:2125`; `wait_for_timeout(600)` `:2132`.
const COMPOSER_CLICK_TIMEOUT: Duration = Duration::from_secs(10);
const TYPE_SETTLE: Duration = Duration::from_millis(600);
/// `_clear_ability_tag`: `wait_for_timeout(400)` `:2360`.
const ABILITY_CLEAR_SETTLE: Duration = Duration::from_millis(400);
/// Do dai cat thong diep loi trong log/`last`: Python `str(e)[:80]` (`:826`, `:1913`, `:2203`, `:2020`)
/// va `[:100]` (`:2083`, `:2168`, `:2366`).
const ERR_SNIPPET_SHORT: usize = 80;
const ERR_SNIPPET_LONG: usize = 100;
/// Thoi luong mac dinh cua body Nhanh khi khong co nut thoi luong (ruling NOTES §3.7).
const DEFAULT_DURATION_S: u16 = 10;

// ---------------------------------------------------------------------------------------------
// Ham thuan (co unit test)
// ---------------------------------------------------------------------------------------------

/// `:2475` `target_mode = "pro" if mode in ("pro_max", "agent") else mode` — nut che do chi co
/// 高速/プロ.
#[must_use]
pub fn target_mode(mode: Mode) -> Mode {
    if mode.is_pro_like() {
        Mode::Pro
    } else {
        Mode::Fast
    }
}

/// Port `_is_mode_match` (`:1867-1871`): `strip()` roi `re.search(re.escape(label), cur, I)` cho
/// tung nhan — tuc CHUOI CON khong phan biet hoa/thuong; fast <-> `FAST_MODE_LABELS`, con lai <->
/// `PRO_MODE_LABELS`. Chay tren chu `innerText` da keo ve (the gioi Python, xem doc module).
#[must_use]
pub fn is_mode_match(current_label: &str, target: Mode) -> bool {
    let cur = current_label.trim().to_lowercase();
    let labels: &[&str] = if target_mode(target) == Mode::Fast {
        &FAST_MODE_LABELS
    } else {
        &PRO_MODE_LABELS
    };
    labels.iter().any(|l| cur.contains(&l.to_lowercase()))
}

/// `:1875` `re.search(r"(\d+)\s*(?:s|秒)", current, re.IGNORECASE)` — `\d` Unicode nhu Python `re`.
static DUR_NUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+)\s*(?:s|秒)").unwrap_or_else(|e| panic!("regex thoi luong hop le: {e}"))
});

/// `int()` Python tren chuoi `\d+`: nhan chu so ASCII va full-width (U+FF10..U+FF19 — `int("１０")
/// == 10`); chu so Unicode khac -> `None` (Python van doc duoc, chap nhan lech).
fn py_int(digits: &str) -> Option<u64> {
    digits.chars().try_fold(0_u64, |acc, c| {
        let d = c.to_digit(10).or_else(|| {
            ('０'..='９')
                .contains(&c)
                .then(|| u32::from(c) - u32::from('０'))
        })?;
        acc.checked_mul(10)?.checked_add(u64::from(d))
    })
}

/// Port `_is_duration_match` (`:1874-1876`): so DAU TIEN dung truoc `s`/`秒` bang `duration`.
#[must_use]
pub fn is_duration_match(current: &str, duration: u16) -> bool {
    DUR_NUM_RE
        .captures(current)
        .and_then(|c| py_int(&c[1]))
        .is_some_and(|n| n == u64::from(duration))
}

/// `{x!r}` Python cho thong diep loi: `lvc_core::error::py_repr` (nhay don, `'` -> nhay kep) + escape
/// `\n`/`\r`/`\t` nhu `repr()` (Python in `'比率\n16:9'` — 2 ky tu `\` `n`).
fn py_repr(s: &str) -> String {
    lvc_core::error::py_repr(s)
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// `:2048-2049` `want = "/".join(FAST_MODE_LABELS | PRO_MODE_LABELS)`;
/// `f"Cannot switch Dola mode to {mode} ({want}); ..."` -> `ModeSwitch { want: "fast (高速/Nhanh/Fast)" }`.
fn mode_want(mode: Mode) -> String {
    let target = target_mode(mode);
    let labels: &[&str] = if target == Mode::Fast {
        &FAST_MODE_LABELS
    } else {
        &PRO_MODE_LABELS
    };
    format!("{} ({})", target.as_str(), labels.join("/"))
}

/// Port `_mode_unavailable_message` phan chuoi (`:1964-1967`) — KHONG co tien to `MODE_UNAVAILABLE: `
/// (Display cua [`DolaError::ModeUnavailable`] them). `seen` in kieu list Python `['a', "b'c"]`;
/// `shot` rong -> `n/a`.
#[must_use]
pub fn mode_unavailable_message(url: &str, seen: &[String], shot: &str) -> String {
    let labels = ALL_MODE_LABELS.join("/");
    let items = seen
        .iter()
        .map(|s| py_repr(s))
        .collect::<Vec<_>>()
        .join(", ");
    let shot = if shot.is_empty() { "n/a" } else { shot };
    format!(
        "Dola page shows no {labels} mode button even after reload \
         (url {url}; buttons seen: [{items}]; screenshot {shot})"
    )
}

/// `:2153` `ver = model_key.replace("seedance_v", "")` -> `"2.5"`.
fn model_version(model: Model) -> &'static str {
    model.wire_str().trim_start_matches("seedance_v")
}

/// `:1918-1919` `f"DURATION_UNAVAILABLE: requested {duration}s, selector shows {current!r}"` (phan sau ma).
fn duration_mismatch_msg(duration: u16, current: &str) -> String {
    format!("requested {duration}s, selector shows {}", py_repr(current))
}

/// `:2207` phan sau ma `RATIO_UNAVAILABLE: `.
fn ratio_unavailable_msg(ratio: &str, last: &str) -> String {
    format!("không chọn được tỉ lệ {ratio} trên giao diện Nhanh sau {RATIO_TRIES} lần ({last})")
}

/// Python `(str(e).splitlines() or [repr(e)])[0][:n]` — dong dau, cat `n` ky tu.
fn first_line(s: &str, n: usize) -> String {
    s.lines().next().unwrap_or("").chars().take(n).collect()
}

/// `time.strftime('%Y%m%d_%H%M%S')` `:1958` — Python theo gio may; o day UTC (std khong co mui gio,
/// khong them dependency chi de dat ten file). `pub(crate)`: `submit.rs` dat ten `no_conv_<acc>_<ts>.png`.
pub(crate) fn timestamp_utc() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, mo, d) = civil_from_days(secs / 86_400);
    let rem = secs % 86_400;
    format!(
        "{y:04}{mo:02}{d:02}_{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Ngay Gregorian tu so ngay ke tu 1970-01-01 (thuat toan Howard Hinnant, chi cho ngay >= 1970).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    (y, m, d)
}

// ---------------------------------------------------------------------------------------------
// Helper CDP nho (Locator cua Playwright = chuoi finder JS, danh gia lai moi lan)
// ---------------------------------------------------------------------------------------------

fn js_exists(finder: &str) -> String {
    format!("!!({finder})")
}

/// Playwright `is_visible()`: rect khac 0 va khong `visibility:hidden`.
fn js_visible(finder: &str) -> String {
    format!(
        "(()=>{{const e=({finder}); if(!e) return false; const r=e.getBoundingClientRect(); \
         return r.width>0 && r.height>0 && getComputedStyle(e).visibility!=='hidden';}})()"
    )
}

/// `innerText` (`null` khi khong co phan tu) — Playwright `inner_text()`.
fn js_inner_text(finder: &str) -> String {
    format!("(()=>{{const e=({finder}); return e ? (e.innerText||'') : null;}})()")
}

/// `document.querySelectorAll(sel).length` — Playwright `locator(sel).count()`.
fn js_count_sel(selector: &str) -> String {
    let json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    format!("document.querySelectorAll({json}).length")
}

/// `locator(sel).filter(has_text=/re/i).count()`.
fn js_count_text(selector: &str, re_src: &str) -> String {
    let lit = super::labels::js_regex_literal(re_src, "i");
    let json = serde_json::to_string(selector).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "[...document.querySelectorAll({json})].filter(e => {lit}.test((e.innerText||'').trim())).length"
    )
}

/// Bieu thuc bool; "context bi huy" (trang tu tai lai) -> `false` nhu `wait_until`.
async fn eval_bool(page: &Page, js: &str) -> Result<bool, BrowserError> {
    match ops::eval_json::<bool>(page, js).await {
        Ok(b) => Ok(b),
        Err(e) if ops::context_gone(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

async fn exists(page: &Page, finder: &str) -> Result<bool, BrowserError> {
    eval_bool(page, &js_exists(finder)).await
}

async fn is_visible(page: &Page, finder: &str) -> Result<bool, BrowserError> {
    eval_bool(page, &js_visible(finder)).await
}

async fn count_sel(page: &Page, selector: &str) -> Result<i64, BrowserError> {
    ops::eval_json(page, &js_count_sel(selector)).await
}

/// `innerText.strip()` hoac `None` khi khong co phan tu.
async fn inner_text(page: &Page, finder: &str) -> Result<Option<String>, BrowserError> {
    let v: Option<String> = ops::eval_json(page, &js_inner_text(finder)).await?;
    Ok(v.map(|s| s.trim().to_string()))
}

/// Playwright `locator.inner_text(timeout)`: cho phan tu GAN VAO DOM toi da `timeout` roi doc;
/// het gio -> `Timeout` (Playwright `TimeoutError`, Python de lan len).
async fn inner_text_wait(
    page: &Page,
    finder: &str,
    timeout: Duration,
    what: &str,
) -> Result<String, BrowserError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(t) = inner_text(page, finder).await? {
            return Ok(t);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(BrowserError::Timeout(format!(
                "inner_text {what}: khong thay phan tu sau {timeout:?}"
            )));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Playwright `locator.click(timeout)`: cho phan tu VISIBLE (rect != 0) toi da `timeout` roi bam
/// chuot that (`ops::click_js`); het gio -> `Timeout`. KHONG kiem enabled/stable/che khuat (Task 5 M-1).
async fn click_wait(
    page: &Page,
    finder: &str,
    timeout: Duration,
    what: &str,
) -> Result<(), BrowserError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if ops::click_js(page, finder).await? {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(BrowserError::Timeout(format!(
                "click {what}: khong thay/an sau {timeout:?}"
            )));
        }
        tokio::time::sleep(POLL).await;
    }
}

// ---------------------------------------------------------------------------------------------
// Menu / cookie banner
// ---------------------------------------------------------------------------------------------

/// Port `_close_open_menus` (`:1845-1851`): con `[role="menu"]` (menu radix dang mo chan click vao
/// o nhap) thi `Escape` + 300 ms, toi da 3 lan.
///
/// # Errors
/// Loi CDP/JS.
pub async fn close_open_menus(page: &Page) -> Result<(), BrowserError> {
    for _ in 0..MENU_CLOSE_TRIES {
        if !eval_bool(page, OPEN_MENU_JS).await? {
            return Ok(());
        }
        ops::press(page, "Escape").await?;
        tokio::time::sleep(MENU_ESC_SETTLE).await;
    }
    Ok(())
}

/// Port `:1987-1993`: nut cookie banner (`COOKIE_OK_FINDER` = nut DAU TIEN co chu OK/Đồng ý, chi khi
/// visible) -> bam + 500 ms. Khong co -> khong lam gi. Python boc `try/except: pass` — caller
/// ([`select_mode`]) nuot loi nhu the.
///
/// # Errors
/// Loi CDP/JS.
pub async fn dismiss_cookie_banner(page: &Page) -> Result<(), BrowserError> {
    if ops::click_js(page, COOKIE_OK_FINDER).await? {
        tracing::info!("da tat cookie banner (OK/Đồng ý)");
        tokio::time::sleep(COOKIE_SETTLE).await;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Chon che do
// ---------------------------------------------------------------------------------------------

/// 3 ung vien cua `_wait_mode_button` (`:1927-1931`) theo dung thu tu.
fn mode_button_candidates() -> [String; 3] {
    [
        finder_button_text(&MODE_BTN_SRC),
        finder_button_text(MODE_BTN_LOOSE_SRC),
        MODE_HAS_TEXT_FINDER.to_string(),
    ]
}

/// Port `_wait_mode_button` (`:1925-1945`): luot 1 — ung vien nao `count() and is_visible()` thi
/// tra ngay; luot 2 — tung ung vien `wait_for(state="attached", timeout=min(t, 5000))` (chi can
/// GAN VAO DOM, khong can visible; tuan tu nen toi da 3 x 5 s nhu Python). Khong co -> `None`.
///
/// Khac Python: loi CDP -> `Err` (Python `except: pass`); loi "context bi huy" van coi la chua
/// thay va di tiep.
async fn wait_mode_button(page: &Page, timeout: Duration) -> Result<Option<String>, BrowserError> {
    let cands = mode_button_candidates();
    for c in &cands {
        if is_visible(page, c).await? {
            return Ok(Some(c.clone()));
        }
    }
    for c in &cands {
        if ops::wait_until(page, &js_exists(c), timeout.min(ATTACHED_WAIT_CAP), POLL).await? {
            return Ok(Some(c.clone()));
        }
    }
    Ok(None)
}

/// `:2001-2009`: bam o nhap (`DRAFT_BOX_SEL`), `Control+A`, `Delete` — xoa nhap cu khi composer con
/// mo tu truoc. Loi -> `Err` (caller nuot nhu Python `except: pass`).
async fn clear_draft(page: &Page) -> Result<(), BrowserError> {
    let json = serde_json::to_string(DRAFT_BOX_SEL)?;
    let finder = format!("document.querySelector({json})");
    if exists(page, &finder).await? {
        click_wait(page, &finder, DRAFT_CLICK_TIMEOUT, "o nhap").await?;
        ops::press(page, "Control+A").await?;
        ops::press(page, "Delete").await?;
    }
    Ok(())
}

/// [`clear_ability_tag`] nhung loi -> `"error"` nhu Python `_clear_ability_tag` (`:2365-2367`) —
/// `select_mode` chi so sanh `!= "none"`.
async fn clear_ability_tag_lenient(page: &Page) -> String {
    match clear_ability_tag(page).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %first_line(&e.to_string(), ERR_SNIPPET_LONG), "không gỡ được thẻ kỹ năng");
            "error".to_string()
        }
    }
}

/// `:1957-1959`: `logs/mode_fail_<ts>.png` (tao thu muc neu thieu) -> duong dan da ghi.
async fn screenshot_mode_fail(page: &Page) -> Result<String, BrowserError> {
    tokio::fs::create_dir_all(LOGS_DIR).await?;
    let path = Path::new(LOGS_DIR).join(format!("mode_fail_{}.png", timestamp_utc()));
    page.save_screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .build(),
        &path,
    )
    .await
    .map_err(BrowserError::Cdp)?;
    Ok(path.display().to_string())
}

/// Port `_mode_unavailable_message` (`:1951-1967`): `raise_if_logged_out(page, "", "khi chọn chế độ")`
/// TRUOC (co the la `LoggedOut`), chup anh (loi -> `n/a`), `_BUTTONS_JS` (10 s, loi -> `[]`), URL.
async fn mode_unavailable_error(page: &Page) -> DolaError {
    if let Err(e) = session::raise_if_logged_out(page, "", "khi chọn chế độ").await {
        return e;
    }
    let shot = match screenshot_mode_fail(page).await {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!(error = %e, "khong chup duoc anh mode_fail");
            String::new()
        }
    };
    let seen: Vec<String> = tokio::time::timeout(
        BUTTONS_JS_TIMEOUT,
        ops::eval_fn::<Vec<String>>(page, BUTTONS_JS),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default();
    let url = ops::url(page).await.unwrap_or_default();
    DolaError::ModeUnavailable(mode_unavailable_message(&url, &seen, &shot))
}

/// `:2030-2049`: 2 vong doc nhan -> khop thi xong; khong thi bam nut, 700 ms, bam muc menu
/// (`FAST_ITEM_SRC`/`PRO_ITEM_SRC` trong `MENU_ITEM_SEL`, neu co), 1500 ms, dong menu, doc lai.
async fn switch_mode_rounds(page: &Page, btn: &str, target: Mode) -> Result<String, DolaError> {
    let item_src = if target == Mode::Fast {
        FAST_ITEM_SRC
    } else {
        PRO_ITEM_SRC
    };
    let item = finder_menu_item(item_src);
    let mut current = String::new();
    for _ in 0..MODE_ROUNDS {
        current = inner_text_wait(page, btn, MODE_TEXT_TIMEOUT, "nut che do").await?;
        if is_mode_match(&current, target) {
            return Ok(current);
        }
        click_wait(page, btn, MODE_CLICK_TIMEOUT, "nut che do").await?;
        tokio::time::sleep(MENU_SETTLE).await;
        if exists(page, &item).await? {
            click_wait(page, &item, MODE_CLICK_TIMEOUT, "muc menu che do").await?;
        }
        tokio::time::sleep(ITEM_SETTLE).await;
        close_open_menus(page).await?;
        current = inner_text_wait(page, btn, MODE_TEXT_TIMEOUT, "nut che do").await?;
        if is_mode_match(&current, target) {
            return Ok(current);
        }
    }
    Err(DolaError::ModeSwitch {
        want: mode_want(target),
        shows: current,
    })
}

/// Port `select_mode`/`_select_mode_locked` (`:1970-2050`) — tra NHAN hien tai cua nut (Python tra
/// `current`, caller in `Dola mode: {label}`). `mode` ProMax/Agent -> nut プロ (`:2475`).
///
/// Thu tu (dung Python):
/// 1. cookie banner (loi nuot) -> `_wait_mode_button(8 s)`;
/// 2. khong thay: [`clear_ability_tag`] != "none" -> xoa nhap (`DRAFT_BOX_SEL`, Ctrl+A, Delete; loi
///    nuot), ngu 800 ms, cho 5 s;
/// 3. van khong: dong menu, `reload` (loi chi warn), ngu 5 s, cho 15 s, go the lan nua, cho 5 s;
/// 4. van khong: [`DolaError::ModeUnavailable`] (sau `raise_if_logged_out`);
/// 5. co nut: 2 vong doc/bam ([`switch_mode_rounds`]); lech: [`DolaError::ModeSwitch`] (khong ma;
///    caller `Agent` tu nuot, `:2480`).
///
/// Khong port `_mode_lock_for` (khoa theo context cho wtabs — P2 moi tab mot Chrome rieng).
///
/// # Errors
/// `ModeUnavailable`, `ModeSwitch`, `LoggedOut` (tu `raise_if_logged_out`), `Browser` (CDP/timeout).
pub async fn select_mode(page: &Page, mode: Mode) -> Result<String, DolaError> {
    let target = target_mode(mode);
    if let Err(e) = dismiss_cookie_banner(page).await {
        tracing::debug!(error = %e, "cookie banner: bo qua");
    }
    let mut btn = wait_mode_button(page, MODE_BTN_WAIT).await?;
    if btn.is_none() {
        // 2026-09-17 (do that, wtabs): Dola mo lai trang voi composer video CON MO + nhap cu -> thanh
        // nut che do bi thay bang thanh composer. Go the ky nang + xoa nhap roi tim lai.
        let cleared = clear_ability_tag_lenient(page).await;
        if cleared != "none" {
            if let Err(e) = clear_draft(page).await {
                tracing::debug!(error = %e, "xoa nhap cu: bo qua");
            }
            tokio::time::sleep(CLEAR_DRAFT_SETTLE).await;
            btn = wait_mode_button(page, MODE_BTN_RETRY).await?;
            tracing::info!(
                cleared = %cleared,
                found = btn.is_some(),
                "composer video còn mở từ trước → đã gỡ thẻ; nút chế độ"
            );
        }
    }
    if btn.is_none() {
        // v0.54: lan mo profile dau trang chua ve thanh nut sau 8 s -> chua gui gi nen tai lai 1 lan.
        tracing::info!("Mode button not found in 8s -> Escape + reload once");
        close_open_menus(page).await?;
        if let Err(e) = ops::reload(page, Duration::ZERO).await {
            tracing::warn!(error = %first_line(&e.to_string(), ERR_SNIPPET_SHORT), "reload failed");
        }
        tokio::time::sleep(RELOAD_SETTLE).await;
        btn = wait_mode_button(page, MODE_BTN_AFTER_RELOAD).await?;
        if btn.is_none() && clear_ability_tag_lenient(page).await != "none" {
            // nhap + composer van duoc Dola khoi phuc sau reload
            tokio::time::sleep(CLEAR_DRAFT_SETTLE).await;
            btn = wait_mode_button(page, MODE_BTN_RETRY).await?;
        }
    }
    let Some(btn) = btn else {
        return Err(mode_unavailable_error(page).await);
    };
    let label = switch_mode_rounds(page, &btn, target).await?;
    tracing::info!(label = %label, mode = mode.as_str(), "Dola mode");
    Ok(label)
}

// ---------------------------------------------------------------------------------------------
// Mo composer Nhanh
// ---------------------------------------------------------------------------------------------

/// `:2057-2058`, `:2077-2078`: composer Nhanh dang mo <=> co nut thoi luong (`button, div, span`
/// khop `DUR_BTN_SRC`) hoac nut ti le (`button` khop `RATIO_BTN_SRC`).
async fn composer_open(page: &Page) -> Result<bool, BrowserError> {
    Ok(
        exists(page, &finder_menu_item_in("button, div, span", DUR_BTN_SRC)).await?
            || exists(page, &finder_button_text(RATIO_BTN_SRC)).await?,
    )
}

/// Mot lan bam 動画を作成 (`:2067-2080`): nut visible -> lan 1-2 bam chuot (`timeout=5000*attempt`),
/// lan 3 `e.click()` JS; khong visible -> `page.click("text=動画を作成")`; 1500 ms; `Ok(true)` khi
/// composer da mo HOAC la lan cuoi (Python `if opened or attempt == 3: return`).
async fn open_composer_attempt(page: &Page, attempt: u32) -> Result<bool, BrowserError> {
    let btn = finder_button_text(VIDEO_BTN_SRC);
    let timeout = COMPOSER_CLICK_BASE * attempt;
    if is_visible(page, &btn).await? {
        if attempt < COMPOSER_TRIES {
            click_wait(page, &btn, timeout, "nut 動画を作成").await?;
        } else {
            let _: serde_json::Value = ops::eval_json(page, &format!("({btn}).click()")).await?;
        }
    } else {
        click_wait(
            page,
            &finder_text_lax(UI_LABELS.video_btn),
            timeout,
            "text=動画を作成",
        )
        .await?;
    }
    tokio::time::sleep(COMPOSER_SETTLE).await;
    Ok(composer_open(page).await? || attempt == COMPOSER_TRIES)
}

/// Port `open_video_composer(page, "fast")` (`:2053-2089`): da mo (nut thoi luong/ti le/the
/// `動画を作成 ✕`) -> khong bam lai (bam lai se dong). Khong thi toi da 3 lan
/// ([`open_composer_attempt`]); giua cac lan: dong menu + `1500*attempt` ms. Het 3 lan (chi khi lan
/// 3 nem loi) -> `raise_if_logged_out` roi `ModeUnavailable("không mở được composer Nhanh ...")`.
///
/// # Errors
/// `ModeUnavailable`, `LoggedOut`, `Browser`.
pub async fn open_video_composer_fast(page: &Page) -> Result<(), DolaError> {
    if composer_open(page).await?
        || exists(page, &finder_menu_item_in("button, div", VIDEO_TAG_SRC)).await?
    {
        tracing::info!("Video composer already active, skipping toggle.");
        return Ok(());
    }
    // 2026-09-17 (do that, 3 profile cung luc): click 動画を作成 treo >5 s o lan tai trang dau.
    let mut last = String::new();
    for attempt in 1..=COMPOSER_TRIES {
        match open_composer_attempt(page, attempt).await {
            Ok(true) => return Ok(()),
            Ok(false) => last = "composer chưa mở sau khi bấm".to_string(),
            Err(e) => last = first_line(&e.to_string(), ERR_SNIPPET_LONG),
        }
        tracing::warn!(
            attempt,
            tries = COMPOSER_TRIES,
            last = %last,
            "mở composer Nhanh chưa được → đóng menu, đợi rồi thử lại"
        );
        close_open_menus(page).await?;
        tokio::time::sleep(COMPOSER_RETRY_BASE * attempt).await;
    }
    session::raise_if_logged_out(page, "", "khi mở composer Nhanh").await?;
    Err(DolaError::ModeUnavailable(format!(
        "không mở được composer Nhanh (nút 動画を作成) sau {COMPOSER_TRIES} lần ({last})"
    )))
}

// ---------------------------------------------------------------------------------------------
// Anh dinh kem cu
// ---------------------------------------------------------------------------------------------

/// `int(await page.evaluate(COUNT_ATTACH_JS) or 0)`.
async fn count_attach(page: &Page) -> Result<i64, BrowserError> {
    let v: Option<i64> = ops::eval_fn(page, COUNT_ATTACH_JS).await?;
    Ok(v.unwrap_or(0))
}

/// Mot the (`:811-829`): hover -> 250 ms -> voi tung goc `(dx, dy)` bam `(x+w-dx, y+dy)` -> 350 ms ->
/// so the giam -> `Ok(true)`. Khong hover/bam duoc (an/mat) hoac het 4 goc -> `Ok(false)`.
async fn remove_one_attachment(page: &Page, card: &str) -> Result<bool, BrowserError> {
    if !ops::hover_js(page, card).await? {
        return Ok(false);
    }
    tokio::time::sleep(ATTACH_HOVER_SETTLE).await;
    let before = count_attach(page).await?;
    for (dx, dy) in ATTACH_OFFSETS {
        if !ops::click_js_at_offset(page, card, dx, dy).await? {
            return Ok(false);
        }
        tokio::time::sleep(ATTACH_CLICK_SETTLE).await;
        if count_attach(page).await? < before {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Port `_clear_composer_attachments` (`:794-833`): Dola giu nhap anh dinh kem qua chat moi/tai
/// lai; nut ✕ KHONG co trong DOM — chi bam toa do goc tren phai thumbnail (`ATTACH_CARD_SEL`) moi
/// xoa. Tra so the da xoa. Dem ban dau loi -> `Ok(0)`; loi giua chung -> warn + dung (nhu Python);
/// dem cuoi loi -> `Err` (Python ngoai `try`).
///
/// # Errors
/// Loi CDP o lan dem cuoi.
pub async fn clear_composer_attachments(page: &Page) -> Result<usize, BrowserError> {
    let n = match count_attach(page).await {
        Ok(n) => n,
        Err(e) => {
            tracing::debug!(error = %e, "khong dem duoc anh dinh kem");
            return Ok(0);
        }
    };
    let Ok(n) = usize::try_from(n) else {
        return Ok(0);
    };
    if n == 0 {
        return Ok(0);
    }
    tracing::info!(n, "composer còn ảnh đính kèm cũ → xoá trước khi gửi");
    let card = format!(
        "document.querySelector({})",
        serde_json::to_string(ATTACH_CARD_SEL)?
    );
    let mut removed = 0_usize;
    for _ in 0..n + ATTACH_EXTRA_ROUNDS {
        if !exists(page, &card).await? {
            break;
        }
        match remove_one_attachment(page, &card).await {
            Ok(true) => removed += 1,
            Ok(false) => break,
            Err(e) => {
                tracing::warn!(error = %first_line(&e.to_string(), ERR_SNIPPET_SHORT), "xoá ảnh cũ lỗi");
                break;
            }
        }
    }
    let left = count_attach(page).await?;
    tracing::info!(removed, left, "đã xoá ảnh cũ");
    Ok(removed)
}

// ---------------------------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------------------------

/// `:2138-2149` — thu tu Python (xem doc module): `button` khop `MODEL_BTN_SRC` (JS `\b` ASCII —
/// thuc te KHONG khop "モデル 2.0高速") -> `get_by_text(label, exact=True)` cho tung
/// `UI_LABELS.model_current` (visible) -> `get_by_text("モデル")` (luon gan, co the khong ton tai).
async fn find_model_button(page: &Page) -> Result<String, BrowserError> {
    let by_re = finder_button_text(MODEL_BTN_SRC);
    if is_visible(page, &by_re).await? {
        return Ok(by_re);
    }
    for label in UI_LABELS.model_current {
        let f = finder_text_exact(label);
        if is_visible(page, &f).await? {
            return Ok(f);
        }
    }
    Ok(finder_text_lax(UI_LABELS.model_prefix))
}

async fn select_model_inner(page: &Page, model: Model) -> Result<(), BrowserError> {
    let btn = find_model_button(page).await?;
    // :2151-2156 — nut da hien dung phien ban (2.0/2.5/1.0) -> khong bam lai
    if is_visible(page, &btn).await? {
        let txt = inner_text(page, &btn).await?.unwrap_or_default();
        let ver = model_version(model);
        if !ver.is_empty() && txt.contains(ver) {
            tracing::debug!(model = model.as_str(), "nut model da dung phien ban");
            return Ok(());
        }
    }
    // :2158-2166
    if exists(page, &btn).await? {
        click_wait(page, &btn, MODEL_CLICK_TIMEOUT, "nut model").await?;
        tokio::time::sleep(MODEL_MENU_SETTLE).await;
        let options: &[&str] = MODEL_OPTIONS
            .iter()
            .find(|(m, _)| *m == model)
            .map_or(&[], |(_, o)| o);
        for option in options {
            let f = finder_text_lax(option);
            if is_visible(page, &f).await? {
                click_wait(page, &f, MODEL_CLICK_TIMEOUT, "option model").await?;
                tokio::time::sleep(MODEL_OPTION_SETTLE).await;
                tracing::info!(option, "đã chọn model");
                return Ok(());
            }
        }
        tracing::warn!(
            model = model.as_str(),
            "không thấy option model trong menu → giữ model hiện tại"
        );
    }
    Ok(())
}

/// Port `_select_model` (`:2136-2169`): nut model da chua so phien ban -> thoi; khong thi bam,
/// 500 ms, bam option DAU TIEN visible trong `MODEL_OPTIONS[model]` (`get_by_text(exact=False)`),
/// 500 ms. MOI loi deu nuot (warn) nhu Python `except Exception: print(...)` — ham nay hien khong
/// bao gio `Err`; giu chu ky theo brief de caller `?` duoc.
///
/// # Errors
/// Khong (xem tren).
pub async fn select_model(page: &Page, model: Model) -> Result<(), BrowserError> {
    if let Err(e) = select_model_inner(page, model).await {
        tracing::warn!(
            model = model.as_str(),
            error = %first_line(&e.to_string(), ERR_SNIPPET_LONG),
            "Failed to set model, keeping current"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Ti le
// ---------------------------------------------------------------------------------------------

/// Mot lan (`:2181-2201`): `Ok(None)` = xong; `Ok(Some(last))` = chua dung (`selector shows ...`);
/// `Err` = ngoai le (Python `except` -> dong menu, thu lai).
async fn select_ratio_attempt(
    page: &Page,
    btn: &str,
    want: &str,
) -> Result<Option<String>, BrowserError> {
    if is_visible(page, btn).await? {
        let cur = inner_text(page, btn).await?.unwrap_or_default();
        if cur.contains(want) {
            return Ok(None);
        }
        click_wait(page, btn, RATIO_CLICK_TIMEOUT, "nut ti le").await?;
    } else {
        click_wait(
            page,
            &finder_text_lax(UI_LABELS.ratio_btn),
            RATIO_CLICK_TIMEOUT,
            "text=比率",
        )
        .await?;
    }
    tokio::time::sleep(RATIO_MENU_SETTLE).await;
    let item = finder_menu_item_in(r#"[role="menuitem"]"#, &ratio_item_src(want));
    click_wait(page, &item, RATIO_CLICK_TIMEOUT, "menuitem ti le").await?;
    tokio::time::sleep(RATIO_ITEM_SETTLE).await;
    close_open_menus(page).await?;
    let cur = if exists(page, btn).await? {
        inner_text_wait(page, btn, RATIO_TEXT_TIMEOUT, "nut ti le").await?
    } else {
        String::new()
    };
    if cur.contains(want) {
        tracing::info!(ratio = %cur.replace('\n', " "), "ratio");
        return Ok(None);
    }
    Ok(Some(format!("selector shows {}", py_repr(&cur))))
}

/// Port `_select_ratio(page, ratio, tries=3)` (`:2176-2207`): nut `button` khop `RATIO_BTN_SRC`
/// da chua `16:9` -> xong; khong thi bam (hoac `text=比率`), 600 ms, `[role="menuitem"]` khop
/// `^\s*16:9(?:\b|\s|$)`, 400 ms, dong menu, doc lai. 3 lan (700 ms giua) that bai ->
/// `raise_if_logged_out` roi [`DolaError::RatioUnavailable`] (chua gui -> khong ton luot).
///
/// # Errors
/// `RatioUnavailable`, `LoggedOut`, `Browser` (chi khi `close_open_menus` trong nhanh loi hong).
pub async fn select_ratio(page: &Page, ratio: Ratio) -> Result<(), DolaError> {
    let want = ratio.as_str();
    let btn = finder_button_text(RATIO_BTN_SRC);
    let mut last = String::new();
    for attempt in 1..=RATIO_TRIES {
        match select_ratio_attempt(page, &btn, want).await {
            Ok(None) => return Ok(()),
            Ok(Some(l)) => last = l,
            Err(e) => {
                last = first_line(&e.to_string(), ERR_SNIPPET_SHORT);
                close_open_menus(page).await?;
            }
        }
        tracing::warn!(
            ratio = want,
            attempt,
            tries = RATIO_TRIES,
            last = %last,
            "chưa chọn được tỉ lệ"
        );
        tokio::time::sleep(RATIO_RETRY_PAUSE).await;
    }
    session::raise_if_logged_out(page, "", "khi chọn tỉ lệ").await?;
    Err(DolaError::RatioUnavailable(ratio_unavailable_msg(
        want, &last,
    )))
}

// ---------------------------------------------------------------------------------------------
// Thoi luong
// ---------------------------------------------------------------------------------------------

/// Ket qua doc nut thoi luong (`:1888-1909`).
enum DurRead {
    /// Nhan da khop trong 10 x 500 ms -> Python `return` NGAY (khong dong menu).
    Matched(String),
    /// Da bam nut + muc menu, doc lai nhan.
    Read(String),
    /// Khong co nut thoi luong (Python `current = ""`).
    NoButton,
}

async fn read_or_pick_duration(
    page: &Page,
    btn: &str,
    duration: u16,
) -> Result<DurRead, BrowserError> {
    // Cho toi da 5 s de DOM cap nhat nhan
    for _ in 0..DUR_POLLS {
        if exists(page, btn).await? {
            let current = inner_text_wait(page, btn, DUR_TEXT_TIMEOUT, "nut thoi luong").await?;
            if is_duration_match(&current, duration) {
                return Ok(DurRead::Matched(current));
            }
        }
        tokio::time::sleep(DUR_POLL).await;
    }
    if !exists(page, btn).await? {
        return Ok(DurRead::NoButton);
    }
    // Khong khop -> mo dropdown chon option co that
    click_wait(page, btn, DUR_CLICK_TIMEOUT, "nut thoi luong").await?;
    tokio::time::sleep(DUR_MENU_SETTLE).await;
    let item = finder_menu_item_in(DUR_MENU_ITEM_SEL, &dur_item_src(duration));
    if exists(page, &item).await? {
        click_wait(page, &item, DUR_CLICK_TIMEOUT, "muc thoi luong").await?;
    }
    tokio::time::sleep(DUR_ITEM_SETTLE).await;
    Ok(DurRead::Read(
        inner_text_wait(page, btn, DUR_TEXT_AFTER, "nut thoi luong").await?,
    ))
}

/// Port `select_fast_duration` (`:1879-1922`): nut `button, div, span` khop `DUR_BTN_SRC` ("10s",
/// "15 秒 (β)"); 10 x 500 ms cho nhan khop `duration` -> xong; khong thi bam, 600 ms, muc
/// `DUR_MENU_ITEM_SEL` khop `^\s*{d}s(?:\b|\s|$)`, 400 ms, doc lai; dong menu; van lech ->
/// [`DolaError::DurationUnavailable`] (`requested {d}s, selector shows {cur!r}`); ngoai le ->
/// dong menu + `cannot select {d}s in fast mode (...)`.
///
/// Ruling (NOTES §3.7): KHONG co nut thoi luong ma `duration == 10` -> `Ok` (body mac dinh 10 s);
/// Python nem `DURATION_UNAVAILABLE` ca cho 10 s.
///
/// # Errors
/// `DurationUnavailable`, `Browser` (chi khi `close_open_menus` loi).
pub async fn select_fast_duration(page: &Page, duration: u16) -> Result<(), DolaError> {
    let btn = finder_menu_item_in("button, div, span", DUR_BTN_SRC);
    let read = match read_or_pick_duration(page, &btn, duration).await {
        Ok(r) => r,
        Err(e) => {
            close_open_menus(page).await?;
            return Err(DolaError::DurationUnavailable(format!(
                "cannot select {duration}s in fast mode ({})",
                e.to_string()
                    .chars()
                    .take(ERR_SNIPPET_SHORT)
                    .collect::<String>()
            )));
        }
    };
    // `Some(nhan)` = da doc/bam (`:1907`), `None` = khong co nut (`:1909` `current = ""`)
    let current = match read {
        DurRead::Matched(current) => {
            tracing::info!(current = %current, duration, "Duration selector already matches");
            return Ok(());
        }
        DurRead::Read(current) => Some(current),
        DurRead::NoButton => None,
    };
    close_open_menus(page).await?;
    match current {
        Some(cur) if is_duration_match(&cur, duration) => {
            tracing::info!(current = %cur, "Duration selector");
            Ok(())
        }
        Some(cur) => Err(DolaError::DurationUnavailable(duration_mismatch_msg(
            duration, &cur,
        ))),
        None if duration == DEFAULT_DURATION_S => {
            tracing::info!(
                "khong co nut thoi luong, yeu cau 10 s -> giu mac dinh 10 s (ruling §3.7)"
            );
            Ok(())
        }
        None => Err(DolaError::DurationUnavailable(duration_mismatch_msg(
            duration, "",
        ))),
    }
}

// ---------------------------------------------------------------------------------------------
// Hook thoi luong (ADR-044)
// ---------------------------------------------------------------------------------------------

/// Port `duration_hook_state` (`:2257-2261`): `data-lvc-hook` cua `<html>`, `""` khi chua co.
/// "Context bi huy" (Dola dieu huong sau khi gui) -> `""` nhu Python `except: return ""`; loi CDP
/// khac -> `Err`.
///
/// # Errors
/// Loi CDP (khong phai loi context).
pub async fn hook_state(page: &Page) -> Result<String, BrowserError> {
    match ops::eval_json::<Option<String>>(
        page,
        "document.documentElement.getAttribute('data-lvc-hook')",
    )
    .await
    {
        Ok(v) => Ok(v.unwrap_or_default()),
        Err(e) if ops::context_gone(&e) => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// Port `install_duration_hook` (`:2249-2254`) + kiem `:2282-2283`: chay `DURATION_HOOK_JS` (thay
/// `__DUR__`) — chromiumoxide `Runtime.evaluate` DA o MAIN world nen KHONG can tiem `<script>` nhu
/// patchright — roi doc `data-lvc-hook`; phai la `installed:N` (hoac `patched:N` neu hook cu da
/// doi mot request) -> `Ok`; khac -> `DurationUnavailable("không cài được hook ...")`.
///
/// # Errors
/// `DurationUnavailable` khi trang thai khong dung; `Browser` (JS/CDP).
pub async fn install_duration_hook(page: &Page, duration: u16) -> Result<(), DolaError> {
    let _: serde_json::Value = ops::eval_json(page, &duration_hook_script(duration)).await?;
    let state = hook_state(page).await?;
    if state == format!("installed:{duration}") || state == format!("patched:{duration}") {
        tracing::info!(state = %state, "hook duration da cai (ADR-044)");
        return Ok(());
    }
    Err(DolaError::DurationUnavailable(format!(
        "không cài được hook {duration}s vào trang (state={})",
        py_repr(&state)
    )))
}

/// Vong kiem sau khi gui (`:2289-2295`): 20 x 250 ms cho `data-lvc-hook == "patched:N"` ->
/// `Ok(true)`; het -> `Ok(false)` (caller nem `DurationUnavailable` voi [`hook_state`] cuoi).
///
/// # Errors
/// Loi CDP (khong phai loi context).
pub async fn verify_hook_patched(page: &Page, duration: u16) -> Result<bool, BrowserError> {
    let want = format!("patched:{duration}");
    for _ in 0..HOOK_POLLS {
        if hook_state(page).await? == want {
            return Ok(true);
        }
        tokio::time::sleep(HOOK_POLL).await;
    }
    Ok(false)
}

// ---------------------------------------------------------------------------------------------
// Go & gui
// ---------------------------------------------------------------------------------------------

/// Port `_type_and_send` (`:2117-2133`) + `_enter_text` (`:1231-1239`): dong menu -> bam o soan
/// (`finder_composer`, cho toi 10 s; khong co -> `Timeout` nhu `box.click(timeout=10000)`) ->
/// `append`: `End`, khong: `Control+A` + `Delete` (session am tai dung page: chu cu con nguyen se
/// bi go chong -> Dola "Illegal request") -> `Input.insertText` MOT PHAT (`TYPE_PASTE_THRESHOLD=0`
/// -> luon `insert_text`) -> 600 ms -> `Enter`. Khong log noi dung, chi do dai.
///
/// # Errors
/// `Timeout` (khong co o soan), loi CDP.
pub async fn type_and_send(page: &Page, text: &str, append: bool) -> Result<(), BrowserError> {
    close_open_menus(page).await?;
    click_wait(page, finder_composer(), COMPOSER_CLICK_TIMEOUT, "o soan").await?;
    if append {
        ops::press(page, "End").await?;
    } else {
        ops::press(page, "Control+A").await?;
        ops::press(page, "Delete").await?;
    }
    ops::insert_text(page, text).await?;
    tokio::time::sleep(TYPE_SETTLE).await;
    ops::press(page, "Enter").await?;
    tracing::info!(len = text.chars().count(), append, "đã gõ và gửi prompt");
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The ky nang
// ---------------------------------------------------------------------------------------------

/// Port `_clear_ability_tag` (`:2349-2367`): khong co `ABILITY_TAG_SEL` va khong co `button, div,
/// span` khop `ABILITY_TAG_SRC` -> `"none"`; co -> `CLEAR_ABILITY_TAG_JS` (`cleared-svg` |
/// `cleared` | `no-close-button`), 400 ms, dem con lai de log. Loi -> `Err` (Python tra `"error"`;
/// [`select_mode`] tu quy ve `"error"`).
///
/// # Errors
/// Loi CDP/JS.
pub async fn clear_ability_tag(page: &Page) -> Result<String, BrowserError> {
    let by_text = js_count_text("button, div, span", ABILITY_TAG_SRC);
    if count_sel(page, ABILITY_TAG_SEL).await? == 0
        && ops::eval_json::<i64>(page, &by_text).await? == 0
    {
        return Ok("none".to_string());
    }
    let res: String = ops::eval_fn(page, CLEAR_ABILITY_TAG_JS).await?;
    tokio::time::sleep(ABILITY_CLEAR_SETTLE).await;
    let left =
        count_sel(page, ABILITY_TAG_SEL).await? + ops::eval_json::<i64>(page, &by_text).await?;
    tracing::info!(result = %res, left, "gỡ thẻ kỹ năng trong ô nhập");
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_core::{Mode, Model};

    /// `_is_mode_match` (`:1867-1871`): chuoi con, khong phan biet hoa/thuong, sau `strip()`;
    /// fast <-> (高速|Nhanh|Fast), con lai <-> (プロ|Pro).
    #[test]
    fn is_mode_match_theo_python() {
        for s in ["高速", " 高速\nNew ", "Nhanh\nMới", "FAST", "2.0 Fast"] {
            assert!(is_mode_match(s, Mode::Fast), "{s:?}");
            assert!(!is_mode_match(s, Mode::Pro), "{s:?}");
        }
        for s in ["プロ", "Pro >", "PRO", "pro\nNew"] {
            assert!(is_mode_match(s, Mode::Pro), "{s:?}");
            assert!(is_mode_match(s, Mode::ProMax), "{s:?}");
            assert!(is_mode_match(s, Mode::Agent), "{s:?}");
            assert!(!is_mode_match(s, Mode::Fast), "{s:?}");
        }
        assert!(!is_mode_match("", Mode::Fast));
        assert!(!is_mode_match("モデル", Mode::Pro));
        // Python re.search(re.escape("Pro"), "Professional", I) -> True (chuoi con) — giu nguyen
        assert!(is_mode_match("Professional", Mode::Pro));
    }

    /// `_is_duration_match` (`:1874-1876`): so DAU TIEN truoc `s`/`秒` == duration.
    #[test]
    fn is_duration_match_theo_python() {
        assert!(is_duration_match("10s", 10));
        assert!(is_duration_match(" 15 秒 (β)", 15));
        assert!(is_duration_match("30S", 30));
        assert!(is_duration_match("10 s", 10));
        assert!(!is_duration_match("10s", 15));
        assert!(!is_duration_match("", 10));
        assert!(!is_duration_match("10", 10));
        assert!(!is_duration_match("秒", 10));
        // so dau tien quyet dinh (re.search lay match dau)
        assert!(is_duration_match("10s / 15s", 10));
        assert!(!is_duration_match("10s / 15s", 15));
        // so 0 dau: int("010") == 10; chu so full-width: int("１０") == 10 (oracle Python)
        assert!(is_duration_match("010s", 10));
        assert!(is_duration_match("１０s", 10));
    }

    /// `_mode_unavailable_message` (`:1964-1967`): danh sach nut in kieu Python `['a', 'b']`,
    /// `screenshot n/a` khi khong chup duoc. KHONG co tien to `MODE_UNAVAILABLE: ` (Display cua
    /// `DolaError::ModeUnavailable` them).
    #[test]
    fn mode_unavailable_message_theo_python() {
        let seen = vec!["動画を作成".to_string(), "It's".to_string()];
        let m = mode_unavailable_message("https://www.dola.com/chat", &seen, "");
        assert_eq!(
            m,
            "Dola page shows no 高速/Nhanh/Fast/プロ/Pro mode button even after reload \
             (url https://www.dola.com/chat; buttons seen: ['動画を作成', \"It's\"]; screenshot n/a)"
        );
        let m2 = mode_unavailable_message("u", &[], r"logs\mode_fail_20260918_031500.png");
        assert!(
            m2.ends_with(r"buttons seen: []; screenshot logs\mode_fail_20260918_031500.png)"),
            "{m2}"
        );
        assert_eq!(
            DolaError::ModeUnavailable(m2.clone()).to_string(),
            format!("MODE_UNAVAILABLE: {m2}")
        );
    }

    /// `:2048` `want = "/".join(FAST_MODE_LABELS)` / `PRO_MODE_LABELS`; thong diep
    /// `f"Cannot switch Dola mode to {mode} ({want}); ..."` -> truong `want` cua `ModeSwitch`.
    #[test]
    fn mode_want_gom_ten_va_nhan() {
        assert_eq!(mode_want(Mode::Fast), "fast (高速/Nhanh/Fast)");
        assert_eq!(mode_want(Mode::Pro), "pro (プロ/Pro)");
        // target da chuan hoa: ProMax/Agent -> pro (:2475)
        assert_eq!(mode_want(Mode::ProMax), "pro (プロ/Pro)");
        assert_eq!(mode_want(Mode::Agent), "pro (プロ/Pro)");
        assert_eq!(target_mode(Mode::Agent), Mode::Pro);
        assert_eq!(target_mode(Mode::ProMax), Mode::Pro);
        assert_eq!(target_mode(Mode::Pro), Mode::Pro);
        assert_eq!(target_mode(Mode::Fast), Mode::Fast);
        let e = DolaError::ModeSwitch {
            want: mode_want(Mode::Fast),
            shows: "プロ".into(),
        };
        assert_eq!(
            e.to_string(),
            "Cannot switch Dola mode to fast (高速/Nhanh/Fast); button shows \"プロ\""
        );
    }

    /// `:2153` `ver = model_key.replace("seedance_v", "")`.
    #[test]
    fn model_version_tu_wire_str() {
        assert_eq!(model_version(Model::Seedance25), "2.5");
        assert_eq!(model_version(Model::Seedance20), "2.0");
        assert_eq!(model_version(Model::Seedance10), "1.0");
    }

    /// `{cur!r}` Python: xuong dong hien `\n` (2 ky tu), nhay don; co `'` -> nhay kep.
    #[test]
    fn py_repr_escape_xuong_dong() {
        assert_eq!(py_repr("比率\n16:9"), r"'比率\n16:9'");
        assert_eq!(py_repr("10s"), "'10s'");
        assert_eq!(py_repr(""), "''");
        assert_eq!(py_repr("It's"), "\"It's\"");
        assert_eq!(py_repr("a\tb\r"), r"'a\tb\r'");
    }

    /// Thong diep loi thoi luong/ti le khop Python (`:1918-1919`, `:2207`).
    #[test]
    fn thong_diep_duration_va_ratio() {
        assert_eq!(
            duration_mismatch_msg(15, "10s"),
            "requested 15s, selector shows '10s'"
        );
        assert_eq!(
            DolaError::DurationUnavailable(duration_mismatch_msg(15, "")).to_string(),
            "DURATION_UNAVAILABLE: requested 15s, selector shows ''"
        );
        assert_eq!(
            ratio_unavailable_msg("16:9", "selector shows '比率\\n9:16'"),
            "không chọn được tỉ lệ 16:9 trên giao diện Nhanh sau 3 lần (selector shows '比率\\n9:16')"
        );
        assert_eq!(first_line("dong 1\ndong 2", 4), "dong");
        assert_eq!(first_line("", 4), "");
        assert_eq!(first_line("日本語テキスト", 3), "日本語");
    }

    /// Hang thoi gian khop so Python (khong magic number).
    #[test]
    fn hang_thoi_gian_khop_python() {
        assert_eq!(MENU_ESC_SETTLE.as_millis(), 300); // :1851
        assert_eq!(COOKIE_SETTLE.as_millis(), 500); // :1992
        assert_eq!(MODE_BTN_WAIT.as_millis(), 8000); // :1995
        assert_eq!(MODE_BTN_RETRY.as_millis(), 5000); // :2011, :2025
        assert_eq!(MODE_BTN_AFTER_RELOAD.as_millis(), 15000); // :2023
        assert_eq!(ATTACHED_WAIT_CAP.as_millis(), 5000); // :1941
        assert_eq!(RELOAD_SETTLE.as_millis(), 5000); // :2022
        assert_eq!(MENU_SETTLE.as_millis(), 700); // :2036
        assert_eq!(ITEM_SETTLE.as_millis(), 1500); // :2041
        assert_eq!(COMPOSER_SETTLE.as_millis(), 1500); // :2076
        assert_eq!(RATIO_MENU_SETTLE.as_millis(), 600); // :2191
        assert_eq!(DUR_MENU_SETTLE.as_millis(), 600); // :1901
        assert_eq!(TYPE_SETTLE.as_millis(), 600); // :2132
        assert_eq!(HOOK_POLL.as_millis(), 250); // :2295
        assert_eq!(HOOK_POLLS, 20); // :2291
        assert_eq!(
            ATTACH_OFFSETS,
            [(4.0, 4.0), (6.0, 6.0), (2.0, 2.0), (8.0, 8.0)]
        ); // :819
        assert_eq!((COMPOSER_CLICK_BASE * 3).as_millis(), 15000); // :2070 5000*attempt
        assert_eq!((COMPOSER_RETRY_BASE * 2).as_millis(), 3000); // :2085 1500*attempt
    }

    /// Lich Gregorian cho ten file `mode_fail_<ts>.png` (oracle `datetime`).
    #[test]
    fn civil_from_days_va_timestamp() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19723), (2024, 1, 1));
        assert_eq!(civil_from_days(20714), (2026, 9, 18));
        assert_eq!(civil_from_days(11016), (2000, 2, 29));
        let ts = timestamp_utc();
        assert_eq!(ts.len(), 15, "{ts}");
        assert_eq!(ts.as_bytes()[8], b'_', "{ts}");
        assert!(ts.starts_with("20"), "{ts}");
    }

    /// Finder JS sinh ra hop le ve mat chuoi (chay that o smoke).
    #[test]
    fn js_helper_sinh_chuoi() {
        assert_eq!(js_exists("X"), "!!(X)");
        assert!(js_visible("X").contains("getBoundingClientRect"));
        assert!(js_inner_text("X").contains("innerText"));
        assert_eq!(
            js_count_sel("a[b='c']"),
            "document.querySelectorAll(\"a[b='c']\").length"
        );
        let c = js_count_text("button, div, span", ABILITY_TAG_SRC);
        assert!(c.contains("querySelectorAll(\"button, div, span\")"), "{c}");
        assert!(c.contains(&format!("/{ABILITY_TAG_SRC}/i.test(")), "{c}");
        assert!(c.ends_with(".length"), "{c}");
        let cands = mode_button_candidates();
        assert!(cands[0].contains("/(高速|Nhanh|Fast|プロ|Pro)/i"));
        assert!(cands[1].contains(r"/(?:^|\s|\n)(高速|Nhanh|Fast|プロ|Pro)/i"));
        assert_eq!(cands[2], MODE_HAS_TEXT_FINDER);
    }
}
