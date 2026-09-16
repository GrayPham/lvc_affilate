# P1 — `BrowserDriver` + chromiumoxide: Kế hoạch triển khai

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trả lời câu hỏi cửa tử — *chromiumoxide (CDP thuần Rust) có mở được phiên đăng nhập trên dola.com mà không bị risk-control chặn không?* — bằng một thí nghiệm đo thật, trên nền hạ tầng tái dùng được cho P2.

**Architecture:** Hai module thuần mới trong `lvc-core` (`cookie`, `session`) port y nguyên `cookie_import.py` và `session_state.py`. Crate mới `lvc-browser` chứa `trait BrowserDriver` (ADR-R002) và impl `ChromiumDriver` trên chromiumoxide. Task 7 là thí nghiệm live — **có cổng xin phép người dùng** vì gọi vào dịch vụ bên thứ ba bằng cookie thật.

**Tech Stack:** chromiumoxide 0.9 (chỉ hỗ trợ tokio — không cần feature flag) · async-trait · tokio · futures

**Spec:** `docs/superpowers/specs/2026-09-16-lvc-seedance-rust-design.md` §1.1, ADR-R002, §7.1. Baseline Python `8ec1cfac`.

## Global Constraints

- Mọi ràng buộc P0/P0.5 vẫn áp dụng. `lvc-core` **vẫn không I/O** — `cookie`/`session` là parser + luật thuần.
- **Launch args khớp Python `browser.py:12-18`** từng chuỗi: `--disable-blink-features=AutomationControlled`, `--disable-infobars`, `--silent-debugger-extension-api`, `--no-first-run`, `--no-default-browser-check`; và **bỏ** `--enable-automation` (Playwright `ignore_default_args`).
- **Locale `ja-JP`, timezone `Asia/Tokyo`** — bắt buộc. Dola chọn ngôn ngữ UI theo locale lúc mở lần đầu; mọi selector đời sau (高速/プロ/動画を作成…) là tiếng Nhật. Đổi locale = hỏng toàn bộ tự động hoá.
- Profile persistent tại `accounts/<name>/` — cùng bố cục với bản Python để **tái dùng profile Chromium người dùng đang có**.
- `SESSION_PROBE_JS` khớp Python `browser.py:328-340` từng ký tự.
- **Thí nghiệm live (Task 7) không được chạy tự động** — cần người dùng cung cấp `cookies.txt` và đồng ý.
- Proxy P1: chỉ `--proxy-server=<scheme://host:port>` **không auth**. Proxy có user:pass cần CDP `Fetch.continueWithAuth` → P2.

---

### Task 1: `lvc-core::cookie` — parser Netscape

**Files:**
- Create: `crates/lvc-core/src/cookie.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Produces: `Cookie { name, value, domain, path, secure, http_only, expires: Option<i64> }` (Clone, PartialEq, Serialize, Deserialize); `parse_netscape(&str) -> Vec<Cookie>`; `fill_ss_aliases(Vec<Cookie>) -> Vec<Cookie>`; `validate_dola_cookies(&[Cookie]) -> Result<Vec<Cookie>, CookieError>`; `sessionid_expires(&[Cookie]) -> Option<i64>`; hằng `DOLA_DOMAIN = "dola.com"`, `REQUIRED_COOKIE = "sessionid"`, `MAX_TEXT_BYTES = 200 * 1024`.

- [ ] **Step 1: Test (RED)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Netscape HTTP Cookie File\n\
        .dola.com\tTRUE\t/\tTRUE\t1800000000\tsessionid\tabc123\n\
        #HttpOnly_.dola.com\tTRUE\t/\tTRUE\t0\tuid_tt\tu1\n\
        .other.com\tTRUE\t/\tFALSE\t1800000000\tfoo\tbar\n";

    #[test]
    fn doc_7_cot_tab_bo_comment_nhan_httponly() {
        let c = parse_netscape(SAMPLE);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].name, "sessionid");
        assert_eq!(c[0].domain, ".dola.com");
        assert!(c[0].secure);
        assert!(!c[0].http_only);
        assert_eq!(c[0].expires, Some(1_800_000_000));
        assert_eq!(c[1].name, "uid_tt");
        assert!(c[1].http_only, "#HttpOnly_ prefix");
        assert_eq!(c[1].expires, None, "expiry 0 = cookie phien");
    }

    #[test]
    fn mat_tab_thi_tach_bang_nhieu_khoang_trang() {
        let txt = ".dola.com   TRUE   /   TRUE   1800000000   sessionid   xyz\n";
        let c = parse_netscape(txt);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].value, "xyz");
    }

    #[test]
    fn dong_thieu_cot_hoac_rac_bi_bo() {
        assert!(parse_netscape("a\tb\tc\n\n   \n# x\n").is_empty());
    }

    #[test]
    fn ss_alias_duoc_nhan_ban_khi_chua_co_goc() {
        // CK-BUG-006: Cookie-Editor xuat ban ByteDance chi co sessionid_ss
        let txt = ".dola.com\tTRUE\t/\tTRUE\t0\tsessionid_ss\tS1\n";
        let c = parse_netscape(txt);
        assert_eq!(c.len(), 2);
        let alias = c.iter().find(|k| k.name == "sessionid").unwrap();
        assert_eq!(alias.value, "S1");
        assert!(alias.http_only && alias.secure);
    }

    #[test]
    fn ss_alias_khong_ghi_de_goc_da_co() {
        let txt = ".dola.com\tTRUE\t/\tTRUE\t0\tsessionid\tGOC\n\
                   .dola.com\tTRUE\t/\tTRUE\t0\tsessionid_ss\tSS\n";
        let c = parse_netscape(txt);
        assert_eq!(c.len(), 2);
        assert_eq!(c.iter().find(|k| k.name == "sessionid").unwrap().value, "GOC");
    }

    #[test]
    fn validate_chi_giu_dola_va_can_sessionid() {
        let c = parse_netscape(SAMPLE);
        let dola = validate_dola_cookies(&c).unwrap();
        assert_eq!(dola.len(), 2, "bo .other.com");
        let no_sid = parse_netscape(".dola.com\tTRUE\t/\tTRUE\t0\tfoo\tbar\n");
        let e = validate_dola_cookies(&no_sid).unwrap_err();
        assert_eq!(e.to_string(), "missing 'sessionid' cookie for dola.com (not logged in when exported?)");
        let e = validate_dola_cookies(&[]).unwrap_err();
        assert_eq!(e.to_string(), "no cookies for dola.com found (export cookies while logged in at www.dola.com)");
    }

    #[test]
    fn sessionid_expires_lay_epoch_hoac_none() {
        let c = parse_netscape(SAMPLE);
        assert_eq!(sessionid_expires(&c), Some(1_800_000_000));
        let s = parse_netscape(".dola.com\tTRUE\t/\tTRUE\t0\tsessionid\tx\n");
        assert_eq!(sessionid_expires(&s), None);
    }
}
```

- [ ] **Step 2: RED** — `cargo test -p lvc-core cookie` → FAIL.

- [ ] **Step 3: Viết `cookie.rs`**

```rust
//! Cookie Dola từ định dạng Netscape cookies.txt (HOP-DONG v1.3, ADR-005). Thuần.
//!
//! 7 cột cách TAB: domain, include_subdomains, path, secure, expiry(epoch, 0=phiên), name, value.
//! `#HttpOnly_` là cookie HttpOnly (curl/wget), `#` khác là chú thích.
//! Extension export hay mất tab khi copy-dán → chấp nhận tách bằng ≥2 khoảng trắng.

use serde::{Deserialize, Serialize};

pub const DOLA_DOMAIN: &str = "dola.com";
pub const REQUIRED_COOKIE: &str = "sessionid";
pub const MAX_TEXT_BYTES: usize = 200 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// Epoch giây; `None` = cookie phiên.
    pub expires: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CookieError {
    #[error("no cookies for {DOLA_DOMAIN} found (export cookies while logged in at www.dola.com)")]
    NoDolaCookies,
    #[error("missing '{REQUIRED_COOKIE}' cookie for {DOLA_DOMAIN} (not logged in when exported?)")]
    MissingSessionId,
}

/// Tách theo TAB; thiếu cột thì thử tách theo ≥2 khoảng trắng hoặc TAB (mất tab khi dán).
fn split_cols(line: &str) -> Vec<String> {
    let by_tab: Vec<String> = line.split('\t').map(str::to_string).collect();
    if by_tab.len() >= 7 {
        return by_tab;
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut spaces = 0usize;
    for ch in line.chars() {
        match ch {
            '\t' => {
                out.push(std::mem::take(&mut cur));
                spaces = 0;
            }
            ' ' => {
                spaces += 1;
                if spaces == 2 {
                    cur.truncate(cur.len().saturating_sub(1));
                    out.push(std::mem::take(&mut cur));
                } else if spaces == 1 {
                    cur.push(' ');
                }
            }
            _ => {
                spaces = 0;
                cur.push(ch);
            }
        }
    }
    out.push(cur);
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

#[must_use]
pub fn parse_netscape(text: &str) -> Vec<Cookie> {
    let mut out = Vec::new();
    for raw in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let mut line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let mut http_only = false;
        if let Some(rest) = line.strip_prefix("#HttpOnly_") {
            line = rest;
            http_only = true;
        } else if line.starts_with('#') {
            continue;
        }
        let parts = split_cols(line);
        if parts.len() < 7 {
            continue;
        }
        let domain = parts[0].trim();
        let path = parts[2].trim();
        let secure = parts[3].trim().eq_ignore_ascii_case("TRUE");
        let expiry = parts[4].trim();
        let name = parts[5].trim();
        let value = parts[6..].join("\t").trim().to_string();
        if domain.is_empty() || name.is_empty() {
            continue;
        }
        let expires = expiry
            .parse::<f64>()
            .ok()
            .map(|f| f as i64)
            .filter(|&e| e > 0);
        out.push(Cookie {
            name: name.to_string(),
            value,
            domain: domain.to_string(),
            path: if path.is_empty() { "/".to_string() } else { path.to_string() },
            secure,
            http_only,
            expires,
        });
    }
    fill_ss_aliases(out)
}

/// CK-BUG-006: `sessionid_ss` → thêm bản sao `sessionid` (httpOnly+secure) khi chưa có. Không ghi đè gốc.
#[must_use]
pub fn fill_ss_aliases(cookies: Vec<Cookie>) -> Vec<Cookie> {
    let mut have: std::collections::HashSet<(String, String)> =
        cookies.iter().map(|c| (c.domain.clone(), c.name.clone())).collect();
    let mut extra = Vec::new();
    for c in &cookies {
        if let Some(base) = c.name.strip_suffix("_ss") {
            if !base.is_empty() && !have.contains(&(c.domain.clone(), base.to_string())) {
                let mut alias = c.clone();
                alias.name = base.to_string();
                alias.http_only = true;
                alias.secure = true;
                have.insert((c.domain.clone(), base.to_string()));
                extra.push(alias);
            }
        }
    }
    let mut all = cookies;
    all.extend(extra);
    all
}

/// Chỉ giữ cookie `*.dola.com`; bắt buộc có `sessionid` không rỗng.
///
/// # Errors
/// `NoDolaCookies` / `MissingSessionId` — thông điệp khớp Python.
pub fn validate_dola_cookies(cookies: &[Cookie]) -> Result<Vec<Cookie>, CookieError> {
    let dola: Vec<Cookie> = cookies
        .iter()
        .filter(|c| c.domain.to_lowercase().contains(DOLA_DOMAIN))
        .cloned()
        .collect();
    if dola.is_empty() {
        return Err(CookieError::NoDolaCookies);
    }
    if !dola.iter().any(|c| c.name == REQUIRED_COOKIE && !c.value.is_empty()) {
        return Err(CookieError::MissingSessionId);
    }
    Ok(dola)
}

#[must_use]
pub fn sessionid_expires(cookies: &[Cookie]) -> Option<i64> {
    cookies
        .iter()
        .find(|c| c.name == REQUIRED_COOKIE && c.domain.to_lowercase().contains(DOLA_DOMAIN))
        .and_then(|c| c.expires)
}
```

> Lưu ý `split_cols`: Python dùng `re.split(r"\s{2,}|\t")`. Hàm trên tách khi gặp ≥2 khoảng trắng liên tiếp hoặc TAB, giữ khoảng trắng đơn bên trong cột. Nếu test `mat_tab_thi_tach_bang_nhieu_khoang_trang` không xanh, implementer phải sửa `split_cols` chứ **không** nới test.

- [ ] **Step 4: `lib.rs`** — `pub mod cookie; pub use cookie::{Cookie, CookieError, parse_netscape, validate_dola_cookies, sessionid_expires};`

- [ ] **Step 5: GREEN + gate + commit** — `feat(core): parser cookie Netscape va validate Dola (port cookie_import.py)`

---

### Task 2: `lvc-core::session` — phân loại phiên

**Files:**
- Create: `crates/lvc-core/src/session.rs`
- Modify: `crates/lvc-core/src/lib.rs`

**Interfaces:**
- Produces: `SessionState { LoggedIn, LoggedOut, Unknown }` với `as_str()`; `LOGIN_MARKERS: [&str; 9]`; `has_login_marker(&str) -> bool`; `classify_session(has_sessionid: bool, page_texts: &[String], has_composer: bool) -> SessionState`; `SESSION_PROBE_JS: &str`.

- [ ] **Step 1: Test (RED)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_khop_dau_nhan_khong_dinh_tu_khac() {
        assert!(has_login_marker("Log In"));
        assert!(has_login_marker("ログイン"));
        assert!(has_login_marker("ゲストモード"), "CJK khong can khoang trang");
        assert!(!has_login_marker("login history"), "Log in la tien to cua tu khac? 'login' != 'Log in'");
        assert!(!has_login_marker("Guestbook"), "dinh chu cai sau marker Latin");
        assert!(has_login_marker("Guest"));
        assert!(!has_login_marker(""));
        assert!(!has_login_marker(&"x".repeat(41)), "nhan > 40 ky tu khong tinh");
    }

    #[test]
    fn khong_sessionid_la_logged_out() {
        assert_eq!(classify_session(false, &[], true), SessionState::LoggedOut);
    }

    #[test]
    fn co_marker_la_logged_out_ke_ca_khi_co_o_nhap() {
        // do that 2026-09-06: trang chua dang nhap co o nhap + nut ログイン
        let texts = vec!["Home".to_string(), "ログイン".to_string()];
        assert_eq!(classify_session(true, &texts, true), SessionState::LoggedOut);
    }

    #[test]
    fn sessionid_va_o_nhap_khong_marker_la_logged_in() {
        let texts = vec!["Home".to_string(), "Settings".to_string()];
        assert_eq!(classify_session(true, &texts, true), SessionState::LoggedIn);
    }

    #[test]
    fn thieu_o_nhap_la_unknown() {
        assert_eq!(classify_session(true, &[], false), SessionState::Unknown);
    }

    #[test]
    fn probe_js_khop_python() {
        assert!(SESSION_PROBE_JS.contains("el.childElementCount === 0"));
        assert!(SESSION_PROBE_JS.contains("[contenteditable=\"true\"]"));
        assert!(SESSION_PROBE_JS.contains("texts.length > 400"));
    }
}
```

- [ ] **Step 2: RED** → FAIL.

- [ ] **Step 3: Viết `session.rs`**

```rust
//! Luật "tài khoản đã bị đăng xuất" (HOP-DONG-nhom1 v1, REQ-002). Thuần.

use serde::{Deserialize, Serialize};

/// Chữ đúng nghĩa "đăng nhập"/"khách" trên nhãn; so theo cụm, không phải chuỗi con.
pub const LOGIN_MARKERS: [&str; 9] = [
    "ログイン", "Log In", "Log in", "Sign in", "Sign In", "登录", "登入", "ゲスト", "Guest",
];

/// JS chạy trong trang: gom nhãn lá (≤40 ký tự, không con) + có ô nhập không.
/// Khớp Python `browser.py` `SESSION_PROBE_JS` từng ký tự.
pub const SESSION_PROBE_JS: &str = r#"() => {
  const texts = [];
  for (const el of document.querySelectorAll('button, a, [role="button"], h1, h2, h3, div, span')) {
    const t = (el.textContent || '').trim();
    if (t && t.length <= 40 && el.childElementCount === 0) texts.push(t);
    if (texts.length > 400) break;
  }
  const composer = !!(document.querySelector('textarea') || document.querySelector('[contenteditable="true"]'));
  return {texts, composer};
}"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    LoggedIn,
    LoggedOut,
    Unknown,
}

impl SessionState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoggedIn => "logged_in",
            Self::LoggedOut => "logged_out",
            Self::Unknown => "unknown",
        }
    }
}

/// `text` là NHÃN một phần tử lá. Khớp khi bắt đầu bằng marker và (với Latin) ký tự
/// sau marker không phải chữ/số; CJK (codepoint > U+2E7F) không cần ranh giới.
#[must_use]
pub fn has_login_marker(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 40 {
        return false;
    }
    for marker in LOGIN_MARKERS {
        let Some(rest) = text.strip_prefix(marker) else { continue };
        if rest.is_empty() {
            return true;
        }
        let last_is_cjk = marker.chars().last().is_some_and(|c| (c as u32) > 0x2E7F);
        let next_alnum = rest.chars().next().is_some_and(char::is_alphanumeric);
        if last_is_cjk || !next_alnum {
            return true;
        }
    }
    false
}

/// - không sessionid → `LoggedOut`
/// - có nhãn đăng nhập/khách → `LoggedOut` (Dola cho khách gõ chat nên ô nhập không đủ)
/// - có sessionid + ô nhập + không marker → `LoggedIn`
/// - còn lại → `Unknown` (trang chưa tải xong), caller không được kết luận
#[must_use]
pub fn classify_session(has_sessionid: bool, page_texts: &[String], has_composer: bool) -> SessionState {
    if !has_sessionid {
        return SessionState::LoggedOut;
    }
    if page_texts.iter().any(|t| has_login_marker(t)) {
        return SessionState::LoggedOut;
    }
    if has_composer { SessionState::LoggedIn } else { SessionState::Unknown }
}
```

- [ ] **Step 4: `lib.rs`** — `pub mod session; pub use session::{SESSION_PROBE_JS, SessionState, classify_session, has_login_marker};`

- [ ] **Step 5: GREEN + gate + commit** — `feat(core): phan loai phien dang nhap Dola (port session_state.py)`

---

### Task 3: Scaffold `lvc-browser` + trait `BrowserDriver`

**Files:**
- Create: `crates/lvc-browser/Cargo.toml`, `crates/lvc-browser/src/lib.rs`, `crates/lvc-browser/src/error.rs`, `crates/lvc-browser/src/driver.rs`
- Modify: `Cargo.toml` (workspace members + deps)

**Interfaces:**
- Produces: `BrowserError`; `LaunchOpts { profile_dir: PathBuf, headless: bool, proxy_server: Option<String>, window_offscreen: bool, chrome_executable: Option<PathBuf> }`; `SessionProbe { has_sessionid: bool, texts: Vec<String>, composer: bool, url: String, state: SessionState }`; `#[async_trait] trait BrowserDriver`.

- [ ] **Step 1: Workspace `Cargo.toml`**

`members` thêm `"crates/lvc-browser"`. `[workspace.dependencies]` thêm:
```toml
chromiumoxide = "0.9"
async-trait = "0.1"
futures = "0.3"
tracing = "0.1"
lvc-browser = { path = "crates/lvc-browser" }
```

- [ ] **Step 2: `crates/lvc-browser/Cargo.toml`**

```toml
[package]
name = "lvc-browser"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
lvc-core.workspace = true
chromiumoxide.workspace = true
async-trait.workspace = true
futures.workspace = true
tokio.workspace = true
tracing.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
tempfile = "3"

[lints]
workspace = true
```

- [ ] **Step 3: `error.rs`**

```rust
#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("khong khoi dong duoc trinh duyet: {0}")]
    Launch(String),
    #[error("loi CDP: {0}")]
    Cdp(#[from] chromiumoxide::error::CdpError),
    #[error("profile dang bi tien trinh khac dung: {0}")]
    ProfileBusy(String),
    #[error("loi I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("loi JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("dieu huong that bai toi {url}: {reason}")]
    Navigation { url: String, reason: String },
}
```

- [ ] **Step 4: `driver.rs`**

```rust
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use lvc_core::{Cookie, SessionState};

use crate::BrowserError;

#[derive(Debug, Clone)]
pub struct LaunchOpts {
    /// `accounts/<name>/` — cùng bố cục bản Python để tái dùng profile.
    pub profile_dir: PathBuf,
    pub headless: bool,
    /// `scheme://host:port`, KHÔNG auth (P2).
    pub proxy_server: Option<String>,
    /// Đưa cửa sổ ra ngoài màn hình (-32000,-32000) như SILENT_MODE của Python.
    pub window_offscreen: bool,
    /// `None` → chromiumoxide tự tìm Chrome/Chromium trên máy.
    pub chrome_executable: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionProbe {
    pub has_sessionid: bool,
    pub texts: Vec<String>,
    pub composer: bool,
    pub url: String,
    pub state: SessionState,
}

/// Mọi thao tác trình duyệt đi qua trait này (ADR-R002) để đổi driver được nếu
/// chromiumoxide bị Dola chặn. P1 chỉ có bề mặt cần cho thí nghiệm đăng nhập;
/// `submit`/`poll_and_download` thêm ở P2.
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    type Session: Send;

    async fn launch(&self, opts: &LaunchOpts) -> Result<Self::Session, BrowserError>;
    async fn add_cookies(&self, s: &Self::Session, cookies: &[Cookie]) -> Result<(), BrowserError>;
    async fn goto(&self, s: &Self::Session, url: &str) -> Result<(), BrowserError>;
    async fn probe_session(&self, s: &Self::Session) -> Result<SessionProbe, BrowserError>;
    async fn screenshot(&self, s: &Self::Session, path: &Path) -> Result<(), BrowserError>;
    async fn close(&self, s: Self::Session) -> Result<(), BrowserError>;
}
```

- [ ] **Step 5: `lib.rs`** — `pub mod driver; pub mod error; pub use driver::{BrowserDriver, LaunchOpts, SessionProbe}; pub use error::BrowserError;`

- [ ] **Step 6: `cargo build -p lvc-browser` xanh; `scripts/check.ps1` xanh** (trait chưa có impl — hợp lệ). Cập nhật ban list trong `check.ps1`? KHÔNG — `lvc-core` vẫn không được phụ thuộc chromiumoxide; `lvc-browser` thì được. Chạy gate để xác nhận `lvc-core` vẫn thuần.

- [ ] **Step 7: Commit** — `feat(browser): scaffold lvc-browser va trait BrowserDriver (ADR-R002)`

---

### Task 4: `ChromiumDriver::launch` — stealth + profile + locale/timezone

**Files:**
- Create: `crates/lvc-browser/src/chromium.rs`, `crates/lvc-browser/src/profile.rs`
- Modify: `crates/lvc-browser/src/lib.rs`

**Interfaces:**
- Produces: `ChromiumDriver` (unit struct, `Default`); `ChromiumSession { browser, page, handler_task }`; `profile::disable_session_restore(&Path) -> Result<bool, BrowserError>`; hằng `LAUNCH_ARGS: [&str; 5]`.

> Đây KHÔNG phải transcription thuần — API chromiumoxide 0.9 phải đối chiếu docs.rs. Implementer: nếu tên hàm/builder khác với code dưới, sửa theo docs và ghi rõ trong báo cáo; **không** đổi ngữ nghĩa (5 arg, bỏ enable-automation, ja-JP, Asia/Tokyo).

- [ ] **Step 1: Test `profile.rs` (RED)** — thuần I/O file, test bằng tempdir:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tao_preferences_moi_khi_chua_co() {
        let dir = tempfile::tempdir().unwrap();
        let changed = disable_session_restore(dir.path()).await.unwrap();
        assert!(changed);
        let p = dir.path().join("Default").join("Preferences");
        let v: serde_json::Value = serde_json::from_str(&tokio::fs::read_to_string(&p).await.unwrap()).unwrap();
        assert_eq!(v["session"]["restore_on_startup"], 5);
        assert_eq!(v["profile"]["exit_type"], "Normal");
        assert_eq!(v["profile"]["exited_cleanly"], true);
    }

    #[tokio::test]
    async fn giu_khoa_khac_va_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Default").join("Preferences");
        tokio::fs::create_dir_all(p.parent().unwrap()).await.unwrap();
        tokio::fs::write(&p, r#"{"other":{"keep":1},"session":{"restore_on_startup":1}}"#).await.unwrap();
        assert!(disable_session_restore(dir.path()).await.unwrap());
        assert!(!disable_session_restore(dir.path()).await.unwrap(), "lan 2 khong doi gi");
        let v: serde_json::Value = serde_json::from_str(&tokio::fs::read_to_string(&p).await.unwrap()).unwrap();
        assert_eq!(v["other"]["keep"], 1);
        assert_eq!(v["session"]["restore_on_startup"], 5);
    }
}
```

- [ ] **Step 2: Viết `profile.rs`**

```rust
//! Sửa `Default/Preferences` của profile Chromium TRƯỚC khi mở (port `disable_session_restore`).

use std::path::Path;

use crate::BrowserError;

/// Ghi `session.restore_on_startup=5`, `profile.exit_type="Normal"`, `profile.exited_cleanly=true`
/// để Chromium/Opera không mở lại tab cũ và không hiện "khôi phục trang". Giữ khoá khác.
/// Trả `true` nếu file thay đổi.
///
/// # Errors
/// I/O hoặc JSON hỏng.
pub async fn disable_session_restore(profile_dir: &Path) -> Result<bool, BrowserError> {
    let path = profile_dir.join("Default").join("Preferences");
    let mut root: serde_json::Value = match tokio::fs::read_to_string(&path).await {
        Ok(s) if !s.trim().is_empty() => serde_json::from_str(&s)?,
        _ => serde_json::json!({}),
    };
    let before = root.clone();
    root["session"]["restore_on_startup"] = serde_json::json!(5);
    root["profile"]["exit_type"] = serde_json::json!("Normal");
    root["profile"]["exited_cleanly"] = serde_json::json!(true);
    if root == before {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serde_json::to_string(&root)?).await?;
    Ok(true)
}
```

- [ ] **Step 3: Viết `chromium.rs` (phần launch)**

```rust
//! Impl `BrowserDriver` trên chromiumoxide (CDP thuần Rust).

use std::path::Path;

use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::{SetLocaleOverrideParams, SetTimezoneOverrideParams};
use chromiumoxide::Page;
use futures::StreamExt;
use lvc_core::Cookie;
use tokio::task::JoinHandle;

use crate::{BrowserDriver, BrowserError, LaunchOpts, SessionProbe, profile};

/// Khớp Python `browser.py` `LAUNCH_ARGS` từng chuỗi.
pub const LAUNCH_ARGS: [&str; 5] = [
    "--disable-blink-features=AutomationControlled",
    "--disable-infobars",
    "--silent-debugger-extension-api",
    "--no-first-run",
    "--no-default-browser-check",
];

pub const LOCALE: &str = "ja-JP";
pub const TIMEZONE: &str = "Asia/Tokyo";

#[derive(Debug, Default, Clone, Copy)]
pub struct ChromiumDriver;

pub struct ChromiumSession {
    pub browser: Browser,
    pub page: Page,
    handler: JoinHandle<()>,
}

impl ChromiumDriver {
    fn config(opts: &LaunchOpts) -> Result<BrowserConfig, BrowserError> {
        let mut b = BrowserConfig::builder()
            .user_data_dir(&opts.profile_dir)
            .hide()   // bo --enable-automation + AutomationControlled (tuong duong ignore_default_args)
            .arg(format!("--lang={LOCALE}"))
            .window_size(1280, 900);
        for a in LAUNCH_ARGS {
            b = b.arg(a);
        }
        if !opts.headless {
            b = b.with_head();
        }
        if opts.window_offscreen && !opts.headless {
            b = b.arg("--window-position=-32000,-32000");
        }
        if let Some(p) = &opts.proxy_server {
            b = b.arg(format!("--proxy-server={p}"));
        }
        if let Some(exe) = &opts.chrome_executable {
            b = b.chrome_executable(exe);
        }
        b.build().map_err(BrowserError::Launch)
    }
}

#[async_trait]
impl BrowserDriver for ChromiumDriver {
    type Session = ChromiumSession;

    async fn launch(&self, opts: &LaunchOpts) -> Result<ChromiumSession, BrowserError> {
        tokio::fs::create_dir_all(&opts.profile_dir).await?;
        profile::disable_session_restore(&opts.profile_dir).await?;
        let (browser, mut handler) = Browser::launch(Self::config(opts)?)
            .await
            .map_err(|e| {
                let s = e.to_string();
                if s.contains("already in use") || s.contains("existing browser session") {
                    BrowserError::ProfileBusy(s)
                } else {
                    BrowserError::Launch(s)
                }
            })?;
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
        let page = browser.new_page("about:blank").await?;
        page.execute(SetLocaleOverrideParams::builder().locale(LOCALE).build()).await?;
        page.execute(SetTimezoneOverrideParams::builder().timezone_id(TIMEZONE).build().map_err(BrowserError::Launch)?).await?;
        page.enable_stealth_mode().await?;
        Ok(ChromiumSession { browser, page, handler: handler_task })
    }

    // add_cookies / goto / probe_session / screenshot / close: Task 5
    async fn add_cookies(&self, _s: &ChromiumSession, _c: &[Cookie]) -> Result<(), BrowserError> { todo!() }
    async fn goto(&self, _s: &ChromiumSession, _u: &str) -> Result<(), BrowserError> { todo!() }
    async fn probe_session(&self, _s: &ChromiumSession) -> Result<SessionProbe, BrowserError> { todo!() }
    async fn screenshot(&self, _s: &ChromiumSession, _p: &Path) -> Result<(), BrowserError> { todo!() }
    async fn close(&self, _s: ChromiumSession) -> Result<(), BrowserError> { todo!() }
}
```

> `todo!()` ở Task 4 là tạm — Task 5 thay hết. Clippy `todo` không bị `pedantic` cấm nhưng `-D warnings` có thể bắt `clippy::todo` nếu bật; nếu vậy dùng `unimplemented!("Task 5")`.

- [ ] **Step 4: Test smoke launch** — `crates/lvc-browser/tests/launch_smoke.rs`, đánh dấu `#[ignore]` (cần Chrome trên máy; chạy tay bằng `cargo test -p lvc-browser -- --ignored`):

```rust
use lvc_browser::{BrowserDriver, LaunchOpts, chromium::ChromiumDriver};

#[tokio::test]
#[ignore = "can Chrome/Chromium tren may; chay: cargo test -p lvc-browser -- --ignored"]
async fn launch_headless_va_dong() {
    let dir = tempfile::tempdir().unwrap();
    let opts = LaunchOpts {
        profile_dir: dir.path().join("p"),
        headless: true,
        proxy_server: None,
        window_offscreen: false,
        chrome_executable: None,
    };
    let d = ChromiumDriver;
    let s = d.launch(&opts).await.expect("launch");
    // navigator.webdriver phai la undefined/false sau stealth
    let wd: serde_json::Value = s.page.evaluate("navigator.webdriver").await.unwrap().into_value().unwrap_or(serde_json::Value::Null);
    assert!(wd.is_null() || wd == serde_json::Value::Bool(false), "navigator.webdriver = {wd}");
    let lang: String = s.page.evaluate("navigator.language").await.unwrap().into_value().unwrap();
    assert_eq!(lang, "ja-JP");
    d.close(s).await.unwrap();
}
```

- [ ] **Step 5: `lib.rs`** thêm `pub mod chromium; pub mod profile;`.

- [ ] **Step 6: `cargo test -p lvc-browser` (profile tests xanh, smoke bị ignore); chạy smoke tay `cargo test -p lvc-browser -- --ignored` và DÁN output.** Máy này có Chrome? Kiểm: `Get-Command chrome` hoặc `%ProgramFiles%\Google\Chrome\Application\chrome.exe`; nếu không có, `chromiumoxide` feature `fetcher` tải Chromium — **KHÔNG bật** ở P1, báo BLOCKED nếu không có Chrome.

- [ ] **Step 7: Commit** — `feat(browser): ChromiumDriver::launch voi stealth args, ja-JP/Asia/Tokyo, profile persistent`

---

### Task 5: `add_cookies`, `goto`, `probe_session`, `screenshot`, `close`

**Files:**
- Modify: `crates/lvc-browser/src/chromium.rs`

- [ ] **Step 1: Thay 5 `todo!()`**

```rust
    async fn add_cookies(&self, s: &ChromiumSession, cookies: &[Cookie]) -> Result<(), BrowserError> {
        use chromiumoxide::cdp::browser_protocol::network::CookieParam;
        for c in cookies {
            let mut b = CookieParam::builder()
                .name(&c.name)
                .value(&c.value)
                .domain(&c.domain)
                .path(&c.path)
                .secure(c.secure)
                .http_only(c.http_only);
            if let Some(exp) = c.expires {
                b = b.expires(chromiumoxide::cdp::browser_protocol::network::TimeSinceEpoch::new(exp as f64));
            }
            s.page.set_cookie(b.build().map_err(BrowserError::Launch)?).await?;
        }
        Ok(())
    }

    async fn goto(&self, s: &ChromiumSession, url: &str) -> Result<(), BrowserError> {
        s.page.goto(url).await.map_err(|e| BrowserError::Navigation { url: url.into(), reason: e.to_string() })?;
        s.page.wait_for_navigation().await.map_err(|e| BrowserError::Navigation { url: url.into(), reason: e.to_string() })?;
        Ok(())
    }

    async fn probe_session(&self, s: &ChromiumSession) -> Result<SessionProbe, BrowserError> {
        #[derive(serde::Deserialize)]
        struct Raw { texts: Vec<String>, composer: bool }
        let raw: Raw = s.page.evaluate(lvc_core::SESSION_PROBE_JS).await?.into_value()?;
        let cookies = s.page.get_cookies().await?;
        let has_sessionid = cookies.iter().any(|c| c.name == lvc_core::cookie::REQUIRED_COOKIE && !c.value.is_empty());
        let url = s.page.url().await?.unwrap_or_default();
        let state = lvc_core::classify_session(has_sessionid, &raw.texts, raw.composer);
        Ok(SessionProbe { has_sessionid, texts: raw.texts, composer: raw.composer, url, state })
    }

    async fn screenshot(&self, s: &ChromiumSession, path: &Path) -> Result<(), BrowserError> {
        use chromiumoxide::page::ScreenshotParams;
        let bytes = s.page.screenshot(ScreenshotParams::builder().full_page(false).build()).await?;
        tokio::fs::write(path, bytes).await?;
        Ok(())
    }

    async fn close(&self, mut s: ChromiumSession) -> Result<(), BrowserError> {
        let _ = s.browser.close().await;
        s.handler.abort();
        Ok(())
    }
```

> `into_value()` trả `Result<T, serde_json::Error>` — `?` chuyển qua `BrowserError::Json`. Nếu chữ ký khác trên 0.9, sửa theo docs.rs.

- [ ] **Step 2: Mở rộng smoke test (`#[ignore]`)** — thêm case: set 1 cookie giả trên `about:blank` là KHÔNG hợp lệ (cookie cần domain thật); thay bằng goto `https://example.com`, set cookie `domain=example.com`, `get_cookies` thấy nó; `probe_session` trả `Unknown` (không sessionid → thực ra `LoggedOut`); `screenshot` ra file > 0 byte.

- [ ] **Step 3: Chạy tay smoke, DÁN output. Commit** — `feat(browser): cookie, dieu huong, probe phien, screenshot`

---

### Task 6: `examples/probe_login.rs` — binary thí nghiệm

**Files:**
- Create: `crates/lvc-browser/examples/probe_login.rs`

```rust
//! Thi nghiem cua tu P1: chromiumoxide co mo duoc phien Dola khong?
//! Chay: cargo run -p lvc-browser --example probe_login -- <profile_dir> <cookies.txt> [--headed] [--proxy scheme://host:port]
//! CHI DOC cookies.txt. Ghi: <profile_dir>/, ./probe_<ts>.png, stdout JSON.

use std::path::PathBuf;

use lvc_browser::{BrowserDriver, LaunchOpts, chromium::ChromiumDriver};
use lvc_core::{parse_netscape, validate_dola_cookies};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let profile_dir = PathBuf::from(&args[1]);
    let cookies_txt = tokio::fs::read_to_string(&args[2]).await?;
    let headed = args.iter().any(|a| a == "--headed");
    let proxy = args.iter().position(|a| a == "--proxy").and_then(|i| args.get(i + 1).cloned());

    let cookies = validate_dola_cookies(&parse_netscape(&cookies_txt))?;
    eprintln!("cookies dola: {} (co sessionid)", cookies.len());

    let d = ChromiumDriver;
    let s = d.launch(&LaunchOpts { profile_dir, headless: !headed, proxy_server: proxy, window_offscreen: false, chrome_executable: None }).await?;

    // Cookie can domain hop le -> goto dola truoc (se ve trang khach), set cookie, roi reload
    d.goto(&s, "https://www.dola.com/").await?;
    d.add_cookies(&s, &cookies).await?;
    d.goto(&s, "https://www.dola.com/chat").await?;
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;   // Python: 4s cho SPA

    let probe = d.probe_session(&s).await?;
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let shot = PathBuf::from(format!("probe_{ts}.png"));
    d.screenshot(&s, &shot).await?;

    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "state": probe.state.as_str(),
        "url": probe.url,
        "has_sessionid": probe.has_sessionid,
        "composer": probe.composer,
        "region_restricted": probe.url.contains("/security/region-restricted"),
        "login_markers_seen": probe.texts.iter().filter(|t| lvc_core::has_login_marker(t)).collect::<Vec<_>>(),
        "screenshot": shot,
    }))?);
    d.close(s).await?;
    Ok(())
}
```

- [ ] `cargo build -p lvc-browser --examples` xanh. Commit — `feat(browser): example probe_login cho thi nghiem cua tu`

---

### Task 7: THÍ NGHIỆM LIVE — cần người dùng

**KHÔNG tự chạy.** Controller dừng ở đây và hỏi người dùng:
1. Xuất `cookies.txt` (Netscape) từ trình duyệt đã đăng nhập www.dola.com (Cookie-Editor / Get cookies.txt LOCALLY).
2. Có cần proxy không (máy này ở VN có bị chặn vùng không?).
3. Đồng ý cho chạy example với cookie thật.

Khi có đồng ý:
```
cargo run -p lvc-browser --example probe_login -- accounts/thu_nghiem C:\path\cookies.txt --headed
```

**Đọc kết quả:**
| `state` | `region_restricted` | Kết luận |
|---|---|---|
| `logged_in` | false | **CỬA TỬ MỞ** — chromiumoxide dùng được. Tiến P2. |
| `logged_out` + sessionid có | false | Dola từ chối phiên → khả năng bị phát hiện CDP. Thử `--headed`; nếu vẫn → xem ADR-R002 phương án B (sidecar Python). |
| bất kỳ | true | Chặn vùng, không phải phát hiện bot. Cần proxy/VPN — thử `--proxy`. |
| `unknown` | – | Trang chưa tải xong; tăng sleep, xem screenshot. |

Ghi kết quả thành **ADR-R012** trong spec: đo thật ngày giờ, state, screenshot, quyết định driver.

---

## Cố ý KHÔNG làm ở P1
- Google OAuth + TOTP, silent relogin → P2 (sau khi cửa tử mở).
- CAPTCHA (OpenCV) → P2.
- Proxy có auth (CDP Fetch) → P2.
- Opera + VPN prefs → P2 hoặc bỏ (chromiumoxide + proxy thay thế).
- `submit`/`poll_and_download` trong trait → P2.

## Nghiệm thu P1
- [ ] Gate xanh; `lvc-core` vẫn không phụ thuộc chromiumoxide (check.ps1 kiểm).
- [ ] `cookie`/`session` test khớp Python (thông điệp lỗi từng ký tự).
- [ ] Smoke test `--ignored` chạy tay: `navigator.webdriver` không phải `true`, `navigator.language == "ja-JP"`.
- [ ] Task 7 có kết quả đo thật và ADR-R012.
