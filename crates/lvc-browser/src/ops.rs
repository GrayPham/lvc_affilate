//! Thao tac trang bang CDP THO tren `chromiumoxide::Page` (Task 5, P2). chromiumoxide 0.9 khong
//! co locator kieu Playwright, nen moi thao tac duoc ghep tu vai lenh CDP - tang nay la nen cho
//! `dola::*` (Task 6-12). Moi ham nhan `&Page`, loi -> [`BrowserError`].
//!
//! **MAIN world:** moi JS o day chay qua `Runtime.evaluate` cua chromiumoxide = the gioi CHINH
//! cua trang (cung `window` voi script cua Dola), KHAC Playwright `page.evaluate` (isolated
//! world). He qua: doc/ghi bien toan cuc cua trang duoc thang (hook 30s, dan anh khong can trick
//! `<script>` tag), nhung cung nghia la trang CO THE thay bien ta gan len `window` - dat ten co
//! tien to `__lvc` neu can de lai dau vet.
//!
//! Anh xa ham <-> lenh CDP:
//! | ham | CDP |
//! |---|---|
//! | [`eval_json`], [`eval_fn`], [`eval_with_arg`], [`user_agent`], [`wait_until`] | `Runtime.evaluate` (`awaitPromise`, `returnByValue`) |
//! | [`click_js`], [`click_js_at_offset`] | `Runtime.evaluate` (rect) + `Input.dispatchMouseEvent` mouseMoved -> mousePressed -> mouseReleased |
//! | [`hover_js`] | `Runtime.evaluate` (rect) + `Input.dispatchMouseEvent` mouseMoved (khong bam) |
//! | [`press`] | `Input.dispatchKeyEvent` (rawKeyDown/keyDown + keyUp, modifier bit Ctrl=2) |
//! | [`insert_text`] | `Input.insertText` |
//! | [`set_file_input`] | `DOM.getDocument` + `DOM.querySelector` + `DOM.resolveNode` (qua `Page::find_element`) + `DOM.setFileInputFiles` |
//! | [`delete_cookies`] | `Network.deleteCookies` |
//! | [`cookies`], [`cookie_header`] | `Network.getCookies` (`urls=[url]`) |
//! | [`url`] | trang thai frame chinh cua chromiumoxide (`Page.frameNavigated`) |
//! | [`reload`] | `Page.reload` + cho `Page.domContentEventFired` |
//! | [`goto`] | `Page.navigate` + cho `Page.domContentEventFired` |
//! | [`count_responses`] | `Network.enable` + su kien `Network.responseReceived` |
//! | [`frame_urls`] | `Page.getFrameTree` (de quy) |
//!
//! KHONG dung `Page::authenticate`/`enable_request_intercept` (chung tat cache). KHONG log gia
//! tri cookie/URL co chu ky o bat ky muc nao.

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams, DispatchMouseEventType,
    InsertTextParams, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::network::{
    Cookie as CdpCookie, DeleteCookiesParams, EnableParams as NetworkEnableParams,
    EventResponseReceived, GetCookiesParams,
};
use chromiumoxide::cdp::browser_protocol::page::{
    EventDomContentEventFired, FrameTree, GetFrameTreeParams, NavigateParams, ReloadParams,
};
use chromiumoxide::cdp::js_protocol::runtime::{EvaluateParams, ExceptionDetails};
use chromiumoxide::error::CdpError;
use chromiumoxide::keys::{self, KeyDefinition};
use futures::StreamExt;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::task::JoinHandle;

use crate::{BrowserError, url_host_path};

/// `reload()`/`goto()`: thoi gian toi da cho `Page.domContentEventFired` (Python
/// `page.reload(wait_until="domcontentloaded", timeout=60000)` / `page.goto(..., timeout=60000)`).
pub const RELOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// Bit modifier cua `Input.dispatchKeyEvent`: Alt=1, Ctrl=2, Meta=4, Shift=8.
const MOD_ALT: i64 = 1;
const MOD_CTRL: i64 = 2;
const MOD_META: i64 = 4;
const MOD_SHIFT: i64 = 8;

/// `CdpError::JavascriptException` -> [`BrowserError::Js`] voi thong diep doc duoc; con lai giu
/// nguyen `BrowserError::Cdp`.
fn map_cdp(e: CdpError) -> BrowserError {
    match e {
        CdpError::JavascriptException(ex) => BrowserError::Js(exception_text(&ex)),
        // Task 13: qua `From` de `ChannelSendError` → `Disconnected` (khong `Cdp` chung chung).
        other => BrowserError::from(other),
    }
}

/// Dong dau cua `exception.description` (vd `TypeError: Cannot read properties of null`) hoac
/// `text` (`Uncaught`) khi khong co description.
fn exception_text(ex: &ExceptionDetails) -> String {
    ex.exception
        .as_ref()
        .and_then(|o| o.description.as_deref())
        .and_then(|d| d.lines().next())
        .filter(|l| !l.trim().is_empty())
        .map_or_else(|| ex.text.clone(), str::to_string)
}

/// Loi tham so builder (chuoi) cua chromiumoxide -> `BrowserError::Cdp`.
fn bad_params(e: String) -> BrowserError {
    BrowserError::Cdp(CdpError::msg(e))
}

/// `Runtime.evaluate` bieu thuc `js` (MAIN world), `awaitPromise` + `returnByValue`. `undefined`
/// (khong co `value`) -> `Value::Null` thay vi loi "No value found" cua chromiumoxide.
async fn eval_value(page: &Page, js: &str) -> Result<serde_json::Value, BrowserError> {
    let params = EvaluateParams::builder()
        .expression(js)
        .await_promise(true)
        .return_by_value(true)
        .build()
        .map_err(bad_params)?;
    let res = page.evaluate(params).await.map_err(map_cdp)?;
    Ok(res.value().cloned().unwrap_or(serde_json::Value::Null))
}

/// Chay bieu thuc JS (KHONG phai ham) trong MAIN world va deserialize ket qua JSON.
/// Promise duoc await; `undefined` -> `null` (dung `Option<T>`/`serde_json::Value` de nhan).
///
/// # Errors
/// `BrowserError::Js` khi JS nem ngoai le; `Json` khi ket qua khong khop `T`; `Cdp` khac.
pub async fn eval_json<T: DeserializeOwned>(page: &Page, js: &str) -> Result<T, BrowserError> {
    let v = eval_value(page, js).await?;
    Ok(serde_json::from_value(v)?)
}

/// Goi ham JS KHONG tham so `js_fn` (`() => ...`) — khop Python `page.evaluate(js)` khi `js` la
/// ham (Playwright tu goi); ket qua/Promise nhu [`eval_json`]. Dung cho moi hang `*_JS` dang ham
/// trong `dola::labels` (`DOLA_LANG_JS`, `MODE_BTN_JS`, `SESSION_JS`...).
///
/// # Errors
/// Nhu [`eval_json`].
pub async fn eval_fn<T: DeserializeOwned>(page: &Page, js_fn: &str) -> Result<T, BrowserError> {
    eval_json(page, &format!("({js_fn})()")).await
}

/// Goi `(js_fn)(arg)` voi `arg` serialize JSON (khop Python `page.evaluate(js, arg)`); `js_fn`
/// la bieu thuc ham (`(x) => ...` hoac `function (x) {...}`), ket qua/Promise nhu [`eval_json`].
///
/// # Errors
/// Nhu [`eval_json`]; them `Json` neu `arg` khong serialize duoc.
pub async fn eval_with_arg<T: DeserializeOwned, A: Serialize + ?Sized>(
    page: &Page,
    js_fn: &str,
    arg: &A,
) -> Result<T, BrowserError> {
    // JSON la tap con cua bieu thuc JS (ke ca U+2028/2029 trong chuoi, ES2019) -> ghep thang.
    let json = serde_json::to_string(arg)?;
    eval_json(page, &format!("({js_fn})({json})")).await
}

/// Rect (CSS px, toa do viewport frame chinh) cua phan tu `finder_js` tra ve.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// `finder_js` -> `Element|null`; cuon vao giua viewport roi lay `getBoundingClientRect()`.
/// `None` khi khong tim thay HOAC rect 0x0 (vd `display:none`, chua render) - dung chung cho
/// [`click_js`], [`click_js_at_offset`], [`hover_js`].
async fn visible_rect(page: &Page, finder_js: &str) -> Result<Option<Rect>, BrowserError> {
    let js = format!(
        "(()=>{{const e=({finder_js}); if(!e) return null; \
         e.scrollIntoView({{block:'center',inline:'nearest',behavior:'instant'}}); \
         const r=e.getBoundingClientRect(); \
         return {{x:r.x,y:r.y,w:r.width,h:r.height}};}})()"
    );
    let v = eval_value(page, &js).await?;
    if v.is_null() {
        return Ok(None);
    }
    let r: Rect = serde_json::from_value(v)?;
    if r.w <= 0.0 || r.h <= 0.0 {
        return Ok(None);
    }
    Ok(Some(r))
}

/// `Input.dispatchMouseEvent mouseMoved` toi `(x, y)` - Blink cap nhat `:hover`/`mouseenter`.
async fn move_mouse(page: &Page, x: f64, y: f64) -> Result<(), BrowserError> {
    let moved = DispatchMouseEventParams::builder()
        .r#type(DispatchMouseEventType::MouseMoved)
        .x(x)
        .y(y)
        .build()
        .map_err(bad_params)?;
    page.execute(moved).await.map_err(map_cdp)?;
    Ok(())
}

/// Chuot THAT qua CDP: `mouseMoved` -> `mousePressed`(trai, `clickCount` 1) -> `mouseReleased`.
async fn click_at(page: &Page, x: f64, y: f64) -> Result<(), BrowserError> {
    move_mouse(page, x, y).await?;
    for kind in [
        DispatchMouseEventType::MousePressed,
        DispatchMouseEventType::MouseReleased,
    ] {
        let ev = DispatchMouseEventParams::builder()
            .r#type(kind)
            .x(x)
            .y(y)
            .button(MouseButton::Left)
            .click_count(1)
            .build()
            .map_err(bad_params)?;
        page.execute(ev).await.map_err(map_cdp)?;
    }
    Ok(())
}

/// Bam CHUOT THAT vao tam phan tu ma bieu thuc `finder_js` tra ve (`Element|null`) - KHONG
/// dung `element.click()` JS (React/tiptap cua Dola phan biet duoc). Cuon vao giua viewport
/// truoc. Tra `Ok(false)` khi khong tim thay hoac rect 0x0 (an), `Ok(true)` khi da bam.
///
/// KHAC Playwright `locator.click()` (review Task 5 M-1): KHONG kiem tra actionability - khong
/// doi enabled/stable, khong `elementFromPoint` de chac tam rect khong bi modal/toast che;
/// `Ok(true)` chi nghia la "da gui 3 su kien chuot toi tam rect". Task 6-9 tu kiem ket qua
/// (vd `wait_until` menu mo) nhu Python van lam sau moi lan bam.
///
/// # Errors
/// Loi JS trong `finder_js` (`Js`) hoac loi CDP.
pub async fn click_js(page: &Page, finder_js: &str) -> Result<bool, BrowserError> {
    let Some(r) = visible_rect(page, finder_js).await? else {
        return Ok(false);
    };
    click_at(page, r.x + r.w / 2.0, r.y + r.h / 2.0).await?;
    Ok(true)
}

/// Nhu [`click_js`] nhung bam tai `(phai - dx_from_right, tren + dy)` cua rect - cho nut x cua
/// the anh dinh kem, vốn KHONG co trong DOM (Python `_clear_composer_attachments`,
/// `video_worker_ui.py:794-833`: `box.x + box.width - dx, box.y + dy`).
///
/// **Task 7 PHAI goi [`hover_js`] roi ngu 250 ms TRUOC ham nay** (Python :812-813 `th.hover()` +
/// `wait_for_timeout(250)` roi moi `mouse.click` :820): nut x chi duoc React ve ra sau khi hover,
/// con `mouseMoved` -> `mousePressed` o day di lien nhau, khong kip render (smoke
/// `click_js_at_offset_bam_goc_phai_tren_the_dinh_kem` chung minh bam thang thi truot).
///
/// # Errors
/// Nhu [`click_js`].
pub async fn click_js_at_offset(
    page: &Page,
    finder_js: &str,
    dx_from_right: f64,
    dy: f64,
) -> Result<bool, BrowserError> {
    let Some(r) = visible_rect(page, finder_js).await? else {
        return Ok(false);
    };
    click_at(page, r.x + r.w - dx_from_right, r.y + dy).await?;
    Ok(true)
}

/// Dua chuot toi tam phan tu (`Input.dispatchMouseEvent mouseMoved`, Playwright `hover`) - KHONG
/// bam. Blink cap nhat `:hover` + phat `mouseenter`/`mouseover` nhu chuot that. Tra `Ok(false)`
/// khi khong tim thay hoac rect 0x0. Dung truoc [`click_js_at_offset`] (xem doc o do).
///
/// # Errors
/// Nhu [`click_js`].
pub async fn hover_js(page: &Page, finder_js: &str) -> Result<bool, BrowserError> {
    let Some(r) = visible_rect(page, finder_js).await? else {
        return Ok(false);
    };
    move_mouse(page, r.x + r.w / 2.0, r.y + r.h / 2.0).await?;
    Ok(true)
}

/// Tra dinh nghia phim theo bang US cua chromiumoxide (`keys.rs`, chep tu Puppeteer); nhan
/// bi danh `Ctrl`/`Cmd`, va ky tu don khong khop thi thu hoa/thuong.
fn key_def(name: &str) -> Result<&'static KeyDefinition, BrowserError> {
    let canonical = match name {
        "Ctrl" => "Control",
        "Cmd" | "Command" => "Meta",
        "Esc" => "Escape",
        other => other,
    };
    keys::get_key_definition(canonical)
        .or_else(|| {
            let mut chars = canonical.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            keys::get_key_definition(c.to_ascii_lowercase().to_string())
                .or_else(|| keys::get_key_definition(c.to_ascii_uppercase().to_string()))
        })
        .ok_or_else(|| BrowserError::Cdp(CdpError::msg(format!("phim khong ho tro: {name:?}"))))
}

fn modifier_bit(def: &KeyDefinition) -> i64 {
    match def.key {
        "Alt" => MOD_ALT,
        "Control" => MOD_CTRL,
        "Meta" => MOD_META,
        "Shift" => MOD_SHIFT,
        _ => 0,
    }
}

/// `Input.dispatchKeyEvent`. `text` = `Some` -> `keyDown` (sinh keypress/char), `None` ->
/// `rawKeyDown` (Puppeteer/Playwright lam vay). Luon kem `key`/`code`/`windowsVirtualKeyCode`.
async fn dispatch_key(
    page: &Page,
    down: bool,
    def: &KeyDefinition,
    modifiers: i64,
    text: Option<&str>,
) -> Result<(), BrowserError> {
    let kind = match (down, text) {
        (true, Some(_)) => DispatchKeyEventType::KeyDown,
        (true, None) => DispatchKeyEventType::RawKeyDown,
        (false, _) => DispatchKeyEventType::KeyUp,
    };
    let mut b = DispatchKeyEventParams::builder()
        .r#type(kind)
        .modifiers(modifiers)
        .key(def.key)
        .code(def.code)
        .windows_virtual_key_code(def.key_code)
        .native_virtual_key_code(def.key_code);
    // Review Task 5 I-1: KHONG dung let-chain (`if down && let Some(t) = text`) - can rustc 1.88,
    // workspace ghim rust-version 1.85.
    if down {
        if let Some(t) = text {
            b = b.text(t).unmodified_text(t);
        }
    }
    page.execute(b.build().map_err(bad_params)?)
        .await
        .map_err(map_cdp)?;
    Ok(())
}

/// Nhan phim nhu Playwright `keyboard.press`: `"Enter"`, `"Delete"`, `"End"`, `"Escape"`,
/// `"Control+A"`... Modifier (Control/Shift/Alt/Meta) duoc nhan truoc (rawKeyDown, bit
/// modifier tich luy), phim chinh keyDown+keyUp, roi nha modifier theo thu tu nguoc. Co
/// Ctrl/Alt/Meta thi phim chinh KHONG kem `text` (khong chen ky tu) - `Control+A` = chon tat ca.
///
/// Gioi han (review Task 5 M-9): `"Shift+a"` gui `text` chu thuong (Playwright gui ban shifted
/// `"A"`); ngoai 5 phim Python dung (`Enter`/`Delete`/`End`/`Escape`/`Control+A`) - chua can.
///
/// # Errors
/// Ten phim khong co trong bang US (`Cdp` voi thong diep "phim khong ho tro") hoac loi CDP.
pub async fn press(page: &Page, key: &str) -> Result<(), BrowserError> {
    let mut parts: Vec<&str> = key.split('+').collect();
    let main = parts
        .pop()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| BrowserError::Cdp(CdpError::msg(format!("phim rong: {key:?}"))))?;
    let main_def = key_def(main)?;
    let mut held: Vec<&'static KeyDefinition> = Vec::with_capacity(parts.len());
    let mut modifiers = 0_i64;
    for m in parts {
        let def = key_def(m)?;
        let bit = modifier_bit(def);
        if bit == 0 {
            return Err(BrowserError::Cdp(CdpError::msg(format!(
                "{m:?} khong phai modifier (Control/Shift/Alt/Meta) trong {key:?}"
            ))));
        }
        modifiers |= bit;
        dispatch_key(page, true, def, modifiers, None).await?;
        held.push(def);
    }
    // Playwright: chi con Shift (hoac khong modifier) moi sinh text; Ctrl/Alt/Meta -> text rong.
    let text = if modifiers & !MOD_SHIFT == 0 {
        main_def
            .text
            .or_else(|| (main_def.key.chars().count() == 1).then_some(main_def.key))
    } else {
        None
    };
    dispatch_key(page, true, main_def, modifiers, text).await?;
    dispatch_key(page, false, main_def, modifiers, None).await?;
    for def in held.iter().rev() {
        modifiers &= !modifier_bit(def);
        dispatch_key(page, false, def, modifiers, None).await?;
    }
    Ok(())
}

/// `Input.insertText` - dan ca chuoi MOT PHAT vao phan tu dang focus (Python `TYPE_PASTE_THRESHOLD=0`
/// -> luon `keyboard.insert_text`; do 2026-09-07: 3759 ky tu vao tiptap mat 0.06s). Khong sinh
/// keydown, `\n` KHONG phai Enter (khong gui). Caller phai focus truoc (vd [`click_js`] o textarea).
///
/// # Errors
/// Loi CDP.
pub async fn insert_text(page: &Page, text: &str) -> Result<(), BrowserError> {
    page.execute(InsertTextParams::new(text))
        .await
        .map_err(map_cdp)?;
    Ok(())
}

/// `DOM.setFileInputFiles` len `<input type=file>` dau tien khop `css` (Playwright
/// `set_input_files`); duong dan duoc ep TUYET DOI (Chrome yeu cau). File phai ton tai - kiem
/// TRUOC khi goi CDP (review M-2: Chrome nhan path treo va chi hong o buoc upload sau, kho chan
/// doan; Playwright cung bao loi phia client). `paths` rong -> loi CDP (chromiumoxide bo truong
/// `files` khi rong; Python khong co ca "xoa file").
///
/// # Errors
/// `Io(NotFound)` khi mot path khong phai file; khong co phan tu khop `css` (`Cdp`); loi CDP.
pub async fn set_file_input(page: &Page, css: &str, paths: &[PathBuf]) -> Result<(), BrowserError> {
    let mut files = Vec::with_capacity(paths.len());
    for p in paths {
        if !p.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("file dinh kem khong ton tai: {}", p.display()),
            )
            .into());
        }
        files.push(std::path::absolute(p)?.to_string_lossy().into_owned());
    }
    let el = page.find_element(css).await.map_err(map_cdp)?;
    let params = SetFileInputFilesParams::builder()
        .files(files)
        .object_id(el.remote_object_id.clone())
        .build()
        .map_err(bad_params)?;
    page.execute(params).await.map_err(map_cdp)?;
    Ok(())
}

/// `Network.deleteCookies` tung ten voi `domain` CHINH XAC (Dola: goi 2 lan cho `.dola.com` va
/// `www.dola.com`). Ten khong ton tai -> khong loi.
///
/// # Errors
/// Loi CDP.
pub async fn delete_cookies(page: &Page, domain: &str, names: &[&str]) -> Result<(), BrowserError> {
    for name in names {
        let params = DeleteCookiesParams::builder()
            .name(*name)
            .domain(domain)
            .build()
            .map_err(bad_params)?;
        page.execute(params).await.map_err(map_cdp)?;
    }
    Ok(())
}

/// Cookie doc qua `Network.getCookies` — chi 3 truong ma lop Dola can (Python
/// `context.cookies(url)` tra dict, `cookie_value(cookies, name)` doc `name`/`value`). `Debug` CHE
/// `value` (chinh sach: khong bao gio log gia tri cookie).
#[derive(Clone, PartialEq, Eq)]
pub struct CookieKv {
    pub domain: String,
    pub name: String,
    pub value: String,
}

impl CookieKv {
    #[must_use]
    pub fn new(domain: &str, name: &str, value: &str) -> Self {
        Self {
            domain: domain.to_string(),
            name: name.to_string(),
            value: value.to_string(),
        }
    }
}

impl From<CdpCookie> for CookieKv {
    fn from(c: CdpCookie) -> Self {
        Self {
            domain: c.domain,
            name: c.name,
            value: c.value,
        }
    }
}

impl fmt::Debug for CookieKv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieKv")
            .field("domain", &self.domain)
            .field("name", &self.name)
            .field("value", &"***")
            .finish()
    }
}

/// `Network.getCookies(urls=[url])` -> danh sach [`CookieKv`] (Python
/// `context.cookies("https://www.dola.com")`, `browser.py:358`, `video_worker_ui.py:1178`). Gioi han theo `url` chu KHONG lay
/// cookie cua tab hien tai (co the la trang OAuth/loi sau redirect — review P1 I-2). KHONG log
/// gia tri.
///
/// # Errors
/// Loi CDP.
pub async fn cookies(page: &Page, url: &str) -> Result<Vec<CookieKv>, BrowserError> {
    let raw = page
        .execute(
            GetCookiesParams::builder()
                .urls(vec![url.to_string()])
                .build(),
        )
        .await
        .map_err(map_cdp)?
        .result
        .cookies;
    Ok(raw.into_iter().map(CookieKv::from).collect())
}

/// [`cookies`] gop thanh header `Cookie` dang `a=b; c=d` (cho reqwest, Task 10). KHONG log gia tri.
///
/// # Errors
/// Loi CDP.
pub async fn cookie_header(page: &Page, url: &str) -> Result<String, BrowserError> {
    Ok(cookies(page, url)
        .await?
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; "))
}

/// `navigator.userAgent` THAT cua trang (khong hardcode) - dung cho reqwest o Task 10.
///
/// # Errors
/// Nhu [`eval_json`].
pub async fn user_agent(page: &Page) -> Result<String, BrowserError> {
    eval_json(page, "navigator.userAgent").await
}

/// URL frame chinh theo chromiumoxide (cap nhat qua `Page.frameNavigated`); rong neu chua biet.
///
/// # Errors
/// Loi CDP.
pub async fn url(page: &Page) -> Result<String, BrowserError> {
    Ok(page.url().await.map_err(map_cdp)?.unwrap_or_default())
}

/// Truthiness JS cho ket qua JSON cua `pred_js` (`undefined`/`null`/`0`/`""`/`false` -> false).
fn truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => true,
    }
}

/// Loi "context bi huy" khi trang tu dieu huong giua chung (Dola tu tai lai khi dong bo ngon
/// ngu/phien - Python `_eval_retry`, `video_worker_ui.py:1036-1052`): coi nhu `false`, cho tiep.
pub(crate) fn context_gone(e: &BrowserError) -> bool {
    let s = e.to_string();
    s.contains("context was destroyed")
        || s.contains("Cannot find context")
        || s.contains("target navigated or closed")
}

/// Danh gia bieu thuc `pred_js` moi `every` cho toi khi truthy (`Ok(true)`) hoac het `timeout`
/// (`Ok(false)`); danh gia it nhat 1 lan. "Execution context was destroyed"/"Cannot find
/// context" (trang tu tai lai) -> coi false va tiep tuc; ngoai le JS khac -> `Err`.
///
/// # Errors
/// `Js` khi `pred_js` nem ngoai le (khong phai loi context), `Cdp` khac.
pub async fn wait_until(
    page: &Page,
    pred_js: &str,
    timeout: Duration,
    every: Duration,
) -> Result<bool, BrowserError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match eval_value(page, pred_js).await {
            Ok(v) if truthy(&v) => return Ok(true),
            Ok(_) => {}
            Err(e) if context_gone(&e) => {}
            Err(e) => return Err(e),
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(every.min(deadline - now)).await;
    }
}

/// `Page.reload` roi cho `Page.domContentEventFired` (toi da [`RELOAD_TIMEOUT`]), sau do ngu
/// `settle` - khop Python `page.reload(wait_until="domcontentloaded", timeout=60000)` +
/// `wait_for_timeout(...)` (`video_worker_ui.py:1158`, `:2019`). Cho `DOMContentLoaded` chu KHONG
/// cho `load` (ruling review Task 5, deviation 4): SPA nang co the `load` muon hon DCL nhieu giay
/// hoac khong bao gio -> Python thanh cong ma ta lai Timeout. Dang ky nghe su kien TRUOC khi gui
/// reload de khong lo.
///
/// # Errors
/// `Timeout` khi khong thay `DOMContentLoaded` trong 60s (Python cung raise - ca hai cho goi deu
/// try/except, caller tu quyet bo qua), `Cdp`.
pub async fn reload(page: &Page, settle: Duration) -> Result<(), BrowserError> {
    let mut dcl = page
        .event_listener::<EventDomContentEventFired>()
        .await
        .map_err(map_cdp)?;
    page.execute(ReloadParams::default())
        .await
        .map_err(map_cdp)?;
    match tokio::time::timeout(RELOAD_TIMEOUT, dcl.next()).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(BrowserError::Timeout(
                "reload: luong su kien Page.domContentEventFired dong truoc khi trang tai xong"
                    .into(),
            ));
        }
        Err(_elapsed) => {
            return Err(BrowserError::Timeout(format!(
                "reload: khong nhan Page.domContentEventFired trong {RELOAD_TIMEOUT:?}"
            )));
        }
    }
    tokio::time::sleep(settle).await;
    Ok(())
}

/// `Page.navigate` toi `url` roi cho `Page.domContentEventFired` (toi da [`RELOAD_TIMEOUT`]), sau
/// do ngu `settle` — khop Python `page.goto(url, wait_until="domcontentloaded", timeout=60000)` +
/// `wait_for_timeout(...)` (`video_worker_ui.py:1195-1196`, `:2466-2467`). Khac
/// `BrowserDriver::goto` (cho `load` qua `wait_for_navigation`, can `ChromiumSession`): ham nay
/// chi can `&Page` va dung o `DOMContentLoaded` nhu Python (ly do: xem [`reload`]).
///
/// # Errors
/// `Navigation` khi Chrome bao `errorText` (vd `net::ERR_NAME_NOT_RESOLVED`), `Timeout` khi khong
/// thay `DOMContentLoaded` trong 60 s, `Cdp` khac.
pub async fn goto(page: &Page, url: &str, settle: Duration) -> Result<(), BrowserError> {
    let mut dcl = page
        .event_listener::<EventDomContentEventFired>()
        .await
        .map_err(map_cdp)?;
    navigate(page, url).await?;
    match tokio::time::timeout(RELOAD_TIMEOUT, dcl.next()).await {
        Ok(Some(_)) => {}
        // Review T13 M-1: thong diep loi chi mang host/path (khong query — URL co chu ky).
        Ok(None) => {
            return Err(BrowserError::Timeout(format!(
                "goto {}: luong su kien Page.domContentEventFired dong truoc khi trang tai xong",
                url_host_path(url)
            )));
        }
        Err(_elapsed) => {
            return Err(BrowserError::Timeout(format!(
                "goto {}: khong nhan Page.domContentEventFired trong {RELOAD_TIMEOUT:?}",
                url_host_path(url)
            )));
        }
    }
    tokio::time::sleep(settle).await;
    Ok(())
}

/// Task 13: `Page.navigate` THO — `NavigateReturns.error_text` (vd `net::ERR_CONNECTION_REFUSED`,
/// `net::ERR_PROXY_CONNECTION_FAILED`) → [`BrowserError::Navigation`] voi URL DA CAT query
/// ([`url_host_path`]); loi CDP khac qua `From` (`ChannelSendError` → `Disconnected`). Khong dung
/// `Page::goto` cua chromiumoxide vi no gop `error_text` vao `CdpError::ChromeMessage` (mat phan
/// loai). chromiumoxide chi tra phan hoi `Page.navigate` SAU khi khung chinh `load` xong (hoac
/// `request_timeout`), nen ham nay da bao ham "cho tai" o muc `load`.
///
/// # Errors
/// `Navigation` (errorText), `Disconnected`, `Cdp`.
pub async fn navigate(page: &Page, url: &str) -> Result<(), BrowserError> {
    let resp = page.execute(NavigateParams::new(url)).await?;
    match resp.result.error_text {
        Some(net_error) => Err(BrowserError::Navigation {
            url_host_path: url_host_path(url),
            net_error,
        }),
        None => Ok(()),
    }
}

/// Bo dem `Network.responseReceived` (Python `page.on("response")` trong
/// `attach_reference_images`): giu `(url, status)`; drop -> ngung nghe. `Debug` chi in so
/// luong (URL upload cua Dola co chu ky - khong in).
pub struct ResponseCounter {
    seen: Arc<Mutex<Vec<(String, u16)>>>,
    task: JoinHandle<()>,
}

impl ResponseCounter {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(String, u16)>> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// So phan hoi 2xx co URL chua `url_part` (vd `"/upload/v1/"`).
    #[must_use]
    pub fn count_2xx(&self, url_part: &str) -> usize {
        self.lock()
            .iter()
            .filter(|(u, st)| u.contains(url_part) && (200..300).contains(st))
            .count()
    }

    /// Ban sao moi `(url, status)` da thay - de chan doan; KHONG log ra ngoai.
    #[must_use]
    pub fn snapshot(&self) -> Vec<(String, u16)> {
        self.lock().clone()
    }
}

impl fmt::Debug for ResponseCounter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseCounter")
            .field("seen", &self.lock().len())
            .finish_non_exhaustive()
    }
}

impl Drop for ResponseCounter {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// `Network.enable` + nghe `Network.responseReceived` tu luc goi; tra bo dem. Goi TRUOC hanh
/// dong can dem (dang ky listener di cung kenh lenh CDP nen luon xong truoc lenh ke tiep).
///
/// # Errors
/// Loi CDP.
pub async fn count_responses(page: &Page) -> Result<ResponseCounter, BrowserError> {
    page.execute(NetworkEnableParams::default())
        .await
        .map_err(map_cdp)?;
    let mut events = page
        .event_listener::<EventResponseReceived>()
        .await
        .map_err(map_cdp)?;
    let seen: Arc<Mutex<Vec<(String, u16)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let task = tokio::spawn(async move {
        while let Some(ev) = events.next().await {
            let status = u16::try_from(ev.response.status).unwrap_or(0);
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((ev.response.url.clone(), status));
        }
    });
    Ok(ResponseCounter { seen, task })
}

fn collect_frame_urls(t: &FrameTree, out: &mut Vec<String>) {
    let mut u = t.frame.url.clone();
    if let Some(frag) = &t.frame.url_fragment {
        u.push_str(frag);
    }
    out.push(u);
    for c in t.child_frames.iter().flatten() {
        collect_frame_urls(c, out);
    }
}

/// URL moi frame (frame chinh truoc, roi con chau theo `Page.getFrameTree`) - Python
/// `find_captcha_frame`: tim frame co `"bdcaptcha.html"` trong URL.
///
/// Gioi han (review Task 5 M-5, luu y cho Task 8): chi hoi target CHINH. iframe khac site
/// (OOPIF, vd captcha) VAN xuat hien kem URL nhu mot muc con, nhung frame LONG BEN TRONG mot OOPIF
/// thi khong (Python `page.frames` gom ca chung). Du de tim `bdcaptcha.html`.
///
/// # Errors
/// Loi CDP.
pub async fn frame_urls(page: &Page) -> Result<Vec<String>, BrowserError> {
    let tree = page
        .execute(GetFrameTreeParams::default())
        .await
        .map_err(map_cdp)?
        .result
        .frame_tree;
    let mut out = Vec::new();
    collect_frame_urls(&tree, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truthy_theo_js() {
        use serde_json::json;
        assert!(!truthy(&json!(null)));
        assert!(!truthy(&json!(false)));
        assert!(!truthy(&json!(0)));
        assert!(!truthy(&json!(0.0)));
        assert!(!truthy(&json!("")));
        assert!(truthy(&json!(true)));
        assert!(truthy(&json!(1)));
        assert!(truthy(&json!("x")));
        assert!(truthy(&json!([])));
        assert!(truthy(&json!({})));
    }

    #[test]
    fn key_def_bi_danh_va_ky_tu_don() {
        assert_eq!(key_def("Ctrl").unwrap().key, "Control");
        assert_eq!(key_def("Control").unwrap().code, "ControlLeft");
        assert_eq!(key_def("Esc").unwrap().key, "Escape");
        assert_eq!(key_def("Enter").unwrap().text, Some("\r"));
        assert_eq!(key_def("A").unwrap().code, "KeyA");
        assert_eq!(key_def("a").unwrap().key_code, 65);
        assert_eq!(key_def("Delete").unwrap().key_code, 46);
        assert_eq!(key_def("End").unwrap().key_code, 35);
        assert!(key_def("PhimKhongCo").is_err());
        assert!(key_def("").is_err());
    }

    #[test]
    fn modifier_bit_dung_bang_cdp() {
        assert_eq!(modifier_bit(key_def("Alt").unwrap()), 1);
        assert_eq!(modifier_bit(key_def("Control").unwrap()), 2);
        assert_eq!(modifier_bit(key_def("Meta").unwrap()), 4);
        assert_eq!(modifier_bit(key_def("Shift").unwrap()), 8);
        assert_eq!(modifier_bit(key_def("a").unwrap()), 0);
    }

    #[test]
    fn context_gone_chi_bat_loi_context() {
        let gone = BrowserError::Cdp(CdpError::msg(
            "Error -32000: Execution context was destroyed.",
        ));
        assert!(context_gone(&gone));
        let gone2 = BrowserError::Cdp(CdpError::msg(
            "Error -32000: Cannot find context with specified id",
        ));
        assert!(context_gone(&gone2));
        let js = BrowserError::Js("TypeError: null".into());
        assert!(!context_gone(&js));
        let other = BrowserError::Timeout("x".into());
        assert!(!context_gone(&other));
    }

    #[test]
    fn frame_urls_de_quy_gom_fragment() {
        let v = serde_json::json!({
            "frame": {"id": "A", "loaderId": "l", "url": "https://www.dola.com/chat", "urlFragment": "#x",
                      "domainAndRegistry": "dola.com", "securityOrigin": "https://www.dola.com",
                      "mimeType": "text/html", "secureContextType": "Secure", "crossOriginIsolatedContextType": "NotIsolated", "gatedAPIFeatures": []},
            "childFrames": [{
                "frame": {"id": "B", "loaderId": "l2", "url": "https://x.dola.com/bdcaptcha.html",
                          "domainAndRegistry": "dola.com", "securityOrigin": "https://x.dola.com",
                          "mimeType": "text/html", "secureContextType": "Secure", "crossOriginIsolatedContextType": "NotIsolated", "gatedAPIFeatures": []}
            }]
        });
        let tree: FrameTree = serde_json::from_value(v).unwrap();
        let mut out = Vec::new();
        collect_frame_urls(&tree, &mut out);
        assert_eq!(
            out,
            vec![
                "https://www.dola.com/chat#x",
                "https://x.dola.com/bdcaptcha.html"
            ]
        );
    }

    /// Chinh sach log: `Debug` cua `CookieKv` KHONG in gia tri cookie.
    #[test]
    fn cookie_kv_debug_che_gia_tri() {
        let c = CookieKv::new(".dola.com", "sessionid", "bi-mat-123");
        let s = format!("{c:?}");
        assert!(s.contains("sessionid") && s.contains(".dola.com"), "{s}");
        assert!(!s.contains("bi-mat-123"), "lo gia tri cookie: {s}");
        assert!(s.contains("***"), "{s}");
    }

    #[test]
    fn exception_text_lay_dong_dau_description() {
        let v = serde_json::json!({
            "exceptionId": 1, "text": "Uncaught", "lineNumber": 0, "columnNumber": 0,
            "exception": {"type": "object", "subtype": "error",
                          "description": "TypeError: Cannot read properties of null (reading 'foo')\n    at <anonymous>:1:6"}
        });
        let ex: ExceptionDetails = serde_json::from_value(v).unwrap();
        assert_eq!(
            exception_text(&ex),
            "TypeError: Cannot read properties of null (reading 'foo')"
        );
        let v2 = serde_json::json!({"exceptionId": 1, "text": "Uncaught SyntaxError", "lineNumber": 0, "columnNumber": 0});
        let ex2: ExceptionDetails = serde_json::from_value(v2).unwrap();
        assert_eq!(exception_text(&ex2), "Uncaught SyntaxError");
    }
}
