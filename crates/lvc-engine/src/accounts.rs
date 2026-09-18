//! `Engine::{list_with_status, import_cookies, verify, set_scheduling, set_proxy, delete,
//! import_legacy}` (P3 Task 10, tab Tai khoan UI) — quan ly tai khoan: liet ke + trang thai tinh
//! toan (nhu `browser_pool.py:490-548 list_accounts`), nhap cookie Netscape cho tai khoan MOI (R11),
//! kiem tra dang nhap (verify), bat/tat lich, sua proxy rieng, xoa tai khoan, va nhap du lieu ban
//! Python cu (R12).
//!
//! Cac ham o day la method tren [`Engine<D>`] (khong tach `AccountService` rieng) — giong cach
//! `dag.rs`/`resume.rs`/`settings.rs` da lam: `Ctx<D>` (lvc-app, Task 9) chi giu MOT `Arc<Engine<D>>`,
//! them mot kieu boc nua se phai xuyen tham chieu qua lai giua hai struct ma khong loi ich gi (moi
//! ham deu can `self.cfg`/`self.pool()`/`self.driver`/`self.db` — deu la truong/accessor co san cua
//! `Engine`).
//!
//! **Khac `dag.rs`/`resume.rs`**: day la thao tac ADMIN doc lap (bam nut trong tab Tai khoan), KHONG
//! phai mot buoc trong pipeline tao video — nen KHONG dang ky `CancellationToken`/`EngineHooks`
//! (khong co nut "Huy" cho nhap cookie/verify, xem "Concerns" trong bao cao Task 10), KHONG dung
//! `SessionCache` (phien headless cho nhap cookie/verify khong duoc lan vao cache dung cho video that
//! — se khien lan `generate_video` ke tiep tai dung nham mot phien headless).
//!
//! **Khong bao gio log gia tri cookie** (chi so luong/ten cookie tim thay qua `lvc_core::cookie`) va
//! **khong bao gio log mat khau proxy** ([`lvc_core::proxy::mask_proxy`] cho `list_with_status`;
//! `set_proxy`/`import_cookies` luu proxy THO vao DB — DB khong phai log, nhung khong bao gio dua
//! chuoi proxy vao `tracing::*!`/thong diep loi).

use std::path::{Component, Path};
use std::time::Duration;

use lvc_browser::dola::labels::DOLA_CHAT_URL;
use lvc_browser::driver::SessionProbe;
use lvc_browser::{BrowserDriver, BrowserError, LaunchOpts};
use lvc_core::cookie::Cookie;
use lvc_core::proxy::mask_proxy;
use lvc_core::{BrowserKind, Mode, SessionState, effective_proxy, parse_proxy, tokyo_day};
use lvc_store::StoreError;
use serde::Serialize;
use tracing::warn;

use crate::EngineError;
use crate::generate::{Engine, now_unix};

/// Doi trang on dinh TRUOC khi doc dang nhap (trang chat vua tai xong co the chua render kip) —
/// `#[cfg(test)]` = 0 de test khong phai cho that (nhu `resume.rs::PROBE_SETTLE`, cung ly do).
#[cfg(not(test))]
const PROBE_SETTLE: Duration = Duration::from_secs(3);
#[cfg(test)]
const PROBE_SETTLE: Duration = Duration::from_secs(0);
/// So lan doc lai `probe_session` toi da khi con `SessionState::Unknown` (nhu `resume.rs`).
const PROBE_RETRY_TRIES: u32 = 3;
#[cfg(not(test))]
const PROBE_RETRY_DELAY: Duration = Duration::from_secs(1);
#[cfg(test)]
const PROBE_RETRY_DELAY: Duration = Duration::from_secs(0);

async fn settle(dur: Duration) {
    if !dur.is_zero() {
        tokio::time::sleep(dur).await;
    }
}

/// C1 (review fix-round 1, Critical): `name` PHAI la MOT thanh phan duong dan an toan TRUOC khi
/// dung toi bat ky filesystem/DB nao — port `NAME_RE = ^[A-Za-z0-9_-]{1,32}$` (Python
/// `server.py:100,1587,1682`). KHONG them dependency `regex` cho MOT bieu thuc don gian — tu kiem
/// tung byte (cung cach lam voi `lvc_core::scene::valid_account`, nhung ham do `private` trong
/// module khac nen khong dung lai truc tiep duoc). Kem kiem belt-and-braces
/// `Path::new(name).components()` la DUNG MOT `Component::Normal`: chan `""`/`".."`/`"a/b"` (regex
/// da chan) VA duong dan tuyet doi kieu `"C:\\Windows\\Temp"` tren Windows (`Path::join` voi mot
/// duong dan tuyet doi se THAY THE ca goc, khong noi vao `accounts_dir` — vi du that o review C1).
fn validate_name(name: &str) -> Result<&str, AccountError> {
    let chars_ok = !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    let mut components = Path::new(name).components();
    let one_normal_component =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if chars_ok && one_normal_component {
        Ok(name)
    } else {
        Err(AccountError::InvalidName)
    }
}

/// I4 (review fix-round 1) + Minor 2: browser HIEU LUC — khac cot tho `accounts_meta.browser`
/// (thuong RONG; Python `browser_pool.py:519` doc qua `self.effective_browser_for(a)`,
/// `:714-720 normalize_browser`). Gia tri khong hop le -> `chromium` + `warn!` (nhu `open_session`
/// trong `generate.rs` da lam cho duong tao video that — truoc ban sua nay ca `list_with_status` lan
/// `verify` roi vao truong hop nay AM THAM, khong log gi ca).
fn effective_browser(name: &str, raw: &str) -> BrowserKind {
    BrowserKind::parse(raw).unwrap_or_else(|e| {
        warn!(
            account = name,
            browser = raw,
            error = %e,
            "browser khong hop le -> dung chromium"
        );
        BrowserKind::Chromium
    })
}

/// Loi cua cac ham quan ly tai khoan — `code()` la nguon DUY NHAT cho ma `IpcError.code` (lvc-app
/// Task 10, `impl From<AccountError> for IpcError` trong `commands/mod.rs`, giong cach `EngineError`/
/// `StoreError` da lam o Task 9).
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("tai khoan '{0}' da ton tai")]
    AccountExists(String),
    #[error("khong tim thay tai khoan '{0}'")]
    AccountNotFound(String),
    #[error("ten tai khoan khong duoc rong")]
    InvalidName,
    #[error("{0}")]
    CookieInvalid(String),
    #[error("cookie khong dang nhap duoc sau khi kiem tra (khong co phien dang nhap hop le)")]
    CookieNotLoggedIn,
    #[error("{0}")]
    InvalidProxy(String),
    #[error("{0}")]
    ProfileBusy(String),
    #[error("tai khoan '{0}' dang duoc su dung (co tac vu dang chay)")]
    AccountBusy(String),
    /// I5 (fix-round 1, final-review): thu muc `accounts_dir.join(name)` KHONG rong VA khong
    /// "trong nhu" mot profile Chromium (khong co `Default/` hay `Local State`) — bao ve mot
    /// `accountsDir` bi chon nham (vd tro thang vao goc thu muc source code) khoi bi
    /// `remove_dir_all` xoa sach chi vi ten thu muc con trung ngau nhien voi mot dong DB
    /// `accounts_meta`. Dong DB VAN CON (khong xoa) de nguoi dung sua lai cai dat `accountsDir`
    /// roi thu lai — xem [`Engine::delete`].
    #[error(
        "thu muc '{0}' khong giong profile Chromium va khong rong — kiem tra lai cai dat \
         'Thư mục tài khoản' (accountsDir), du lieu tai khoan trong DB van con nguyen"
    )]
    NotAProfile(String),
    #[error(transparent)]
    Browser(#[from] BrowserError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Loi tang engine khac (hiem — cac ham o day chi goi `self.pool()`/`self.refresh_account_count`
    /// vi that ra chi that bai vi `Store`, da tach o `From<EngineError>` duoi; nhanh nay la luoi an
    /// toan cho cac bien the con lai cua `EngineError` chua tung xuat hien tu duong nay).
    #[error("{0}")]
    Engine(String),
}

impl AccountError {
    /// Ma on dinh cho `IpcError.code` (lvc-app Task 10) — KHONG dung `Display`/chuoi loi lam ma vi
    /// thong diep co the doi (dich thuat, chi tiet hoa) ma ma thi khong.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::AccountExists(_) => "ACCOUNT_EXISTS",
            Self::AccountNotFound(_) => "ACCOUNT_NOT_FOUND",
            Self::InvalidName => "INVALID_NAME",
            Self::CookieInvalid(_) => "COOKIE_INVALID",
            Self::CookieNotLoggedIn => "COOKIE_NOT_LOGGED_IN",
            Self::InvalidProxy(_) => "INVALID_PROXY",
            Self::ProfileBusy(_) | Self::Browser(BrowserError::ProfileBusy(_)) => "PROFILE_BUSY",
            Self::AccountBusy(_) => "ACCOUNT_BUSY",
            Self::NotAProfile(_) => "NOT_A_PROFILE",
            Self::Browser(_) => "BROWSER",
            Self::Store(_) => "STORE",
            Self::Io(_) => "IO",
            Self::Engine(_) => "ENGINE",
        }
    }
}

impl From<EngineError> for AccountError {
    fn from(e: EngineError) -> Self {
        match e {
            EngineError::Store(se) => Self::Store(se),
            other => Self::Engine(other.to_string()),
        }
    }
}

/// Mot tai khoan + trang thai tinh toan tai `now` — port `browser_pool.py:490-548 list_accounts`
/// (xem bang doi chieu day du trong `task-10-context.md`). `Serialize` camelCase cho IPC (Task 10);
/// KHONG co `Deserialize` (chi tra ve, khong bao gio nhan tu UI).
///
/// Bo so voi Python (chua port o P3, ghi trong context — N1-IDEA-001 kiem proxy): `proxy_ok`/
/// `proxy_error`/`proxy_checked_at`, `auto_verify_retries`.
// 6 truong bool doc lap (scheduling/login_ok/cooling/rate_limited/quota_blocked/proxy_set) khop
// dung hinh dang danh sach cot cua Python (browser_pool.py:490-548) — khong phai trang thai loai
// tru lan nhau nen khong gop duoc thanh enum; giong ly do `Limits` (lvc-core/limits.rs) da allow.
#[allow(
    clippy::struct_excessive_bools,
    reason = "moi truong la mot co trang thai DOC LAP cua Python list_accounts, khong loai tru lan nhau"
)]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    pub name: String,
    pub scheduling: bool,
    pub note: String,
    pub email: String,
    pub created_at: f64,
    pub last_used_at: f64,
    pub login_ok: bool,
    pub login_checked_at: f64,
    pub cooldown_until: f64,
    pub cooling: bool,
    pub rate_limited_until: f64,
    pub rate_limited: bool,
    pub limit_reason: String,
    pub quota_blocked_until: f64,
    pub quota_blocked: bool,
    pub quota_reason: String,
    pub credit_balance: Option<i64>,
    pub credit_checked_at: f64,
    /// Da qua [`mask_proxy`] — KHONG BAO GIO mat khau day du (che ung dung UI, chua ke DB/log).
    pub proxy: String,
    /// I4 (review fix-round 1): browser HIEU LUC — khong phai cot tho `accounts_meta.browser`
    /// (thuong RONG); da qua `BrowserKind::parse` + mac dinh `"chromium"` khi khong hop le, giong
    /// Python `effective_browser_for`.
    pub browser: String,
    /// `bool(proxy)` cua Python — TRUOC khi mask, tren chuoi proxy THO (rong = chua dat proxy
    /// rieng, dung proxy chung).
    pub proxy_set: bool,
    pub logged_out_at: f64,
    pub auto_verify_state: String,
    pub auto_verify_tries: i64,
    pub cookie_expires_at: f64,
    /// = luot da dung mode `fast` hom nay.
    pub used_today: i64,
    /// = han muc ngay cua mode `fast`.
    pub limit: u32,
    /// `max(0, limit - used_today)`.
    pub remaining: i64,
    pub used_pro: i64,
    pub limit_pro: u32,
    pub used_pro_max: i64,
    pub limit_pro_max: u32,
    pub used_agent: i64,
    pub limit_agent: u32,
}

/// Ket qua `import_cookies`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCookiesReport {
    pub name: String,
    /// Han cookie `sessionid` (epoch giay) neu co — `None` = cookie phien (khong het han tuong
    /// minh) HOAC khong doc duoc tu cookie text.
    pub sessionid_expires_at: Option<i64>,
}

/// Ket qua `verify`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub state: SessionState,
    /// Gia tri VUA GHI vao DB — `Some(true)`/`Some(false)` khi `state` la `LoggedIn`/`LoggedOut`
    /// (DB da cap nhat); `None` khi `state` la `Unknown` (DB KHONG doi — xem doc [`Engine::verify`]).
    pub login_ok: Option<bool>,
    pub checked_at: f64,
}

/// Ket qua `import_legacy` — cac truong dau nhu `lvc_store::legacy::ImportReport` (chep tay thay vi
/// boc/flatten: giu `lvc-store` khong phai them `Serialize` chi cho mot lenh Tauri dung o `lvc-app`,
/// xem "Concerns" trong bao cao Task 10), them `profiles_copied` (R12 — P0 chi nhap DB, T10 them
/// buoc copy thu muc `accounts/<name>`), them `scripts`/`scenes` (P3c Task 1 R5 — nhap bang
/// `scripts`/`scenes` tu `tasks.db` Python neu co, xem `lvc_store::legacy::ImportReport`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportReport {
    pub tasks: usize,
    pub accounts: usize,
    pub usage: usize,
    pub settings: usize,
    pub skipped: Vec<String>,
    /// So thu muc `accounts/<name>` da copy tu `python_dir` sang `accounts_dir` (chi khi CHUA co
    /// san — khong bao gio ghi de).
    pub profiles_copied: usize,
    /// I3 (review fix-round 1): ten thu muc `accounts/<name>` (Python) KHONG duoc copy — dich da co
    /// san TU TRUOC (khong ghi de), ten khong hop le (C1), hoac copy/doi ten that bai giua chung
    /// (vd dang bi Chrome cua ban Python cu giu file). Khac `skipped` (chi 2 file DB co dinh) —
    /// day la danh sach TEN TAI KHOAN, co the rong.
    pub profiles_skipped: Vec<String>,
    /// P3c Task 1 (R5): so dong `scripts` da nhap tu `tasks.db` Python (0 neu ban do chua tung
    /// mo tab Kich ban — khong phai loi).
    pub scripts: usize,
    /// P3c Task 1 (R5): so dong `scenes` da nhap — xem [`LegacyImportReport::scripts`].
    pub scenes: usize,
}

/// Chep de qui (lap, khong dung async fn de qui de tranh phai `Box::pin`) mot cay thu muc — dung boi
/// `import_legacy` de copy `accounts/<name>` (profile Chrome: nhieu thu muc con, chi file + thu
/// muc thuong, khong theo symlink).
async fn copy_dir_all(src: &Path, dest: &Path) -> std::io::Result<()> {
    let mut stack = vec![(src.to_path_buf(), dest.to_path_buf())];
    while let Some((s, d)) = stack.pop() {
        tokio::fs::create_dir_all(&d).await?;
        let mut rd = tokio::fs::read_dir(&s).await?;
        while let Some(entry) = rd.next_entry().await? {
            let ty = entry.file_type().await?;
            let to = d.join(entry.file_name());
            if ty.is_dir() {
                stack.push((entry.path(), to));
            } else if ty.is_file() {
                tokio::fs::copy(entry.path(), to).await?;
            }
            // symlink: bo qua (profile Chrome khong dung, va copy theo se ra ngoai accounts_dir).
        }
    }
    Ok(())
}

/// I3 (review fix-round 1): thu muc tam `.import-<name>` con SOT LAI tu mot lan `import_legacy`
/// truoc bi ngat giua chung (crash/kill tool) — xoa SACH truoc khi copy lai, tranh tron noi dung cu
/// (co the da hong do copy do dang) voi noi dung moi. `Ok(())` ca khi thu muc khong ton tai
/// (idempotent).
async fn clear_stale_staging(staging: &Path) -> std::io::Result<()> {
    if staging.exists() {
        tokio::fs::remove_dir_all(staging).await?;
    }
    Ok(())
}

/// I1 (review fix-round 1): so lan thu lai `remove_dir_all` khi Windows con giu handle cua thu muc
/// profile TRONG CHOC LAT sau khi `driver.close` da tra ve (tien trinh Chrome con dang thoat hoan
/// toan) — 3 lan, cach nhau [`REMOVE_DIR_RETRY_DELAY`], truoc khi coi la that bai THAT SU
/// (`PROFILE_BUSY`).
const REMOVE_DIR_RETRIES: u32 = 3;
#[cfg(not(test))]
const REMOVE_DIR_RETRY_DELAY: Duration = Duration::from_millis(500);
#[cfg(test)]
const REMOVE_DIR_RETRY_DELAY: Duration = Duration::from_millis(0);

async fn remove_dir_all_retry(dir: &Path) -> std::io::Result<()> {
    let mut attempt = 0u32;
    loop {
        match tokio::fs::remove_dir_all(dir).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt < REMOVE_DIR_RETRIES => {
                attempt += 1;
                warn!(
                    dir = %dir.display(),
                    attempt,
                    error = %e,
                    "remove_dir_all that bai, thu lai (Windows co the con giu handle mot chut)"
                );
                settle(REMOVE_DIR_RETRY_DELAY).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// I5 (fix-round 1, final-review finding I5): lop phong thu CUOI CUNG truoc `remove_dir_all` trong
/// [`Engine::delete`] — mot `accountsDir` bi chon nham (vd goc thu muc source code) khong duoc
/// bien MOI thu muc con thanh "tai khoan" xoa duoc chi vi trung ten voi mot dong `accounts_meta`
/// (xem [`AccountPool::rows_with_dirs`](crate::pool::AccountPool::rows_with_dirs), tu dong
/// `upsert` moi thu muc con — port Python `_ensure_meta`).
///
/// `true` (AN TOAN de `remove_dir_all`) khi thu muc:
/// - KHONG ton tai (khong con gi de mat — nhanh nay hiem gap trong thuc te vi `Engine::delete` da
///   tu kiem `dir.exists()` truoc khi goi ham nay, giu lai cho goi truc tiep/test);
/// - RONG (khong co entry nao — khong co gi de mat); HOAC
/// - co mot entry ten `Default` (thu muc Chromium tao ngay lan dau mo, chua ho so nguoi dung mac
///   dinh) hoac `Local State` (file JSON cau hinh cap profile Chromium luon tao) — hai dau hieu
///   ON DINH nhat cua MOT profile Chromium that su, xem `chromium.rs::config_ep_user_data_dir_tuyet_doi`.
///
/// Doc thu muc THAT (khong chi doan qua ten) vi day la lop phong thu cuoi cung — muc tieu la
/// KHONG BAO GIO xoa nham mot thu muc nguoi dung lo tro `accountsDir` vao.
fn looks_like_chromium_profile(dir: &Path) -> std::io::Result<bool> {
    let entries = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(e) => return Err(e),
    };
    let mut any_entry = false;
    for entry in entries {
        let entry = entry?;
        any_entry = true;
        let name = entry.file_name();
        if name == "Default" || name == "Local State" {
            return Ok(true);
        }
    }
    Ok(!any_entry)
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// browser_pool.py:490-548 `list_accounts` — xem bang doi chieu day du trong doc [`AccountStatus`]
    /// / `task-10-context.md`. Dung [`crate::pool::AccountPool::rows_with_dirs`] lam nguon danh sach
    /// (CUNG nguon voi `candidates_for`/`pick_account` — UI khong bao gio thay tai khoan khac voi
    /// nhung gi engine dieu phoi thuc su xet).
    ///
    /// # Errors
    /// `Store` khi doc DB loi.
    pub async fn list_with_status(&self, now: f64) -> Result<Vec<AccountStatus>, EngineError> {
        let rows = self.pool().rows_with_dirs(now).await?;
        let day = tokyo_day(now);
        let repo = self.pool().repo();
        let limit = self.limits.daily_limit(Mode::Fast);
        let limit_pro = self.limits.daily_limit(Mode::Pro);
        let limit_pro_max = self.limits.daily_limit(Mode::ProMax);
        let limit_agent = self.limits.daily_limit(Mode::Agent);

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let used_today = repo.used_today(&row.name, &day, Mode::Fast).await?;
            let used_pro = repo.used_today(&row.name, &day, Mode::Pro).await?;
            let used_pro_max = repo.used_today(&row.name, &day, Mode::ProMax).await?;
            let used_agent = repo.used_today(&row.name, &day, Mode::Agent).await?;
            // I4 (review fix-round 1): browser HIEU LUC, khong phai cot tho (thuong RONG).
            let browser = effective_browser(&row.name, &row.browser);
            out.push(AccountStatus {
                name: row.name,
                scheduling: row.scheduling,
                note: row.note,
                email: row.email,
                created_at: row.created_at,
                last_used_at: row.last_used_at,
                login_ok: row.login_ok,
                login_checked_at: row.login_checked_at,
                cooldown_until: row.cooldown_until,
                cooling: row.cooldown_until > now,
                rate_limited_until: row.rate_limited_until,
                rate_limited: row.rate_limited_until > now,
                limit_reason: row.limit_reason,
                quota_blocked_until: row.quota_blocked_until,
                quota_blocked: row.quota_blocked_until > now,
                quota_reason: row.quota_reason,
                credit_balance: row.credit_balance,
                credit_checked_at: row.credit_checked_at,
                proxy_set: !row.proxy.trim().is_empty(),
                proxy: mask_proxy(&row.proxy),
                browser: browser.as_str().to_string(),
                logged_out_at: row.logged_out_at,
                auto_verify_state: row.auto_verify_state,
                auto_verify_tries: row.auto_verify_tries,
                cookie_expires_at: row.cookie_expires_at,
                used_today,
                limit,
                remaining: (i64::from(limit) - used_today).max(0),
                used_pro,
                limit_pro,
                used_pro_max,
                limit_pro_max,
                used_agent,
                limit_agent,
            });
        }
        Ok(out)
    }

    /// Dieu huong toi trang chat + doc trang thai dang nhap, co doi on dinh + doc lai gioi han khi
    /// con `Unknown` — dung chung boi [`Self::import_cookies`]/[`Self::verify`] (tuong tu
    /// `resume::resume_open`, nhung KHONG dang ky token huy — xem doc dau module).
    async fn goto_chat_and_probe(
        &self,
        session: &D::Session,
    ) -> Result<SessionProbe, BrowserError> {
        self.driver.goto(session, DOLA_CHAT_URL).await?;
        settle(PROBE_SETTLE).await;
        let mut probe = self.driver.probe_session(session).await?;
        let mut tries = 0u32;
        while probe.state == SessionState::Unknown && tries < PROBE_RETRY_TRIES {
            settle(PROBE_RETRY_DELAY).await;
            probe = self.driver.probe_session(session).await?;
            tries += 1;
        }
        Ok(probe)
    }

    /// R11: nhap cookie Netscape cho MOT TAI KHOAN MOI (thu muc `accounts_dir/<name>` CHUA duoc
    /// ton tai — tai khoan da co dung [`Self::verify`] de kiem tra lai). Luong: parse + kiem cookie
    /// (THUAN, truoc khi dung toi dia/trinh duyet) -> validate proxy -> tao thu muc profile -> launch
    /// HEADLESS (chi doc, khong ai nhin thay cua so) -> dat cookie -> goto chat -> doc dang nhap ->
    /// `LoggedIn` -> giu thu muc + ghi dong `accounts_meta` (upsert + proxy + `login_ok=true`);
    /// `LoggedOut`/`Unknown` (sau vai lan doc lai) hoac bat ky loi nao doc duong -> XOA thu muc vua
    /// tao (khong de rac profile rong/hong lai), tra loi tuong ung.
    ///
    /// Khong bao gio log gia tri cookie (chi dua `&[Cookie]` da parse thang vao `add_cookies`,
    /// khong `format!`/`tracing::*!` chung o day).
    ///
    /// # Errors
    /// - [`AccountError::InvalidName`]: `name` khong khop `NAME_RE` (C1 — rong, qua 32 ky tu, ky tu
    ///   la, hoac khong phai MOT thanh phan duong dan don).
    /// - [`AccountError::InvalidProxy`]: `proxy` sai dinh dang ([`parse_proxy`]) HOAC (hiem) proxy
    ///   chung dang cau hinh cung sai dinh dang khi `proxy` rong ([`effective_proxy`]).
    /// - [`AccountError::CookieInvalid`]: khong tim thay cookie `dola.com` nao / thieu `sessionid`
    ///   hop le ([`lvc_core::cookie::validate_dola_cookies`]).
    /// - [`AccountError::AccountBusy`]: Minor 4 (review fix-round 1) — ten (hop le, CHUA co thu muc)
    ///   dang duoc mot `import_cookies`/tac vu khac thue cung luc (cuc hiem — hai lan goi trung ten
    ///   moi cung luc); giu `Lease` suot qua trinh nhap de scheduler dang chay khong "vo tinh" cap
    ///   lay thu muc profile dang nhap do (`rows_with_dirs` tu them dong `accounts_meta` ngay khi
    ///   thu muc xuat hien, du chua nhap xong).
    /// - [`AccountError::AccountExists`]: thu muc `accounts_dir/<name>` da ton tai.
    /// - [`AccountError::CookieNotLoggedIn`]: dat cookie xong nhung khong dang nhap duoc (thu muc DA
    ///   BI XOA truoc khi tra loi nay).
    /// - [`AccountError::Browser`]/[`AccountError::Io`]/[`AccountError::Store`]: loi tang duoi (thu
    ///   muc da tao cung bi XOA truoc khi tra loi, TRU khi chinh viec xoa cung loi — hiem, giu
    ///   nguyen thu muc de nguoi dung tu don).
    pub async fn import_cookies(
        &self,
        name: &str,
        cookie_text: &str,
        proxy: &str,
    ) -> Result<ImportCookiesReport, AccountError> {
        let name = validate_name(name)?;
        let proxy = proxy.trim();
        // Kiem tra THUAN truoc (khong dung dia/trinh duyet) — that bai o day khong tao rac gi ca.
        parse_proxy(proxy).map_err(|e| AccountError::InvalidProxy(e.to_string()))?;
        let cookies = lvc_core::cookie::parse_netscape(cookie_text);
        let dola_cookies = lvc_core::cookie::validate_dola_cookies(&cookies)
            .map_err(|e| AccountError::CookieInvalid(e.to_string()))?;
        let sessionid_expires_at = lvc_core::cookie::sessionid_expires(&dola_cookies);

        // Minor 4 (review fix-round 1): thue tai khoan MOI suot qua trinh nhap — xem doc # Errors
        // o tren. Giu `_lease` toi cuoi ham (RAII) bang KHONG dung `_` don doc — dat ten ro de
        // clippy/nguoi doc khong tuong nham la bien thua.
        let Some(_lease) = self.pool().try_lease(name, "") else {
            return Err(AccountError::AccountBusy(name.to_string()));
        };

        let dir = self.cfg.accounts_dir.join(name);
        if dir.exists() {
            return Err(AccountError::AccountExists(name.to_string()));
        }
        tokio::fs::create_dir_all(&dir).await?;

        match self
            .import_cookies_on_dir(name, &dir, &dola_cookies, proxy)
            .await
        {
            Ok(()) => {
                // Minor 6 (review fix-round 1): luu han cookie sessionid (Python
                // `_run_add_cookie_job` server.py:1497) — chi khi doc duoc tu cookie text.
                if let Some(exp) = sessionid_expires_at {
                    #[allow(
                        clippy::cast_precision_loss,
                        reason = "epoch giay thuc te < 2^53, ep sang f64 khong mat chinh xac"
                    )]
                    let exp_f64 = exp as f64;
                    self.pool()
                        .repo()
                        .set_cookie_expires_at(name, exp_f64)
                        .await?;
                }
                Ok(ImportCookiesReport {
                    name: name.to_string(),
                    sessionid_expires_at,
                })
            }
            Err(e) => {
                if let Err(io_err) = tokio::fs::remove_dir_all(&dir).await {
                    warn!(
                        account = name,
                        error = %io_err,
                        "import_cookies: khong xoa duoc thu muc profile sau khi that bai"
                    );
                }
                Err(e)
            }
        }
    }

    /// Than `import_cookies` SAU khi thu muc profile da duoc tao — tach rieng de nhanh loi cua
    /// `import_cookies` chi co MOT cho xoa thu muc (o tren), khong lap lai o tung diem `return`.
    async fn import_cookies_on_dir(
        &self,
        name: &str,
        dir: &Path,
        dola_cookies: &[Cookie],
        proxy: &str,
    ) -> Result<(), AccountError> {
        let proxy_spec = effective_proxy(proxy, &self.cfg.global_proxy)
            .map_err(|e| AccountError::InvalidProxy(e.to_string()))?;
        let opts = LaunchOpts {
            profile_dir: dir.to_path_buf(),
            browser: BrowserKind::Chromium,
            executable_override: self.cfg.executable_override.clone(),
            headless: true,
            proxy: proxy_spec,
            logs_dir: self.cfg.logs_dir.clone(),
            ..LaunchOpts::default()
        };
        let session = self.driver.launch(&opts).await?;

        let outcome = self.import_cookies_probe(&session, dola_cookies).await;
        if let Err(e) = self.driver.close(session).await {
            warn!(account = name, error = %e, "import_cookies: khong dong duoc phien (bo qua)");
        }
        outcome?;

        let now = now_unix();
        let repo = self.pool().repo();
        repo.upsert(name).await?;
        repo.set_proxy(name, proxy).await?;
        repo.set_login_ok(name, true, now).await?;
        self.refresh_account_count().await?;
        Ok(())
    }

    /// `add_cookies` + [`Self::goto_chat_and_probe`] roi phan loai: `LoggedIn` -> Ok; `LoggedOut`
    /// HOAC `Unknown` (sau vai lan doc lai — KHAC `resume::resume_open`, o day khong co vong poll
    /// nao sau de "hoc lai" trang thai that su nen `Unknown` cung coi la khong chac chan == that
    /// bai) -> [`AccountError::CookieNotLoggedIn`].
    async fn import_cookies_probe(
        &self,
        session: &D::Session,
        cookies: &[Cookie],
    ) -> Result<(), AccountError> {
        self.driver.add_cookies(session, cookies).await?;
        let probe = self.goto_chat_and_probe(session).await?;
        match probe.state {
            SessionState::LoggedIn => Ok(()),
            SessionState::LoggedOut | SessionState::Unknown => Err(AccountError::CookieNotLoggedIn),
        }
    }

    /// Ruling task-10-context.md: kiem tra dang nhap tren CHINH profile that (KHONG copy) —
    /// tai khoan dang co tac vu dang chay (`AccountPool::try_lease` that bai) -> loi NGAY (khong
    /// cho — khac `lease_wait` cua pipeline video, day la nguoi dung bam nut Kiem tra, muon biet
    /// ngay). `probe_session`: `LoggedIn`/`LoggedOut` -> ghi `login_ok` tuong ung (`login_checked_at
    /// = now`); `Unknown` -> KHONG doi DB (trang chua tai xong, khong the ket luan — giong
    /// `lvc_core::classify_session` da noi ro), chi tra `state` de UI hien "chua ro, thu lai sau".
    ///
    /// I2 (review fix-round 1): loai + dong PHIEN AM (`SessionCache`, neu co) cua tai khoan nay
    /// TRUOC khi mo phien headless rieng cho probe — phien am giu lockfile o muc he dieu hanh MA
    /// KHONG qua `Lease`, nen truoc ban sua nay `driver.launch()` duoi day gap `ProfileBusy` trong
    /// suot `warm_idle` (mac dinh 900 s) sau MOI tac vu tao video tren tai khoan do — dung lung
    /// nhat ngay luc nguoi dung vua bam "Kiem tra" sau khi chay xong.
    ///
    /// # Errors
    /// - [`AccountError::AccountNotFound`]: khong co dong `accounts_meta` cho `name`.
    /// - [`AccountError::InvalidProxy`]: Minor 3 (review fix-round 1) — `accounts_meta.proxy` cua
    ///   dong nay khong con hop le (vd nguoi dung tung ghi truc tiep DB) — kiem TRUOC khi thue de
    ///   khong bao lam `PROFILE_BUSY` (nhu ban cu: `try_lease` tra `None` CA khi proxy hong LAN khi
    ///   dang ban, khong phan biet duoc hai truong hop).
    /// - [`AccountError::ProfileBusy`]: tai khoan dang co tac vu khac giu (lease/OS-level Chrome
    ///   lock — ca hai deu ra ma `PROFILE_BUSY`, xem [`AccountError::code`]).
    /// - [`AccountError::Browser`]: launch/goto/probe loi CDP.
    /// - [`AccountError::Store`]: ghi `login_ok` loi.
    pub async fn verify(&self, name: &str) -> Result<VerifyReport, AccountError> {
        let name = validate_name(name)?;
        let row = self
            .pool()
            .account_row(name)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound(name.to_string()))?;
        // Minor 3: kiem dinh dang proxy TRUOC try_lease (xem doc # Errors o tren).
        let proxy_spec = effective_proxy(&row.proxy, &self.cfg.global_proxy)
            .map_err(|e| AccountError::InvalidProxy(e.to_string()))?;
        let Some(lease) = self.pool().try_lease(name, &row.proxy) else {
            return Err(AccountError::ProfileBusy(format!(
                "tai khoan '{name}' dang co tac vu khac dang chay"
            )));
        };

        // I2: xem doc ham o tren.
        if let Some(s) = self.cache().remove(name).await {
            self.close(name, s).await;
        }

        let browser = effective_browser(name, &row.browser);
        let opts = LaunchOpts {
            profile_dir: self.cfg.accounts_dir.join(name),
            browser,
            executable_override: self.cfg.executable_override.clone(),
            headless: true,
            proxy: proxy_spec,
            logs_dir: self.cfg.logs_dir.clone(),
            ..LaunchOpts::default()
        };
        let session = self.driver.launch(&opts).await?;
        let probe_result = self.goto_chat_and_probe(&session).await;
        if let Err(e) = self.driver.close(session).await {
            warn!(account = name, error = %e, "verify: khong dong duoc phien (bo qua)");
        }
        let probe = probe_result?;
        drop(lease);

        let now = now_unix();
        let login_ok = match probe.state {
            SessionState::LoggedIn => {
                self.pool().repo().set_login_ok(name, true, now).await?;
                Some(true)
            }
            SessionState::LoggedOut => {
                self.pool().repo().set_login_ok(name, false, now).await?;
                Some(false)
            }
            SessionState::Unknown => None,
        };
        Ok(VerifyReport {
            state: probe.state,
            login_ok,
            checked_at: now,
        })
    }

    /// Bat/tat lich mot tai khoan roi bao lai [`Self::refresh_account_count`] (context.md: goi SAU
    /// bat/tat lich). Minor 8 (review fix-round 1): `name` HOP LE nhung CHUA co dong `accounts_meta`
    /// (Python `browser_pool.py:703` `_ensure_meta` truoc khi UPDATE) -> tu tao dong (parity), KHONG
    /// im lang khong lam gi nhu ban cu.
    ///
    /// # Errors
    /// [`AccountError::InvalidName`] khi `name` khong hop le (C1); `Store`/`Engine` khi ghi DB hoac
    /// dem lai so tai khoan loi.
    pub async fn set_scheduling(&self, name: &str, on: bool) -> Result<(), AccountError> {
        let name = validate_name(name)?;
        self.pool().repo().upsert(name).await?;
        self.pool().repo().set_scheduling(name, on).await?;
        self.refresh_account_count().await?;
        Ok(())
    }

    /// browser_pool.py:699-705 `set_proxy`: `text.strip()`; sai dinh dang -> loi ([`parse_proxy`],
    /// thong diep khop Python); `""` = dung proxy chung; `"direct"` = khong dung proxy nao. Luu
    /// CHUOI DA TRIM (khong phai gia tri da chuan hoa/mask — `list_with_status` moi mask luc doc).
    /// Minor 8 (review fix-round 1): `name` HOP LE nhung CHUA co dong -> tu tao (`_ensure_meta`,
    /// xem doc [`Self::set_scheduling`]).
    ///
    /// # Errors
    /// [`AccountError::InvalidName`] khi `name` khong hop le (C1); [`AccountError::InvalidProxy`]
    /// khi sai dinh dang; `Store` khi ghi DB loi.
    pub async fn set_proxy(&self, name: &str, proxy: &str) -> Result<(), AccountError> {
        let name = validate_name(name)?;
        let trimmed = proxy.trim();
        parse_proxy(trimmed).map_err(|e| AccountError::InvalidProxy(e.to_string()))?;
        self.pool().repo().upsert(name).await?;
        self.pool().repo().set_proxy(name, trimmed).await?;
        Ok(())
    }

    /// Xoa dong `accounts_meta` + thu muc profile. Dang co lease (tac vu khac dang chay) ->
    /// [`AccountError::AccountBusy`] NGAY (khong cho). `name` khong ton tai ca dong lan thu muc ->
    /// `Ok(false)` (idempotent, giong `workflow_delete`), KHONG loi.
    ///
    /// I1 (review fix-round 1): thu tu DUNG nhu Python `browser_pool.py:827-840 delete_account`
    /// (khoa -> `close_session` -> `shutil.rmtree` -> `DELETE FROM accounts_meta`) thay vi
    /// "dong DB truoc" cua ban cu: (1) loai + dong PHIEN AM trong `SessionCache` neu co (giu OS-level
    /// lockfile MA KHONG qua `Lease` — trung ly do I2), (2) xoa thu muc (thu lai toi da
    /// [`REMOVE_DIR_RETRIES`] lan — Windows co the con giu handle mot chut sau khi dong phien), (3)
    /// CHI xoa dong DB SAU KHI thu muc da that su het. Neu buoc (2) that bai ->
    /// [`AccountError::ProfileBusy`] va KHONG CO GI bi doi (dong DB VAN CON — khac ban cu, dong da
    /// mat truoc khi biet xoa thu muc co thanh cong hay khong): nguoi dung thu lai `delete` se sach,
    /// row/dir khong bao gio lech nhau.
    ///
    /// I5 (fix-round 1, final-review): TRUOC `remove_dir_all_retry`, kiem them
    /// [`looks_like_chromium_profile`] — mot `accountsDir` bi chon nham (vd nguoi dung tro
    /// Settings vao goc thu muc source code thay vi `.../lvcseedance/accounts`) se khien
    /// [`crate::pool::AccountPool::rows_with_dirs`] tu dong `upsert` MOI thu muc con thanh mot dong
    /// `accounts_meta` (port Python `_ensure_meta`); bam "Xoá" tren mot dong nhu vay TRUOC ban sua
    /// nay se `remove_dir_all` NGUYEN thu muc do (vd `src/`, `.git/`...) sau hai lan xac nhan cua
    /// UI. Gio chi xoa khi thu muc RONG hoac trong nhu profile Chromium that; nguoc lai tra
    /// [`AccountError::NotAProfile`] va GIU nguyen dong DB (nguoi dung sua `accountsDir` roi thu
    /// lai).
    ///
    /// # Errors
    /// [`AccountError::InvalidName`] khi `name` khong hop le (C1); [`AccountError::AccountBusy`]/
    /// [`AccountError::ProfileBusy`] nhu tren; [`AccountError::NotAProfile`] (I5) khi thu muc
    /// khong rong va khong giong profile Chromium; `Store`/`Engine` khi xoa dong hoac dem lai so
    /// tai khoan loi.
    pub async fn delete(&self, name: &str) -> Result<bool, AccountError> {
        let name = validate_name(name)?;
        let Some(lease) = self.pool().try_lease(name, "") else {
            return Err(AccountError::AccountBusy(name.to_string()));
        };
        // I1: loai + dong phien am TRUOC (xem doc ham o tren).
        if let Some(s) = self.cache().remove(name).await {
            self.close(name, s).await;
        }
        let dir = self.cfg.accounts_dir.join(name);
        let dir_existed = dir.exists();
        if dir_existed {
            // I5 (fix-round 1, final-review): CHI `remove_dir_all` khi thu muc RONG hoac "trong
            // nhu" mot profile Chromium — xem doc [`AccountError::NotAProfile`]. Dong DB CHUA bi
            // dung toi o nhanh nay (kiem TRUOC ca `remove_dir_all_retry`).
            let looks_ok = looks_like_chromium_profile(&dir).map_err(|e| {
                AccountError::ProfileBusy(format!("khong doc duoc thu muc profile '{name}': {e}"))
            })?;
            if !looks_ok {
                return Err(AccountError::NotAProfile(dir.display().to_string()));
            }
            remove_dir_all_retry(&dir).await.map_err(|e| {
                AccountError::ProfileBusy(format!("khong xoa duoc thu muc profile '{name}': {e}"))
            })?;
        }
        let row_existed = self.pool().repo().delete(name).await?;
        drop(lease);
        let existed = row_existed || dir_existed;
        if existed {
            self.refresh_account_count().await?;
        }
        Ok(existed)
    }

    /// R12: `lvc_store::legacy::import_legacy` (P0, CHI DOC `python_dir` — mo DB Python
    /// `?mode=ro&immutable=1`, KHONG BAO GIO ghi vao do) roi copy them thu muc `python_dir/accounts/
    /// <name>` sang `accounts_dir` cho nhung tai khoan CHUA co thu muc (P0 chi nhap DB, chua copy
    /// profile — xem `task-10-context.md`).
    ///
    /// # Errors
    /// `Store` khi doc/ghi DB loi; [`AccountError::Io`] khi doc thu muc `accounts/` (liet ke) that
    /// bai (KHONG tinh thieu thu muc `accounts/` — coi nhu khong co profile nao de copy, `Ok(0)`;
    /// loi copy/doi ten TUNG profile rieng khong lam that bai ca ham — I3, xem doc
    /// [`Self::copy_legacy_profiles`]).
    pub async fn import_legacy(
        &self,
        python_dir: &Path,
    ) -> Result<LegacyImportReport, AccountError> {
        let report = lvc_store::legacy::import_legacy(&self.db, python_dir).await?;
        let (profiles_copied, profiles_skipped) = self.copy_legacy_profiles(python_dir).await?;
        self.refresh_account_count().await?;
        Ok(LegacyImportReport {
            tasks: report.tasks,
            accounts: report.accounts,
            usage: report.usage,
            settings: report.settings,
            skipped: report.skipped,
            profiles_copied,
            profiles_skipped,
            scripts: report.scripts,
            scenes: report.scenes,
        })
    }

    /// I3 (review fix-round 1): copy TUNG profile qua thu muc TAM `accounts_dir/.import-<name>`
    /// (dau `.` -> vo hinh voi `AccountPool::profile_dirs`/liet ke tai khoan) TRUOC, `rename` sang
    /// `accounts_dir/<name>` CHI khi copy thanh cong. That bai giua chung (vd profile con dang mo
    /// boi Chrome cua ban Python cu — khong the doc het file khoa) -> XOA SACH thu muc tam, dem ten
    /// do vao `profiles_skipped`, tiep tuc profile KE TIEP (khong dung ca vong for nhu ban cu — mot
    /// profile hong khong duoc lam mat cac profile con lai). Truoc ban sua nay, mot ban copy DO DANG
    /// (loi giua chung) de lai `accounts_dir/<name>` bi coi la "da co" o LAN CHAY SAU — profile hong
    /// vinh vien bi bo qua (rack — khong cookie jar) MA VAN duoc `rows_with_dirs` lich cho chay.
    ///
    /// C1: ten thu muc con KHONG khop `NAME_RE` (belt-and-braces — profile Python thuong luon hop
    /// le, nhung khong tin tuong mu quang du lieu ben ngoai) cung bi bo qua + dem vao
    /// `profiles_skipped`, khong loi ca luot import.
    async fn copy_legacy_profiles(
        &self,
        python_dir: &Path,
    ) -> Result<(usize, Vec<String>), AccountError> {
        let src_accounts = python_dir.join("accounts");
        // Khong co thu muc accounts/ trong ban Python cu -> khong co gi de copy (khong phai loi).
        let Ok(mut rd) = tokio::fs::read_dir(&src_accounts).await else {
            return Ok((0, Vec::new()));
        };
        let mut copied = 0usize;
        let mut skipped = Vec::new();
        while let Some(entry) = rd.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let name_os = entry.file_name();
            let name_lossy = name_os.to_string_lossy();
            if name_lossy.starts_with('.') {
                continue;
            }
            let Ok(name) = validate_name(&name_lossy) else {
                warn!(name = %name_lossy, "import_legacy: bo qua thu muc profile ten khong hop le");
                skipped.push(name_lossy.into_owned());
                continue;
            };
            let dest = self.cfg.accounts_dir.join(name);
            if dest.exists() {
                // Chi copy khi CHUA co san — khong bao gio ghi de profile hien tai.
                skipped.push(name.to_string());
                continue;
            }
            let staging = self.cfg.accounts_dir.join(format!(".import-{name}"));
            if let Err(e) = clear_stale_staging(&staging).await {
                warn!(account = name, error = %e, "import_legacy: khong don duoc thu muc tam sot lai");
                skipped.push(name.to_string());
                continue;
            }
            if let Err(e) = copy_dir_all(&entry.path(), &staging).await {
                warn!(account = name, error = %e, "import_legacy: copy profile that bai (co the dang mo boi Chrome cua ban Python cu)");
                let _ = tokio::fs::remove_dir_all(&staging).await;
                skipped.push(name.to_string());
                continue;
            }
            match tokio::fs::rename(&staging, &dest).await {
                Ok(()) => copied += 1,
                Err(e) => {
                    warn!(account = name, error = %e, "import_legacy: khong doi ten duoc thu muc tam sang dich");
                    let _ = tokio::fs::remove_dir_all(&staging).await;
                    skipped.push(name.to_string());
                }
            }
        }
        Ok((copied, skipped))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use lvc_browser::{FakeCall, FakeStep};
    use lvc_core::Mode;
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{ConnectOptions, Executor};
    use tracing_subscriber::fmt::MakeWriter;

    use super::*;
    use crate::generate::tests::fx;

    const COOKIE_TEXT: &str = ".dola.com\tTRUE\t/\tTRUE\t1800000000\tsessionid\tabc123\n";

    // ---- C1: validate_name (thuan, khong can Fx) ----

    #[test]
    fn validate_name_chan_cac_ten_nguy_hiem() {
        for bad in ["", "..", "a/b", r"C:\x", &"a".repeat(33)] {
            assert!(validate_name(bad).is_err(), "{bad:?} phai bi tu choi");
        }
    }

    #[test]
    fn validate_name_chap_nhan_ten_hop_le() {
        for good in ["a", "acc_1", "ACC-2", &"a".repeat(32)] {
            assert_eq!(validate_name(good).unwrap(), good, "{good:?}");
        }
    }

    // ---- I1: remove_dir_all_retry (thuan, khong can Fx) ----

    #[tokio::test]
    async fn remove_dir_all_retry_thanh_cong_ngay_lan_dau() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("x");
        std::fs::create_dir_all(&target).unwrap();
        remove_dir_all_retry(&target).await.unwrap();
        assert!(!target.exists());
    }

    #[tokio::test]
    async fn remove_dir_all_retry_loi_sau_khi_het_luot_thu_lai() {
        let dir = tempfile::tempdir().unwrap();
        let ghost = dir.path().join("khong-ton-tai");
        let err = remove_dir_all_retry(&ghost).await.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    // ---- import_cookies (R11) ----

    #[tokio::test]
    async fn import_cookies_logged_in_giu_thu_muc_ghi_dong() {
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;
        let report = f
            .engine
            .import_cookies("acc_new", COOKIE_TEXT, "1.2.3.4:8080")
            .await
            .unwrap();
        assert_eq!(report.name, "acc_new");
        assert_eq!(report.sessionid_expires_at, Some(1_800_000_000));
        assert!(f.accounts_dir().join("acc_new").is_dir());

        let row = f.accounts.get("acc_new").await.unwrap().unwrap();
        assert!(row.login_ok);
        assert_eq!(row.proxy, "1.2.3.4:8080");
        assert!(
            (row.cookie_expires_at - 1_800_000_000.0).abs() < f64::EPSILON,
            "Minor 6: han cookie sessionid phai duoc luu vao DB"
        );

        assert_eq!(f.calls(), vec![f.launch("acc_new"), FakeCall::Close]);
    }

    #[tokio::test]
    async fn import_cookies_logged_out_xoa_thu_muc_va_loi() {
        let f = fx(
            &[],
            vec![FakeStep::LaunchWithState(Ok(SessionState::LoggedOut))],
        )
        .await;
        let err = f
            .engine
            .import_cookies("acc_new", COOKIE_TEXT, "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "COOKIE_NOT_LOGGED_IN");
        assert!(!f.accounts_dir().join("acc_new").exists());
        assert!(f.accounts.get("acc_new").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn import_cookies_probe_unknown_cung_tinh_la_khong_dang_nhap() {
        let f = fx(
            &[],
            vec![FakeStep::LaunchWithState(Ok(SessionState::Unknown))],
        )
        .await;
        let err = f
            .engine
            .import_cookies("acc_new", COOKIE_TEXT, "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "COOKIE_NOT_LOGGED_IN");
        assert!(!f.accounts_dir().join("acc_new").exists());
    }

    #[tokio::test]
    async fn import_cookies_thu_muc_da_ton_tai_bao_loi() {
        let f = fx(&["acc1"], vec![]).await;
        let err = f
            .engine
            .import_cookies("acc1", COOKIE_TEXT, "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "ACCOUNT_EXISTS");
    }

    #[tokio::test]
    async fn import_cookies_khong_co_cookie_dola_bao_loi() {
        let f = fx(&[], vec![]).await;
        let err = f
            .engine
            .import_cookies("acc_new", "khong phai cookie hop le", "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "COOKIE_INVALID");
        assert!(!f.accounts_dir().join("acc_new").exists());
    }

    #[tokio::test]
    async fn import_cookies_proxy_sai_dinh_dang_bao_loi() {
        let f = fx(&[], vec![]).await;
        let err = f
            .engine
            .import_cookies("acc_new", COOKIE_TEXT, "khong-hop-le")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_PROXY");
        assert!(!f.accounts_dir().join("acc_new").exists());
    }

    /// C1 (review fix-round 1, Critical): ten khong hop le phai bao `INVALID_NAME` va KHONG DUNG TOI
    /// dia — RED truoc ban sua: `import_cookies("../x", ..)` se `create_dir_all` mot thu muc BEN
    /// NGOAI `accounts_dir` (`Path::join` voi thanh phan `".."`).
    #[tokio::test]
    async fn import_cookies_ten_khong_hop_le_bao_loi_khong_dung_toi_dia() {
        let f = fx(&[], vec![]).await;
        for bad in ["", "..", "a/b", r"C:\x", &"a".repeat(33)] {
            let err = f
                .engine
                .import_cookies(bad, COOKIE_TEXT, "")
                .await
                .unwrap_err();
            assert_eq!(err.code(), "INVALID_NAME", "{bad:?}");
        }
        // "../x" dac biet nguy hiem (thoat khoi accounts_dir that su) — kiem rieng KHONG co thu muc
        // "x" nao xuat hien o thu muc CHA cua accounts_dir.
        let err = f
            .engine
            .import_cookies("../x", COOKIE_TEXT, "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_NAME");
        assert!(!f.dir.path().join("x").exists());
        assert!(!f.accounts_dir().join("x").exists());
    }

    /// Minor 4 (review fix-round 1): ten MOI (hop le, chua co thu muc) nhung dang bi mot `Lease`
    /// khac giu -> `ACCOUNT_BUSY`, khong tao thu muc nao.
    #[tokio::test]
    async fn import_cookies_ten_dang_bi_thue_bao_account_busy() {
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;
        let lease = f.engine.pool().try_lease("acc_new", "").unwrap();
        let err = f
            .engine
            .import_cookies("acc_new", COOKIE_TEXT, "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "ACCOUNT_BUSY");
        assert!(!f.accounts_dir().join("acc_new").exists());
        drop(lease);
        // tha khoa xong thi nhap binh thuong duoc.
        f.engine
            .import_cookies("acc_new", COOKIE_TEXT, "")
            .await
            .unwrap();
    }

    /// Ghi lai moi dong log qua mot `MakeWriter` tuy bien (buffer trong bo nho) — dung boi test
    /// `import_cookies_khong_log_gia_tri_cookie` de kiem THAT SU (khong chi doc code) rang gia tri
    /// cookie khong bao gio lot vao `tracing::*!`.
    #[derive(Clone, Default)]
    struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for CaptureWriter {
        type Writer = Self;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Bat MOI dong log phat ra trong luc goi `import_cookies` (kem canary TRUOC do de chung minh
    /// co che bat log hoat dong that su — khong phai buffer luon rong nen assert "khong thay" luon
    /// dung mot cach vo nghia) roi khang dinh gia tri cookie KHONG xuat hien.
    #[tokio::test]
    async fn import_cookies_khong_log_gia_tri_cookie() {
        let marker = "GIA_TRI_COOKIE_TUYET_MAT_KHONG_DUOC_LOG_RA_NGOAI";
        let cookie_text = format!(".dola.com\tTRUE\t/\tTRUE\t0\tsessionid\t{marker}\n");
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;

        let capture = CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture.clone())
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish();
        let guard = tracing::subscriber::set_default(subscriber);

        // Canary: chung minh capture THAT SU hoat dong truoc khi tin vao "khong thay marker".
        tracing::warn!("canary-truoc-khi-nhap-cookie");
        f.engine
            .import_cookies("acc_new", &cookie_text, "")
            .await
            .unwrap();
        drop(guard);

        let logged = String::from_utf8_lossy(&capture.0.lock().unwrap()).to_string();
        assert!(
            logged.contains("canary-truoc-khi-nhap-cookie"),
            "capture khong hoat dong: {logged}"
        );
        assert!(
            !logged.contains(marker),
            "khong duoc log gia tri cookie: {logged}"
        );
    }

    // ---- verify ----

    #[tokio::test]
    async fn verify_logged_in_ghi_login_ok_true() {
        let f = fx(&["a"], vec![FakeStep::Launch(Ok(()))]).await;
        f.accounts.set_login_ok("a", false, 1.0).await.unwrap();

        let report = f.engine.verify("a").await.unwrap();
        assert_eq!(report.state, SessionState::LoggedIn);
        assert_eq!(report.login_ok, Some(true));
        assert!(f.accounts.get("a").await.unwrap().unwrap().login_ok);
    }

    #[tokio::test]
    async fn verify_logged_out_ghi_login_ok_false() {
        let f = fx(
            &["a"],
            vec![FakeStep::LaunchWithState(Ok(SessionState::LoggedOut))],
        )
        .await;

        let report = f.engine.verify("a").await.unwrap();
        assert_eq!(report.state, SessionState::LoggedOut);
        assert_eq!(report.login_ok, Some(false));
        assert!(!f.accounts.get("a").await.unwrap().unwrap().login_ok);
    }

    #[tokio::test]
    async fn verify_unknown_khong_doi_db() {
        let f = fx(
            &["a"],
            vec![FakeStep::LaunchWithState(Ok(SessionState::Unknown))],
        )
        .await;

        let report = f.engine.verify("a").await.unwrap();
        assert_eq!(report.state, SessionState::Unknown);
        assert_eq!(report.login_ok, None);
        assert!(
            f.accounts.get("a").await.unwrap().unwrap().login_ok,
            "Unknown khong duoc doi DB — mac dinh login_ok=true phai con nguyen"
        );
    }

    #[tokio::test]
    async fn verify_dang_co_lease_bao_profile_busy() {
        let f = fx(&["a"], vec![]).await;
        let lease = f.engine.pool().try_lease("a", "").unwrap();
        let err = f.engine.verify("a").await.unwrap_err();
        assert_eq!(err.code(), "PROFILE_BUSY");
        drop(lease);
        assert!(f.calls().is_empty(), "khong duoc dung browser khi dang ban");
    }

    #[tokio::test]
    async fn verify_tai_khoan_khong_ton_tai_bao_loi() {
        let f = fx(&[], vec![]).await;
        let err = f.engine.verify("ghost").await.unwrap_err();
        assert_eq!(err.code(), "ACCOUNT_NOT_FOUND");
    }

    /// C1: ten khong hop le bi tu choi TRUOC ca khi doc DB (khong tao dong `accounts_meta` nao).
    #[tokio::test]
    async fn verify_ten_khong_hop_le_bao_loi() {
        let f = fx(&[], vec![]).await;
        let err = f.engine.verify("..").await.unwrap_err();
        assert_eq!(err.code(), "INVALID_NAME");
    }

    /// I2 (review fix-round 1, Important): mot phien am (`SessionCache`) da co san cho tai khoan nay
    /// (vd de lai sau mot lan tao video) — `verify` PHAI loai + dong no TRUOC khi mo phien headless
    /// rieng cho probe (khong thi `driver.launch` tren CHINH profile do se dinh `ProfileBusy` do
    /// chinh phien am dang giu lockfile). RED truoc ban sua: phien am van con trong cache, va
    /// `verify` se panic ("kich ban da het") hoac (voi driver that) tra `PROFILE_BUSY` sai.
    #[tokio::test]
    async fn verify_loai_phien_am_trong_cache_truoc_khi_probe() {
        let f = fx(
            &["a"],
            vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))],
        )
        .await;
        let warm = f
            .engine
            .driver
            .launch(&LaunchOpts {
                profile_dir: f.accounts_dir().join("a"),
                ..LaunchOpts::default()
            })
            .await
            .unwrap();
        f.engine.cache().put("a", warm).await;
        assert_eq!(f.engine.cache().len().await, 1);

        let report = f.engine.verify("a").await.unwrap();
        assert_eq!(report.state, SessionState::LoggedIn);
        assert!(
            f.engine.cache().is_empty().await,
            "phien am phai bi loai khoi cache"
        );
        // Launch (thu cong, phien am) -> Close (I2 loai) -> Launch (verify, headless) -> Close (verify xong).
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                FakeCall::Close,
                f.launch("a"),
                FakeCall::Close
            ]
        );
    }

    /// Minor 3 (review fix-round 1): `row.proxy` hong (nguoi dung tung ghi thang vao DB) phai ra
    /// `INVALID_PROXY`, KHONG phai `PROFILE_BUSY` — ban cu: `try_lease` tra `None` ca hai truong
    /// hop (ban VA proxy hong) nen khong phan biet duoc, luon bao ban.
    #[tokio::test]
    async fn verify_proxy_hong_bao_invalid_proxy_khong_phai_profile_busy() {
        let f = fx(&["a"], vec![]).await;
        f.accounts.set_proxy("a", "khong-hop-le").await.unwrap();
        let err = f.engine.verify("a").await.unwrap_err();
        assert_eq!(err.code(), "INVALID_PROXY");
        assert!(
            f.calls().is_empty(),
            "khong duoc dung browser khi proxy hong"
        );
    }

    // ---- list_with_status ----

    #[tokio::test]
    async fn list_with_status_tinh_dung_cac_co_va_luot_dung() {
        let f = fx(&["a", "b"], vec![]).await;
        let now = 10_000.0;
        let day = lvc_core::tokyo_day(now);

        f.accounts
            .set_cooldown_until("a", now + 100.0)
            .await
            .unwrap();
        f.accounts
            .set_rate_limited_until("a", now + 200.0, "daily limit")
            .await
            .unwrap();
        f.accounts
            .set_proxy("a", "1.2.3.4:8080:user:pass")
            .await
            .unwrap();
        f.accounts.bump_usage("a", &day, Mode::Fast).await.unwrap();
        f.accounts.bump_usage("a", &day, Mode::Fast).await.unwrap();

        f.accounts
            .set_quota_blocked_until("b", now + 50.0, "insufficient credits")
            .await
            .unwrap();
        f.accounts.bump_usage("b", &day, Mode::Pro).await.unwrap();

        let list = f.engine.list_with_status(now).await.unwrap();
        assert_eq!(list.len(), 2);

        let a = list.iter().find(|s| s.name == "a").unwrap();
        assert!(a.cooling);
        assert!(a.rate_limited);
        assert_eq!(a.limit_reason, "daily limit");
        assert!(!a.quota_blocked);
        assert_eq!(a.used_today, 2);
        assert_eq!(a.limit, 30, "Limits::default daily_limit_fast");
        assert_eq!(a.remaining, 28);
        assert_eq!(a.proxy, "1.2.3.4:8080:user:***", "proxy phai duoc mask");
        assert!(a.proxy_set);
        assert_eq!(
            a.browser, "chromium",
            "I4: browser HIEU LUC (cot tho rong -> chromium), khong phai chuoi rong"
        );

        let b = list.iter().find(|s| s.name == "b").unwrap();
        assert!(!b.cooling);
        assert!(!b.rate_limited);
        assert!(b.quota_blocked);
        assert_eq!(b.quota_reason, "insufficient credits");
        assert_eq!(b.used_today, 0);
        assert_eq!(b.used_pro, 1);
        assert_eq!(b.limit_pro, 10, "Limits::default daily_limit_pro");
        assert!(!b.proxy_set);
        assert_eq!(b.proxy, "");
    }

    /// I4 + Minor 2 (review fix-round 1): gia tri `browser` khong hop le trong DB (vd nguoi dung
    /// ghi thang) van phai tra ve `"chromium"` (khong panic/loi ca danh sach), va co `warn!` (kiem
    /// gian tiep qua hanh vi — khong crash/khong chuoi rong la du, log cu the da co test rieng
    /// cho `import_cookies` chung minh co che bat log hoat dong).
    #[tokio::test]
    async fn list_with_status_browser_khong_hop_le_thi_ve_chromium() {
        let f = fx(&["a"], vec![]).await;
        f.accounts.set_browser("a", "bogus").await.unwrap();
        let list = f.engine.list_with_status(0.0).await.unwrap();
        assert_eq!(list[0].browser, "chromium");
    }

    // ---- set_proxy / set_scheduling / delete ----

    #[tokio::test]
    async fn set_proxy_sai_dinh_dang_bao_loi_rong_va_direct_ok() {
        let f = fx(&["a"], vec![]).await;
        let err = f.engine.set_proxy("a", "khong-hop-le").await.unwrap_err();
        assert_eq!(err.code(), "INVALID_PROXY");

        f.engine.set_proxy("a", "").await.unwrap();
        assert_eq!(f.accounts.get("a").await.unwrap().unwrap().proxy, "");

        f.engine.set_proxy("a", "direct").await.unwrap();
        assert_eq!(f.accounts.get("a").await.unwrap().unwrap().proxy, "direct");
    }

    /// C1: ten khong hop le tu choi TRUOC khi dung DB.
    #[tokio::test]
    async fn set_proxy_ten_khong_hop_le_bao_loi() {
        let f = fx(&[], vec![]).await;
        let err = f.engine.set_proxy("..", "").await.unwrap_err();
        assert_eq!(err.code(), "INVALID_NAME");
    }

    /// Minor 8 (review fix-round 1): ten HOP LE nhung CHUA co dong `accounts_meta` nao (khac cac
    /// test khac dung `fx(&["a"], ..)` da co san dong) — `set_proxy`/`set_scheduling` phai TU TAO
    /// dong (Python `_ensure_meta`), khong im lang bo qua.
    #[tokio::test]
    async fn set_proxy_va_set_scheduling_tu_tao_dong_khi_ten_hop_le_nhung_chua_co() {
        let f = fx(&[], vec![]).await;
        assert!(f.accounts.get("moi").await.unwrap().is_none());

        f.engine.set_proxy("moi", "direct").await.unwrap();
        assert_eq!(
            f.accounts.get("moi").await.unwrap().unwrap().proxy,
            "direct"
        );

        f.engine.set_scheduling("moi2", false).await.unwrap();
        assert!(!f.accounts.get("moi2").await.unwrap().unwrap().scheduling);
    }

    #[tokio::test]
    async fn set_scheduling_lat_anh_huong_scheduling_count() {
        let f = fx(&["a", "b"], vec![]).await;
        assert_eq!(f.engine.pool().scheduling_count().await.unwrap(), 2);

        f.engine.set_scheduling("a", false).await.unwrap();
        assert_eq!(f.engine.pool().scheduling_count().await.unwrap(), 1);
        assert!(!f.accounts.get("a").await.unwrap().unwrap().scheduling);

        f.engine.set_scheduling("a", true).await.unwrap();
        assert_eq!(f.engine.pool().scheduling_count().await.unwrap(), 2);
    }

    /// C1: ten khong hop le tu choi TRUOC khi dung DB.
    #[tokio::test]
    async fn set_scheduling_ten_khong_hop_le_bao_loi() {
        let f = fx(&[], vec![]).await;
        let err = f.engine.set_scheduling("..", true).await.unwrap_err();
        assert_eq!(err.code(), "INVALID_NAME");
    }

    #[tokio::test]
    async fn delete_xoa_dong_va_thu_muc_idempotent() {
        let f = fx(&["a"], vec![]).await;
        let dir = f.accounts_dir().join("a");
        assert!(dir.is_dir());

        assert!(f.engine.delete("a").await.unwrap());
        assert!(!dir.exists());
        assert!(f.accounts.get("a").await.unwrap().is_none());

        assert!(
            !f.engine.delete("a").await.unwrap(),
            "da xoa -> lan hai tra false, khong loi"
        );
    }

    #[tokio::test]
    async fn delete_dang_co_lease_bao_account_busy() {
        let f = fx(&["a"], vec![]).await;
        let lease = f.engine.pool().try_lease("a", "").unwrap();
        let err = f.engine.delete("a").await.unwrap_err();
        assert_eq!(err.code(), "ACCOUNT_BUSY");
        assert!(f.accounts_dir().join("a").exists(), "chua dong gi bi xoa");

        drop(lease);
        assert!(f.engine.delete("a").await.unwrap());
    }

    /// C1 (review fix-round 1, Critical): ten khong hop le ("" -> ca thu muc `accounts_dir`, ".." ->
    /// thu muc CHA cua no) phai bao `INVALID_NAME` va KHONG DUNG TOI FILESYSTEM. RED truoc ban sua:
    /// `delete("")` xoa TOAN BO `accounts_dir` (`accounts_dir.join("") == accounts_dir`),
    /// `delete("..")` xoa thu muc app-data (DB, settings, downloads) — kiem thuc nghiem trong
    /// review, xem `task-10-review.md` finding C1.
    #[tokio::test]
    async fn delete_ten_khong_hop_le_bao_loi_khong_dung_toi_dia() {
        let f = fx(&["sibling"], vec![]).await;
        for bad in ["", "..", "a/b", r"C:\x", &"a".repeat(33)] {
            let err = f.engine.delete(bad).await.unwrap_err();
            assert_eq!(err.code(), "INVALID_NAME", "{bad:?}");
        }
        // Neu ten rong/".." lot qua duoc validate_name, thu muc chi em nay se bi cuon theo
        // (remove_dir_all tren accounts_dir hoac thu muc cha cua no) — con nguyen la bang chung
        // KHONG dong nao bi dung toi.
        assert!(f.accounts_dir().join("sibling").is_dir());
        assert!(f.accounts.get("sibling").await.unwrap().is_some());
    }

    /// I1 (review fix-round 1, Important): phien am (`SessionCache`) cua tai khoan nay phai bi loai
    /// + dong TRUOC khi xoa thu muc (Windows giu lockfile suot khi Chrome con song). RED truoc ban
    /// sua: phien am van con trong cache sau `delete`, va (voi driver that) `remove_dir_all` se dinh
    /// loi "used by another process" vi chinh phien am chua dong do vua tao ra.
    #[tokio::test]
    async fn delete_loai_va_dong_phien_am_trong_cache_truoc_khi_xoa_thu_muc() {
        let f = fx(&["a"], vec![FakeStep::Launch(Ok(()))]).await;
        let warm = f
            .engine
            .driver
            .launch(&LaunchOpts {
                profile_dir: f.accounts_dir().join("a"),
                ..LaunchOpts::default()
            })
            .await
            .unwrap();
        f.engine.cache().put("a", warm).await;

        assert!(f.engine.delete("a").await.unwrap());
        assert!(f.engine.cache().is_empty().await, "phien am phai bi loai");
        assert_eq!(f.calls(), vec![f.launch("a"), FakeCall::Close]);
    }

    /// I1 (review fix-round 1): "on Windows lock failure the row is gone but the profile stays" —
    /// ban cu xoa dong DB TRUOC roi moi xoa thu muc, nen loi xoa thu muc de lai dong DB DA MAT trong
    /// khi thu muc con nguyen. Ban sua: xoa thu muc TRUOC — that bai o day thi KHONG CO GI bi doi
    /// (dong DB VAN CON), thu lai la sach. Dung khoa file THAT cua Windows (mo doc mot file trong
    /// thu muc profile — `remove_dir_all` se dinh "used by another process" toi khi handle dong).
    #[tokio::test]
    async fn delete_profile_busy_khi_thu_muc_dang_bi_giu_thi_khong_dong_db() {
        // `share_mode(0)` = KHONG chia se gi ca (mac dinh cua Rust std tren Windows la CO
        // FILE_SHARE_DELETE nen `remove_dir_all` van xoa duoc mot file dang mo binh thuong — phai
        // tu tat sharing de mo phong THAT SU mot process khac (Chrome) dang giu file doc quyen).
        use std::os::windows::fs::OpenOptionsExt;

        let f = fx(&["a"], vec![]).await;
        // I5 (fix-round 1, final-review): tu ban sua nay `delete` chi `remove_dir_all` khi thu muc
        // "trong nhu" mot profile Chromium — dat file bi khoa BEN TRONG `Default/` (khong phai
        // ngang hang) de dau hieu profile (`Default/` ton tai) SONG SOT qua lan `remove_dir_all`
        // that bai dau tien: no khong xoa duoc `locked.txt` nen khong xoa duoc ca `Default/` (con
        // con), khac voi dat file khoa ngang hang — luc do `remove_dir_all` co the da xoa xong
        // `Default/` (rong, xoa truoc) TRUOC khi dinh vao `locked.txt` rieng, khien lan goi lai
        // sau khong con thay dau hieu profile nao nua.
        let default_dir = f.accounts_dir().join("a").join("Default");
        std::fs::create_dir_all(&default_dir).unwrap();
        let locked_path = default_dir.join("locked.txt");
        std::fs::write(&locked_path, b"x").unwrap();
        let held = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&locked_path)
            .unwrap();

        let err = f.engine.delete("a").await.unwrap_err();
        assert_eq!(err.code(), "PROFILE_BUSY");
        assert!(
            f.accounts.get("a").await.unwrap().is_some(),
            "I1: dong DB KHONG duoc xoa khi xoa thu muc that bai"
        );
        assert!(f.accounts_dir().join("a").is_dir());

        drop(held);
        assert!(
            f.engine.delete("a").await.unwrap(),
            "tha khoa xong thi xoa lai duoc, sach"
        );
    }

    // ---- I5 (fix-round 1, final-review): looks_like_chromium_profile + delete NotAProfile ----

    #[test]
    fn looks_like_chromium_profile_thu_muc_khong_ton_tai_la_an_toan() {
        let dir = tempfile::tempdir().unwrap();
        assert!(looks_like_chromium_profile(&dir.path().join("khong-ton-tai")).unwrap());
    }

    #[test]
    fn looks_like_chromium_profile_thu_muc_rong_la_an_toan() {
        let dir = tempfile::tempdir().unwrap();
        assert!(looks_like_chromium_profile(dir.path()).unwrap());
    }

    #[test]
    fn looks_like_chromium_profile_co_default_la_profile_that() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Default")).unwrap();
        assert!(looks_like_chromium_profile(dir.path()).unwrap());
    }

    #[test]
    fn looks_like_chromium_profile_co_local_state_la_profile_that() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Local State"), b"{}").unwrap();
        assert!(looks_like_chromium_profile(dir.path()).unwrap());
    }

    #[test]
    fn looks_like_chromium_profile_thu_muc_source_code_khong_phai_profile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src").join("main.rs"), b"fn main() {}").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), b"[package]").unwrap();
        assert!(!looks_like_chromium_profile(dir.path()).unwrap());
    }

    /// I5: thu muc `accountsDir/<name>` KHONG rong VA khong giong profile Chromium (vd mot
    /// `accountsDir` bi chon nham tro thang vao goc source code, o day mo phong bang mot thu muc co
    /// `src/main.rs`) -> `NOT_A_PROFILE`, KHONG dung toi `remove_dir_all`, dong DB VAN CON (khac
    /// `ACCOUNT_BUSY`/`PROFILE_BUSY` — day la loi CAU HINH, khong phai loi tam thoi).
    #[tokio::test]
    async fn delete_thu_muc_khong_giong_profile_bao_not_a_profile_giu_ca_dong_va_thu_muc() {
        let f = fx(&["a"], vec![]).await;
        let dir = f.accounts_dir().join("a");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src").join("main.rs"), b"fn main() {}").unwrap();

        let err = f.engine.delete("a").await.unwrap_err();
        assert_eq!(err.code(), "NOT_A_PROFILE");
        assert!(
            err.to_string().contains(&dir.display().to_string()),
            "{err}"
        );
        assert!(
            f.accounts.get("a").await.unwrap().is_some(),
            "dong DB phai con nguyen de nguoi dung sua accountsDir roi thu lai"
        );
        assert!(dir.is_dir(), "thu muc KHONG duoc dung toi");
        assert!(dir.join("src").join("main.rs").is_file());
    }

    /// I5: thu muc RONG (kich ban mac dinh moi tai khoan — `fx` chi `create_dir_all`, khong tao gi
    /// ben trong) van xoa duoc binh thuong — khong hoi quy hanh vi cu.
    #[tokio::test]
    async fn delete_thu_muc_rong_van_xoa_binh_thuong() {
        let f = fx(&["a"], vec![]).await;
        assert!(f.accounts_dir().join("a").exists());
        assert!(f.engine.delete("a").await.unwrap());
        assert!(!f.accounts_dir().join("a").exists());
    }

    /// I5: thu muc co `Default/` (dau hieu profile Chromium that) van xoa duoc du khong rong.
    #[tokio::test]
    async fn delete_thu_muc_co_default_van_xoa_duoc() {
        let f = fx(&["a"], vec![]).await;
        std::fs::create_dir_all(f.accounts_dir().join("a").join("Default")).unwrap();

        assert!(f.engine.delete("a").await.unwrap());
        assert!(!f.accounts_dir().join("a").exists());
        assert!(f.accounts.get("a").await.unwrap().is_none());
    }

    // ---- import_legacy (R12) ----

    /// DB Python cu toi thieu (`tasks.db`/`pool_usage.db`, cung cach lvc-store/tests/legacy_import.rs
    /// lam) + thu muc `accounts/<name>` — `acc_new` CHUA co trong engine (se duoc copy), `acc_existing`
    /// DA co (khong duoc dung toi).
    async fn make_legacy_dir(dir: &Path) {
        let tasks_db = dir.join("tasks.db");
        let mut conn = SqliteConnectOptions::new()
            .filename(&tasks_db)
            .create_if_missing(true)
            .connect()
            .await
            .unwrap();
        conn.execute(
            "CREATE TABLE tasks (id TEXT PRIMARY KEY, prompt TEXT, created_at REAL, updated_at REAL)",
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, prompt, created_at, updated_at) VALUES ('t1', 'p', 1.0, 2.0)",
        )
        .await
        .unwrap();

        let pool_db = dir.join("pool_usage.db");
        let mut conn = SqliteConnectOptions::new()
            .filename(&pool_db)
            .create_if_missing(true)
            .connect()
            .await
            .unwrap();
        conn.execute(
            "CREATE TABLE accounts_meta (name TEXT PRIMARY KEY, scheduling INTEGER DEFAULT 1,
                note TEXT DEFAULT '', email TEXT DEFAULT '', created_at REAL DEFAULT 0,
                last_used_at REAL DEFAULT 0, login_ok INTEGER DEFAULT 1, proxy TEXT DEFAULT '',
                browser TEXT DEFAULT '')",
        )
        .await
        .unwrap();
        conn.execute("INSERT INTO accounts_meta (name) VALUES ('acc_new')")
            .await
            .unwrap();
        conn.execute("INSERT INTO accounts_meta (name) VALUES ('acc_existing')")
            .await
            .unwrap();
        conn.execute(
            "CREATE TABLE usage (account TEXT, day TEXT, mode TEXT, used INTEGER,
                PRIMARY KEY(account, day, mode))",
        )
        .await
        .unwrap();
        conn.execute("INSERT INTO usage VALUES ('acc_new', '2026-09-15', 'fast', 3)")
            .await
            .unwrap();

        let accounts_root = dir.join("accounts");
        std::fs::create_dir_all(accounts_root.join("acc_new")).unwrap();
        std::fs::write(
            accounts_root.join("acc_new").join("marker.txt"),
            b"noi dung profile cu",
        )
        .unwrap();
        std::fs::create_dir_all(accounts_root.join("acc_existing")).unwrap();
        std::fs::write(
            accounts_root
                .join("acc_existing")
                .join("khong_duoc_dung.txt"),
            b"rac",
        )
        .unwrap();
    }

    #[tokio::test]
    async fn import_legacy_bao_cao_dem_va_chi_copy_profile_chua_co() {
        let legacy_dir = tempfile::tempdir().unwrap();
        make_legacy_dir(legacy_dir.path()).await;

        let f = fx(&["acc_existing"], vec![]).await;
        std::fs::write(
            f.accounts_dir().join("acc_existing").join("hien_tai.txt"),
            b"cua engine, khong bi ghi de",
        )
        .unwrap();

        let report = f.engine.import_legacy(legacy_dir.path()).await.unwrap();
        assert_eq!(report.tasks, 1);
        assert_eq!(
            report.accounts, 1,
            "acc_existing da co dong -> INSERT OR IGNORE bo qua"
        );
        assert_eq!(report.usage, 1);
        assert_eq!(
            report.profiles_copied, 1,
            "chi acc_new (chua co thu muc) duoc copy"
        );
        assert_eq!(
            report.profiles_skipped,
            vec!["acc_existing".to_string()],
            "I3: dest da co san -> dem vao profiles_skipped"
        );

        let copied = f.accounts_dir().join("acc_new").join("marker.txt");
        assert_eq!(
            std::fs::read_to_string(copied).unwrap(),
            "noi dung profile cu"
        );
        assert!(f.accounts.get("acc_new").await.unwrap().is_some());
        // I3: khong con thu muc tam nao sot lai sau khi doi ten thanh cong.
        assert!(!f.accounts_dir().join(".import-acc_new").exists());

        // acc_existing: thu muc engine GIU NGUYEN, khong bi tron voi thu muc Python cu.
        assert!(
            f.accounts_dir()
                .join("acc_existing")
                .join("hien_tai.txt")
                .exists()
        );
        assert!(
            !f.accounts_dir()
                .join("acc_existing")
                .join("khong_duoc_dung.txt")
                .exists()
        );
    }

    #[tokio::test]
    async fn import_legacy_khong_co_thu_muc_accounts_thi_khong_loi() {
        let legacy_dir = tempfile::tempdir().unwrap();
        // Chi tasks.db/pool_usage.db thieu het -> import_legacy (P0) tu bao skipped; khong co
        // accounts/ -> copy_legacy_profiles phai tra Ok(0), khong loi.
        let f = fx(&[], vec![]).await;
        let report = f.engine.import_legacy(legacy_dir.path()).await.unwrap();
        assert_eq!(report.profiles_copied, 0);
        assert_eq!(report.skipped.len(), 2);
        assert!(report.profiles_skipped.is_empty());
    }

    /// C1 (review fix-round 1): ten thu muc profile Python KHONG khop `NAME_RE` (belt-and-braces —
    /// du la du lieu tu ban Python cu, khong tin tuong mu quang) bi bo qua + dem vao
    /// `profiles_skipped`, KHONG loi ca luot import, KHONG copy.
    #[tokio::test]
    async fn import_legacy_bo_qua_ten_thu_muc_python_khong_hop_le() {
        let legacy_dir = tempfile::tempdir().unwrap();
        let accounts_root = legacy_dir.path().join("accounts");
        std::fs::create_dir_all(accounts_root.join("ten co khoang trang")).unwrap();

        let f = fx(&[], vec![]).await;
        let report = f.engine.import_legacy(legacy_dir.path()).await.unwrap();
        assert_eq!(report.profiles_copied, 0);
        assert_eq!(
            report.profiles_skipped,
            vec!["ten co khoang trang".to_string()]
        );
        assert!(!f.accounts_dir().join("ten co khoang trang").exists());
    }

    /// I3 (review fix-round 1): thu muc tam `.import-<name>` SOT LAI tu mot lan chay truoc bi ngat
    /// giua chung phai duoc DON SACH truoc khi copy lai — kiem truc tiep helper `clear_stale_staging`
    /// (don thu muc that su copy la kho gia lap dang tin cay, controller ruling chap nhan test don
    /// vi nay thay the).
    #[tokio::test]
    async fn clear_stale_staging_don_thu_muc_tam_sot_lai() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join(".import-acc");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("rac_cu.txt"), b"rac tu lan truoc bi ngat").unwrap();

        clear_stale_staging(&staging).await.unwrap();
        assert!(!staging.exists());

        // Idempotent: thu muc khong ton tai van Ok(()).
        clear_stale_staging(&staging).await.unwrap();
    }
}
