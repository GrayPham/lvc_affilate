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

use std::path::Path;
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
/// buoc copy thu muc `accounts/<name>`).
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
                browser: row.browser,
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
    /// - [`AccountError::InvalidName`]: `name` rong sau `trim`.
    /// - [`AccountError::InvalidProxy`]: `proxy` sai dinh dang ([`parse_proxy`]) HOAC (hiem) proxy
    ///   chung dang cau hinh cung sai dinh dang khi `proxy` rong ([`effective_proxy`]).
    /// - [`AccountError::CookieInvalid`]: khong tim thay cookie `dola.com` nao / thieu `sessionid`
    ///   hop le ([`lvc_core::cookie::validate_dola_cookies`]).
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
        let name = name.trim();
        if name.is_empty() {
            return Err(AccountError::InvalidName);
        }
        let proxy = proxy.trim();
        // Kiem tra THUAN truoc (khong dung dia/trinh duyet) — that bai o day khong tao rac gi ca.
        parse_proxy(proxy).map_err(|e| AccountError::InvalidProxy(e.to_string()))?;
        let cookies = lvc_core::cookie::parse_netscape(cookie_text);
        let dola_cookies = lvc_core::cookie::validate_dola_cookies(&cookies)
            .map_err(|e| AccountError::CookieInvalid(e.to_string()))?;
        let sessionid_expires_at = lvc_core::cookie::sessionid_expires(&dola_cookies);

        let dir = self.cfg.accounts_dir.join(name);
        if dir.exists() {
            return Err(AccountError::AccountExists(name.to_string()));
        }
        tokio::fs::create_dir_all(&dir).await?;

        match self
            .import_cookies_on_dir(name, &dir, &dola_cookies, proxy)
            .await
        {
            Ok(()) => Ok(ImportCookiesReport {
                name: name.to_string(),
                sessionid_expires_at,
            }),
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
    /// # Errors
    /// - [`AccountError::AccountNotFound`]: khong co dong `accounts_meta` cho `name`.
    /// - [`AccountError::ProfileBusy`]: tai khoan dang co tac vu khac giu (lease/OS-level Chrome
    ///   lock — ca hai deu ra ma `PROFILE_BUSY`, xem [`AccountError::code`]).
    /// - [`AccountError::Browser`]: launch/goto/probe loi CDP.
    /// - [`AccountError::Store`]: ghi `login_ok` loi.
    pub async fn verify(&self, name: &str) -> Result<VerifyReport, AccountError> {
        let name = name.trim();
        let row = self
            .pool()
            .account_row(name)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound(name.to_string()))?;
        let Some(lease) = self.pool().try_lease(name, &row.proxy) else {
            return Err(AccountError::ProfileBusy(format!(
                "tai khoan '{name}' dang co tac vu khac dang chay"
            )));
        };

        let browser = BrowserKind::parse(&row.browser).unwrap_or(BrowserKind::Chromium);
        let opts = LaunchOpts {
            profile_dir: self.cfg.accounts_dir.join(name),
            browser,
            executable_override: self.cfg.executable_override.clone(),
            headless: true,
            proxy: lease.proxy.clone(),
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
    /// bat/tat lich). Khong loi khi `name` khong ton tai (UPDATE khong dong nao — cung khong lam
    /// gi, giong `AccountRepo::set_scheduling`).
    ///
    /// # Errors
    /// `Store`/`Engine` khi ghi DB hoac dem lai so tai khoan loi.
    pub async fn set_scheduling(&self, name: &str, on: bool) -> Result<(), AccountError> {
        self.pool().repo().set_scheduling(name, on).await?;
        self.refresh_account_count().await?;
        Ok(())
    }

    /// browser_pool.py:699-705 `set_proxy`: `text.strip()`; sai dinh dang -> loi ([`parse_proxy`],
    /// thong diep khop Python); `""` = dung proxy chung; `"direct"` = khong dung proxy nao. Luu
    /// CHUOI DA TRIM (khong phai gia tri da chuan hoa/mask — `list_with_status` moi mask luc doc).
    ///
    /// # Errors
    /// [`AccountError::InvalidProxy`] khi sai dinh dang; `Store` khi ghi DB loi.
    pub async fn set_proxy(&self, name: &str, proxy: &str) -> Result<(), AccountError> {
        let trimmed = proxy.trim();
        parse_proxy(trimmed).map_err(|e| AccountError::InvalidProxy(e.to_string()))?;
        self.pool().repo().set_proxy(name, trimmed).await?;
        Ok(())
    }

    /// Xoa dong `accounts_meta` + thu muc profile. Dang co lease (tac vu khac dang chay) ->
    /// [`AccountError::AccountBusy`] NGAY (khong cho); xoa thu muc that bai (Windows: file dang mo
    /// boi tien trinh Chrome con song, vd phien am trong `SessionCache`) ->
    /// [`AccountError::ProfileBusy`] (dong `accounts_meta` co the DA bi xoa luc nay — nguoi dung
    /// thu lai se thay tai khoan bien mat khoi danh sach nhung thu muc van con, xem "Concerns" bao
    /// cao Task 10). `name` khong ton tai ca dong lan thu muc -> `Ok(false)` (idempotent, giong
    /// `workflow_delete`), KHONG loi.
    ///
    /// # Errors
    /// [`AccountError::AccountBusy`]/[`AccountError::ProfileBusy`] nhu tren; `Store`/`Engine` khi
    /// xoa dong hoac dem lai so tai khoan loi.
    pub async fn delete(&self, name: &str) -> Result<bool, AccountError> {
        let name = name.trim();
        let Some(lease) = self.pool().try_lease(name, "") else {
            return Err(AccountError::AccountBusy(name.to_string()));
        };
        let row_existed = self.pool().repo().delete(name).await?;
        let dir = self.cfg.accounts_dir.join(name);
        let dir_existed = dir.exists();
        if dir_existed {
            if let Err(e) = tokio::fs::remove_dir_all(&dir).await {
                return Err(AccountError::ProfileBusy(format!(
                    "khong xoa duoc thu muc profile '{name}': {e}"
                )));
            }
        }
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
    /// `Store` khi doc/ghi DB loi; [`AccountError::Io`] khi doc/copy thu muc `accounts/` that bai
    /// (KHONG tinh thieu thu muc `accounts/` — coi nhu khong co profile nao de copy, `Ok(0)`).
    pub async fn import_legacy(
        &self,
        python_dir: &Path,
    ) -> Result<LegacyImportReport, AccountError> {
        let report = lvc_store::legacy::import_legacy(&self.db, python_dir).await?;
        let profiles_copied = self.copy_legacy_profiles(python_dir).await?;
        self.refresh_account_count().await?;
        Ok(LegacyImportReport {
            tasks: report.tasks,
            accounts: report.accounts,
            usage: report.usage,
            settings: report.settings,
            skipped: report.skipped,
            profiles_copied,
        })
    }

    async fn copy_legacy_profiles(&self, python_dir: &Path) -> Result<usize, AccountError> {
        let src_accounts = python_dir.join("accounts");
        // Khong co thu muc accounts/ trong ban Python cu -> khong co gi de copy (khong phai loi).
        let Ok(mut rd) = tokio::fs::read_dir(&src_accounts).await else {
            return Ok(0);
        };
        let mut copied = 0usize;
        while let Some(entry) = rd.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            let dest = self.cfg.accounts_dir.join(&name);
            if dest.exists() {
                // Chi copy khi CHUA co san — khong bao gio ghi de profile hien tai.
                continue;
            }
            copy_dir_all(&entry.path(), &dest).await?;
            copied += 1;
        }
        Ok(copied)
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

        let copied = f.accounts_dir().join("acc_new").join("marker.txt");
        assert_eq!(
            std::fs::read_to_string(copied).unwrap(),
            "noi dung profile cu"
        );
        assert!(f.accounts.get("acc_new").await.unwrap().is_some());

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
    }
}
