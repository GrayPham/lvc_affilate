//! `AccountPool` — chon / xoay / khoa / danh dau han muc tai khoan (spec §5.2, Task 16).
//!
//! Port phan "lich tai khoan" cua `browser_pool.py` (`BrowserPool`): tai khoan nao duoc chay
//! `mode` luc `now`, thu tu chia deu (`_spread_key` + `pick_account`), khoa theo tung tai khoan
//! (`tokio::Mutex` giu qua `await` suot phien trinh duyet), va cac dau `daily limit` /
//! `cooldown` / `quota` / `logged out` ghi vao `accounts_meta` + `usage` qua
//! [`lvc_store::AccountRepo`].
//!
//! **Thoi gian tiem tu ngoai** (ruling Task 16): moi ham nhan `now: f64` (unix giay) — pool
//! KHONG doc dong ho; Task 17 truyen `SystemTime::now()`. Nho vay moi quy tac (Tokyo midnight,
//! `PICK_RESERVE`, cooldown) test duoc ma khong cho that.
//!
//! **Khac Python (co y, ghi trong report Task 16):**
//! - `candidates_for` gop `pick_account` (:953-972) + `candidates_for(mode, start_account)`
//!   (:940-951): dau danh sach = tai khoan "tuoi" (chua bi chon trong `PICK_RESERVE` s) co
//!   `_spread_key` nho nhat, phan con lai sap theo `_spread_key`. Chi port `ACCOUNT_PICK=rotate`
//!   (mac dinh; `random`/`order` khong port).
//! - Tai khoan co chuoi proxy hong (`parse_proxy` loi) bi BO QUA kem `warn` (Python noi o luc mo
//!   Chrome bang `ValueError` khong ma loi); prefer tai khoan do → `PreferredUnavailable`.
//! - `lease_wait` co gioi han `BUSY_WAIT_MAX` (Python `async with lock` cho vo han khi
//!   `prefer_account`); het gio → `PreferredUnavailable`.
//! - `mark_logged_out(revoked = false)` KHONG len lich auto-verify (`_schedule_auto_verify`,
//!   v0.33 — P3); chi ghi `login_ok = 0`.
//! - `mark_*` tra `Result` (loi `SQLite` noi len thay vi nuot).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use dashmap::DashMap;
use lvc_core::{Limits, Mode, ProxySpec, effective_proxy, next_limit_reset, tokyo_day};
use lvc_store::{AccountRepo, AccountRow, Db};
use tokio::sync::OwnedMutexGuard;

use crate::EngineError;

/// config.py:202 `PICK_RESERVE` — giay "giu cho" sau khi mot luong chon tai khoan (3 luong khoi
/// dong cung luc khong chon trung 1 tai khoan LRU).
pub const PICK_RESERVE_S: f64 = 20.0;
/// config.py:197 `BUSY_WAIT_MAX` — cho toi da khi tai khoan duoc chi dinh dang ban.
pub const BUSY_WAIT_MAX: Duration = Duration::from_secs(600);
/// `browser_pool.py:26` `COOLDOWN_SEC` — nghi 30 phut khi Dola bao risk control.
pub const COOLDOWN_RISK_CONTROL: Duration = Duration::from_secs(1800);
/// config.py:290 `PROFILE_BUSY_COOLDOWN` — profile Chrome bi tien trinh khac giu (dot 5 BUG-002).
pub const COOLDOWN_PROFILE_BUSY: Duration = Duration::from_secs(300);
/// config.py:150 `PREFER_ACCOUNT_FORCE` (mac dinh bat): tai khoan nguoi dung CHI DINH chi bi chan
/// khi `_hard_blocked`; trang thai mem (`rate_limited`/quota/han muc/credit) van cho mo trinh duyet
/// gui thu (2026-09-17 "khong mo trinh duyet chay thu sao biet la loi").
pub const PREFER_ACCOUNT_FORCE: bool = true;
/// `browser_pool.py:473`, `:486` `reason[:300]` — cat ly do truoc khi ghi DB (theo ky tu).
const REASON_MAX_CHARS: usize = 300;
/// `browser_pool.py:931` `last_reset = _next_limit_reset() - 86400`.
const DAY_S: f64 = 86_400.0;

/// Bo tai khoan (spec §5.2). Dung chung mot `Arc<AccountPool>` giua cac job.
pub struct AccountPool {
    repo: AccountRepo,
    /// spec §5.2: `DashMap<AccountId, Arc<tokio::Mutex<()>>>` — khoa co y giu qua `await` (lease
    /// tai nguyen, khong phai data guard).
    locks: DashMap<String, Arc<tokio::sync::Mutex<()>>>,
    /// `browser_pool.py:191` `_recent_picks`: ten -> moc `now` luc duoc chon (chi lon toi da bang
    /// so tai khoan nen khong can don).
    recent_picks: Mutex<HashMap<String, f64>>,
    limits: Limits,
    /// config.py `PROXY` — proxy chung; proxy rieng tung tai khoan thang no
    /// (`lvc_core::effective_proxy`).
    global_proxy: String,
    /// Thu muc chua profile; tai khoan = thu muc con (browser_pool.py:405-411 `accounts`).
    accounts_dir: PathBuf,
}

/// Ve thue mot tai khoan: giu khoa profile cho den khi `drop`. `proxy` la proxy hieu luc
/// (rieng > chung; `None` = khong dung proxy) de Task 17 dua vao `LaunchOpts`.
#[derive(Debug)]
#[must_use = "tha Lease la mat khoa tai khoan ngay lap tuc"]
pub struct Lease {
    pub name: String,
    pub proxy: Option<ProxySpec>,
    _guard: OwnedMutexGuard<()>,
}

/// Anh chup mot tai khoan tai `now` — phan can dung cua dict `list_accounts`
/// (browser_pool.py:490-548): dong DB + luot da dung hom nay (fast + mode dang xin).
struct Snapshot {
    row: AccountRow,
    /// `used_today` (= fast) — khoa sap xep `_spread_key` (:936-938).
    used_fast: i64,
    /// `used_today_<mode>` cua mode dang xin.
    used_mode: i64,
    /// `limit_<mode>`.
    limit_mode: i64,
}

impl Snapshot {
    /// `browser_pool.py:514` `cooling`.
    fn cooling(&self, now: f64) -> bool {
        self.row.cooldown_until > now
    }
    /// `browser_pool.py:516` `rate_limited`.
    fn rate_limited(&self, now: f64) -> bool {
        self.row.rate_limited_until > now
    }
    /// `browser_pool.py:519` `quota_blocked`.
    fn quota_blocked(&self, now: f64) -> bool {
        self.row.quota_blocked_until > now
    }
    /// browser_pool.py:905-907 `_used_and_limit`: `used >= limit`.
    fn over_limit(&self) -> bool {
        self.used_mode >= self.limit_mode
    }
    /// browser_pool.py:936-938 `_spread_key`: it dung HOM NAY (fast) nhat truoc, roi lau chua
    /// dung, roi ten.
    fn spread_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.used_fast
            .cmp(&other.used_fast)
            .then_with(|| self.row.last_used_at.total_cmp(&other.row.last_used_at))
            .then_with(|| self.row.name.cmp(&other.row.name))
    }
}

/// `reason[:300]` theo ky tu (Python cat str theo code point).
fn clip_reason(reason: &str) -> String {
    reason.chars().take(REASON_MAX_CHARS).collect()
}

impl AccountPool {
    /// `global_proxy`: chuoi proxy chung (config `PROXY`, co the rong); `accounts_dir`: thu muc
    /// profile (khong ton tai → khong co tai khoan nao, nhu Python).
    pub fn new(
        db: &Db,
        limits: Limits,
        global_proxy: impl Into<String>,
        accounts_dir: impl AsRef<Path>,
    ) -> Self {
        Self {
            repo: AccountRepo::new(db),
            locks: DashMap::new(),
            recent_picks: Mutex::new(HashMap::new()),
            limits,
            global_proxy: global_proxy.into(),
            accounts_dir: accounts_dir.as_ref().to_path_buf(),
        }
    }

    /// browser_pool.py:448-457 `next_limit_reset` — 00:00 Tokyo ke tiep (thuan, o
    /// [`lvc_core::next_limit_reset`]).
    #[must_use]
    pub fn next_limit_reset(now: f64) -> f64 {
        next_limit_reset(now)
    }

    // ===== Chon tai khoan =====

    /// Tai khoan lich duoc cho `mode` luc `now`, theo thu tu nen thu.
    ///
    /// - `prefer = None` (browser_pool.py:940-972): loc `_schedulable` (:909-914) → dau danh sach
    ///   la tai khoan "tuoi" (chua bi chon trong [`PICK_RESERVE_S`] — khong con ai tuoi thi xet
    ///   tat ca) co `_spread_key` nho nhat, ghi nhan moc chon; phan con lai sap theo `_spread_key`.
    ///   Rong = khong tai khoan nao dung duoc (Task 17 quyet dinh `AllAccountsLimited` qua
    ///   [`AccountPool::all_limited_for`]).
    /// - `prefer = Some(name)` (browser_pool.py:1108-1121): CHI tai khoan do. Khong ton tai (khong
    ///   co thu muc/dong) → `PreferredUnavailable("account 'x' does not exist")`; khong lich duoc
    ///   va `_hard_blocked` (:916-919: dang xuat / tat lich / cooling) →
    ///   `PreferredUnavailable("account 'x' not available today (<ly do> for <mode>)")`; trang thai
    ///   mem → van tra ve (`PREFER_ACCOUNT_FORCE`), co `warn`.
    ///
    /// Moi lan goi deu xoa cac moc `rate_limited`/quota/cooldown da qua trong DB
    /// (`_clear_expired_rate_limits`, :459-467, nhu `list_accounts`).
    ///
    /// # Errors
    /// `Store` khi doc/ghi DB loi; `PreferredUnavailable` nhu tren.
    pub async fn candidates_for(
        &self,
        mode: Mode,
        prefer: Option<&str>,
        now: f64,
    ) -> Result<Vec<AccountRow>, EngineError> {
        let snaps = self.snapshot_all(mode, now).await?;
        if let Some(name) = prefer {
            return self
                .preferred_only(snaps, name, mode, now)
                .map(|s| vec![s.row]);
        }
        let mut out: Vec<Snapshot> = snaps
            .into_iter()
            .filter(|s| self.schedulable(s, now))
            .filter(|s| match self.proxy_for(&s.row.proxy) {
                Ok(_) => true,
                Err(e) => {
                    tracing::warn!(account = %s.row.name, error = %e, "bo qua: chuoi proxy hong");
                    false
                }
            })
            .collect();
        if out.is_empty() {
            return Ok(Vec::new());
        }
        let head = self.pick_head(&out, now);
        let head = out.swap_remove(head);
        out.sort_by(Snapshot::spread_cmp);
        Ok(std::iter::once(head).chain(out).map(|s| s.row).collect())
    }

    /// browser_pool.py:1108-1121: nhanh `prefer_account`.
    fn preferred_only(
        &self,
        snaps: Vec<Snapshot>,
        name: &str,
        mode: Mode,
        now: f64,
    ) -> Result<Snapshot, EngineError> {
        let Some(chosen) = snaps.into_iter().find(|s| s.row.name == name) else {
            return Err(EngineError::PreferredUnavailable(format!(
                "account '{name}' does not exist"
            )));
        };
        if !self.schedulable(&chosen, now) {
            let why = self.unavailable_reason(&chosen, now);
            if Self::hard_blocked(&chosen, now) || !PREFER_ACCOUNT_FORCE {
                return Err(EngineError::PreferredUnavailable(format!(
                    "account '{name}' not available today ({why} for {})",
                    mode.as_str()
                )));
            }
            // browser_pool.py:1120: van thu vi Dola tu choi thi khong mat luot, tool hoc lai trang thai that
            tracing::warn!(
                account = name,
                why,
                "do nguoi dung chi dinh -> van thu du tool cho la khong dung duoc"
            );
        }
        if let Err(e) = self.proxy_for(&chosen.row.proxy) {
            return Err(EngineError::PreferredUnavailable(format!(
                "account '{name}': {e}"
            )));
        }
        Ok(chosen)
    }

    /// browser_pool.py:953-972 `pick_account` (`ACCOUNT_PICK=rotate`): chi so cua tai khoan "tuoi"
    /// (chua bi chon trong `PICK_RESERVE` s; khong con ai tuoi thi xet tat ca) co `_spread_key`
    /// nho nhat; ghi nhan moc chon.
    fn pick_head(&self, out: &[Snapshot], now: f64) -> usize {
        let mut picks = self
            .recent_picks
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let is_fresh = |s: &Snapshot| {
            picks
                .get(&s.row.name)
                .is_none_or(|&t| now - t >= PICK_RESERVE_S)
        };
        let best_of = |pred: &dyn Fn(&Snapshot) -> bool| {
            out.iter()
                .enumerate()
                .filter(|(_, s)| pred(s))
                .min_by(|(_, a), (_, b)| a.spread_cmp(b))
                .map(|(i, _)| i)
        };
        let idx = best_of(&is_fresh)
            .or_else(|| best_of(&|_| true))
            .expect("out khong rong (da kiem o candidates_for)");
        picks.insert(out[idx].row.name.clone(), now);
        idx
    }

    /// browser_pool.py:405-411 `accounts` + :490-548 `list_accounts`: xoa moc het han, quet thu
    /// muc profile (tu them `accounts_meta` cho thu muc moi — `_ensure_meta`), chi giu tai khoan
    /// CO thu muc, kem luot da dung hom nay (ngay Tokyo).
    async fn snapshot_all(&self, mode: Mode, now: f64) -> Result<Vec<Snapshot>, EngineError> {
        self.repo.clear_expired_limits(now).await?;
        let dir_names = self.profile_dirs().await;
        let mut rows = self.repo.list().await?;
        let mut added = false;
        for name in &dir_names {
            if !rows.iter().any(|r| &r.name == name) {
                self.repo.upsert(name).await?;
                added = true;
            }
        }
        if added {
            rows = self.repo.list().await?;
        }
        let day = tokyo_day(now);
        let limit_mode = i64::from(self.limits.daily_limit(mode));
        let mut out = Vec::with_capacity(dir_names.len());
        for row in rows {
            if !dir_names.contains(&row.name) {
                continue;
            }
            let used_fast = self.repo.used_today(&row.name, &day, Mode::Fast).await?;
            let used_mode = if mode == Mode::Fast {
                used_fast
            } else {
                self.repo.used_today(&row.name, &day, mode).await?
            };
            out.push(Snapshot {
                row,
                used_fast,
                used_mode,
                limit_mode,
            });
        }
        Ok(out)
    }

    /// browser_pool.py:405-411: ten thu muc con (khong bat dau bang `.`) cua `accounts_dir`;
    /// thu muc khong ton tai / khong doc duoc → rong.
    async fn profile_dirs(&self) -> Vec<String> {
        let mut names = Vec::new();
        let Ok(mut rd) = tokio::fs::read_dir(&self.accounts_dir).await else {
            return names;
        };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let is_dir = entry.file_type().await.is_ok_and(|t| t.is_dir());
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_dir && !name.starts_with('.') {
                names.push(name);
            }
        }
        names.sort_unstable();
        names
    }

    /// browser_pool.py:909-914 `_schedulable`.
    fn schedulable(&self, s: &Snapshot, now: f64) -> bool {
        s.row.scheduling
            && !s.cooling(now)
            && !s.rate_limited(now)
            && !s.quota_blocked(now)
            && !s.over_limit()
            && s.row.login_ok
            && !self.credit_blocks(s, now)
    }

    /// browser_pool.py:916-919 `_hard_blocked`: chac chan khong dung duoc (dang xuat / tat lich /
    /// cooldown ky thuat). Con lai la suy luan mem tu chu Dola tra lan truoc.
    fn hard_blocked(s: &Snapshot, now: f64) -> bool {
        !s.row.login_ok || !s.row.scheduling || s.cooling(now)
    }

    /// browser_pool.py:921-926 `_unavailable_reason` (dung thu tu uu tien cua Python).
    fn unavailable_reason(&self, s: &Snapshot, now: f64) -> &'static str {
        if !s.row.login_ok {
            "logged out"
        } else if s.rate_limited(now) {
            "rate limited"
        } else if s.quota_blocked(now) {
            "quota blocked"
        } else if s.cooling(now) {
            "cooling"
        } else if s.over_limit() {
            "daily limit reached"
        } else if self.credit_blocks(s, now) {
            "insufficient credits"
        } else {
            "scheduling off"
        }
    }

    /// browser_pool.py:927-933 `_credit_blocks` (dot 7 REQ-003): so du credit da doc
    /// `< video_required_points` CHI chan khi duoc doc SAU moc reset gan nhat; sang ngay → so cu
    /// het hieu luc.
    fn credit_blocks(&self, s: &Snapshot, now: f64) -> bool {
        match s.row.credit_balance {
            None => false,
            Some(bal) if bal >= i64::from(self.limits.video_required_points) => false,
            Some(_) => {
                let last_reset = next_limit_reset(now) - DAY_S;
                s.row.credit_checked_at >= last_reset
            }
        }
    }

    /// browser_pool.py:730-732 `effective_proxy_for` + `parse_proxy` (proxies.py): rieng > chung,
    /// `direct` chan proxy chung.
    fn proxy_for(&self, own: &str) -> Result<Option<ProxySpec>, lvc_core::CoreError> {
        effective_proxy(own, &self.global_proxy)
    }

    // ===== Khoa theo tai khoan (spec §5.2) =====

    fn lock_for(&self, name: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.locks
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Thue tai khoan neu profile dang ranh (`try_lock` — browser_pool.py:1132-1136: tai khoan ban
    /// thi bo qua, xoay tai khoan khac). `proxy` = chuoi proxy RIENG cua tai khoan (`row.proxy`).
    /// `None` khi dang ban; cung `None` (kem `warn`) neu chuoi proxy hong — khong xay ra voi dong
    /// tu [`AccountPool::candidates_for`] (da loc), chi la hang rao phong thu.
    pub fn try_lease(&self, name: &str, proxy: &str) -> Option<Lease> {
        let proxy = match self.proxy_for(proxy) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(account = name, error = %e, "khong thue: chuoi proxy hong");
                return None;
            }
        };
        let guard = self.lock_for(name).try_lock_owned().ok()?;
        Some(Lease {
            name: name.to_string(),
            proxy,
            _guard: guard,
        })
    }

    /// Thue tai khoan duoc CHI DINH: dang ban thi CHO (browser_pool.py:1133-1136 dot 7 REQ-004 —
    /// khong bo qua vi chi duoc dung dung tai khoan do), toi da `max` (Task 17 truyen
    /// [`BUSY_WAIT_MAX`]).
    ///
    /// # Errors
    /// `PreferredUnavailable("account 'x': busy for Ns")` khi het `max`; `PreferredUnavailable`
    /// kem thong diep `parse_proxy` khi chuoi proxy hong.
    pub async fn lease_wait(
        &self,
        name: &str,
        proxy: &str,
        max: Duration,
    ) -> Result<Lease, EngineError> {
        let proxy = self
            .proxy_for(proxy)
            .map_err(|e| EngineError::PreferredUnavailable(format!("account '{name}': {e}")))?;
        let lock = self.lock_for(name);
        match tokio::time::timeout(max, lock.lock_owned()).await {
            Ok(guard) => Ok(Lease {
                name: name.to_string(),
                proxy,
                _guard: guard,
            }),
            Err(_) => Err(EngineError::PreferredUnavailable(format!(
                "account '{name}': busy for {}s",
                max.as_secs()
            ))),
        }
    }

    // ===== Ghi nhan ket qua =====

    /// browser_pool.py:425-431 `_claim` + :1187-1189: tru 1 luot (ngay Tokyo, dung mode) va
    /// `last_used_at = now`. Goi khi tao video xong HOAC timeout sau khi da co conversation
    /// (:1230-1237 — Dola van chay, khong gui lai).
    ///
    /// # Errors
    /// `Store` khi ghi DB loi.
    pub async fn claim(&self, name: &str, mode: Mode, now: f64) -> Result<(), EngineError> {
        let day = tokyo_day(now);
        let used = self.repo.bump_usage(name, &day, mode).await?;
        self.repo.touch_last_used(name, now).await?;
        tracing::info!(account = name, mode = mode.as_str(), day, used, "claim");
        Ok(())
    }

    /// browser_pool.py:643-666 `mark_logged_out` (REQ-002): `login_ok = 0` + `logged_out_at` →
    /// bi loai khoi lich toi khi Verify / dang nhap lai. `revoked` (2026-09-17: Dola tu choi
    /// 710022002 roi tu dang xuat): `auto_verify_state = 'revoked'`, khong auto-verify.
    ///
    /// # Errors
    /// `Store` khi ghi DB loi.
    pub async fn mark_logged_out(
        &self,
        name: &str,
        reason: &str,
        revoked: bool,
        now: f64,
    ) -> Result<(), EngineError> {
        self.repo.set_login_ok(name, false, now).await?;
        tracing::warn!(
            account = name,
            revoked,
            reason = %reason.chars().take(120).collect::<String>(),
            "marked LOGGED OUT"
        );
        if revoked {
            self.repo.set_auto_verify(name, "revoked", 0).await?;
        }
        Ok(())
    }

    /// browser_pool.py:476-488 `_mark_daily_limit`: `used` cua (ngay Tokyo, mode) len it nhat
    /// `daily_limit(mode)`; `rate_limited_until` = 00:00 Tokyo ke tiep; `limit_reason` (cat 300);
    /// `last_used_at = now`.
    ///
    /// # Errors
    /// `Store` khi ghi DB loi.
    pub async fn mark_daily_limit(
        &self,
        name: &str,
        mode: Mode,
        reason: &str,
        now: f64,
    ) -> Result<(), EngineError> {
        let day = tokyo_day(now);
        self.repo
            .set_usage(name, &day, mode, self.limits.daily_limit(mode))
            .await?;
        let until = next_limit_reset(now);
        self.repo
            .set_rate_limited_until(name, until, &clip_reason(reason))
            .await?;
        self.repo.touch_last_used(name, now).await?;
        tracing::warn!(
            account = name,
            mode = mode.as_str(),
            until,
            "reached daily limit"
        );
        Ok(())
    }

    /// browser_pool.py:1225-1227 (`RiskControl` → [`COOLDOWN_RISK_CONTROL`]) / :1257-1259 (profile
    /// ban → [`COOLDOWN_PROFILE_BUSY`]): `cooldown_until = now + cooldown`.
    ///
    /// # Errors
    /// `Store` khi ghi DB loi.
    pub async fn mark_cooldown(
        &self,
        name: &str,
        cooldown: Duration,
        now: f64,
    ) -> Result<(), EngineError> {
        let until = now + cooldown.as_secs_f64();
        self.repo.set_cooldown_until(name, until).await?;
        tracing::warn!(account = name, secs = cooldown.as_secs(), "cooldown");
        Ok(())
    }

    /// browser_pool.py:469-474 `_mark_quota_blocked`: `quota_blocked_until` = 00:00 Tokyo ke
    /// tiep, `quota_reason` (cat 300), `last_used_at = now`.
    ///
    /// # Errors
    /// `Store` khi ghi DB loi.
    pub async fn mark_quota_blocked(
        &self,
        name: &str,
        reason: &str,
        now: f64,
    ) -> Result<(), EngineError> {
        let until = next_limit_reset(now);
        self.repo
            .set_quota_blocked_until(name, until, &clip_reason(reason))
            .await?;
        self.repo.touch_last_used(name, now).await?;
        tracing::warn!(account = name, until, "quota blocked");
        Ok(())
    }

    /// browser_pool.py:1013-1019 `all_accounts_limited_for`: `true` neu CO tai khoan dang bat lich
    /// va khong cooling, va TAT CA chung deu `rate_limited` hoac het han muc `mode` — Task 17 dung
    /// de chon `AllAccountsLimited` (429) thay vi loi chung khi `candidates_for` rong.
    ///
    /// # Errors
    /// `Store` khi doc DB loi.
    pub async fn all_limited_for(&self, mode: Mode, now: f64) -> Result<bool, EngineError> {
        let snaps = self.snapshot_all(mode, now).await?;
        let mut any = false;
        for s in snaps.iter().filter(|s| s.row.scheduling && !s.cooling(now)) {
            any = true;
            if !(s.rate_limited(now) || s.over_limit()) {
                return Ok(false);
            }
        }
        Ok(any)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lvc_core::{Limits, Mode};
    use lvc_store::{AccountRepo, Db};

    use super::*;
    use crate::EngineError;

    /// 2026-09-17T03:21:07Z = 12:21:07 Tokyo 17/9 -> reset ke tiep `1_789_657_200` (00:00 Tokyo 18/9).
    const NOW: f64 = 1_789_615_267.0;
    const RESET: f64 = 1_789_657_200.0;
    const DAY: &str = "2026-09-17";

    struct Fx {
        dir: tempfile::TempDir,
        pool: AccountPool,
        repo: AccountRepo,
    }

    async fn fx_with(names: &[&str], limits: Limits, global_proxy: &str) -> Fx {
        let db = Db::open_memory().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let repo = AccountRepo::new(&db);
        for n in names {
            std::fs::create_dir(dir.path().join(n)).unwrap();
            repo.upsert(n).await.unwrap();
        }
        let pool = AccountPool::new(&db, limits, global_proxy, dir.path());
        Fx { dir, pool, repo }
    }

    async fn fx(names: &[&str]) -> Fx {
        fx_with(names, Limits::default(), "").await
    }

    fn names(rows: &[lvc_store::AccountRow]) -> Vec<&str> {
        rows.iter().map(|r| r.name.as_str()).collect()
    }

    fn unavailable(e: &EngineError) -> &str {
        match e {
            EngineError::PreferredUnavailable(s) => s.as_str(),
            other => panic!("phai la PreferredUnavailable, nhan: {other:?}"),
        }
    }

    // ---- candidates_for: loc theo _schedulable (browser_pool.py:909-914) ----

    #[tokio::test]
    async fn candidates_loai_dang_cooldown() {
        let f = fx(&["a", "b"]).await;
        f.repo.set_cooldown_until("a", NOW + 100.0).await.unwrap();
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert_eq!(names(&rows), ["b"]);
        // dung moc cooldown_until (<= now) -> het cooling, ca hai deu vao
        let rows = f
            .pool
            .candidates_for(Mode::Fast, None, NOW + 100.0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        // _clear_expired_rate_limits (browser_pool.py:459-467) da xoa moc trong DB
        let row = f.repo.get("a").await.unwrap().unwrap();
        assert!(row.cooldown_until.abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn candidates_loai_het_han_muc_mode() {
        let limits = Limits {
            daily_limit_pro: 1,
            ..Limits::default()
        };
        let f = fx_with(&["a", "b"], limits, "").await;
        f.repo.set_usage("a", DAY, Mode::Pro, 1).await.unwrap();
        let rows = f.pool.candidates_for(Mode::Pro, None, NOW).await.unwrap();
        assert_eq!(names(&rows), ["b"], "a het han muc pro");
        // han muc tinh RIENG tung mode: fast van con
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert_eq!(rows.len(), 2);
        // sang ngay Tokyo moi -> khoa day khac -> a lai duoc
        let rows = f.pool.candidates_for(Mode::Pro, None, RESET).await.unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[tokio::test]
    async fn candidates_loai_rate_limited_quota_blocked_logged_out_tat_lich() {
        let f = fx(&["rl", "qb", "lo", "off", "ok"]).await;
        f.repo
            .set_rate_limited_until("rl", NOW + 1.0, "r")
            .await
            .unwrap();
        f.repo
            .set_quota_blocked_until("qb", NOW + 1.0, "q")
            .await
            .unwrap();
        f.repo.set_login_ok("lo", false, NOW).await.unwrap();
        f.repo.set_scheduling("off", false).await.unwrap();
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert_eq!(names(&rows), ["ok"]);
    }

    #[tokio::test]
    async fn candidates_chan_credit_thap_chi_khi_doc_sau_moc_reset() {
        // browser_pool.py:927-933 _credit_blocks
        let f = fx(&["thap_moi", "thap_cu", "du", "chua_doc"]).await;
        let last_reset = RESET - 86_400.0;
        f.repo
            .set_credit_balance("thap_moi", Some(1), last_reset)
            .await
            .unwrap(); // >= last_reset -> chan
        f.repo
            .set_credit_balance("thap_cu", Some(0), last_reset - 1.0)
            .await
            .unwrap(); // truoc moc reset -> so cu het hieu luc
        f.repo.set_credit_balance("du", Some(2), NOW).await.unwrap(); // >= 2 -> khong chan
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        let mut got = names(&rows);
        got.sort_unstable();
        assert_eq!(got, ["chua_doc", "du", "thap_cu"]);
    }

    #[tokio::test]
    async fn candidates_bo_thu_muc_khong_ton_tai_va_tu_them_meta() {
        let f = fx(&["co_dir"]).await;
        // co dong DB nhung khong co thu muc profile -> khong lich (browser_pool.py:405-411)
        f.repo.upsert("khong_dir").await.unwrap();
        // co thu muc nhung chua co dong DB -> _ensure_meta tu them, mac dinh lich duoc
        std::fs::create_dir(f.dir.path().join("moi")).unwrap();
        // thu muc an (bat dau bang '.') bi bo qua
        std::fs::create_dir(f.dir.path().join(".an")).unwrap();
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        let mut got = names(&rows);
        got.sort_unstable();
        assert_eq!(got, ["co_dir", "moi"]);
        assert!(f.repo.get("moi").await.unwrap().is_some());
        assert!(f.repo.get(".an").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn candidates_thu_muc_accounts_khong_ton_tai_tra_rong() {
        let db = Db::open_memory().await.unwrap();
        AccountRepo::new(&db).upsert("a").await.unwrap();
        let pool = AccountPool::new(
            &db,
            Limits::default(),
            "",
            std::path::Path::new("Z:/khong/ton/tai"),
        );
        let rows = pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn candidates_bo_proxy_hong() {
        let f = fx(&["hong", "tot"]).await;
        f.repo.set_proxy("hong", "abc").await.unwrap();
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert_eq!(names(&rows), ["tot"]);
        // prefer tai khoan proxy hong -> PreferredUnavailable mang thong diep parse_proxy
        let e = f
            .pool
            .candidates_for(Mode::Fast, Some("hong"), NOW)
            .await
            .unwrap_err();
        let msg = unavailable(&e);
        assert!(msg.starts_with("account 'hong': "), "{msg}");
        assert!(msg.contains("proxy must be"), "{msg}");
    }

    // ---- candidates_for: sap xep (_spread_key 936-938, candidates_for 940-951, pick_account 953-972) ----

    #[tokio::test]
    async fn candidates_sap_theo_spread_key() {
        let f = fx(&["a", "b", "c", "d"]).await;
        // a: dung fast 2 lan hom nay; b, c, d: 0 lan; last_used b=50 c=10 d=10 -> (c, d theo ten)
        f.repo.set_usage("a", DAY, Mode::Fast, 2).await.unwrap();
        f.repo.touch_last_used("b", 50.0).await.unwrap();
        f.repo.touch_last_used("c", 10.0).await.unwrap();
        f.repo.touch_last_used("d", 10.0).await.unwrap();
        // used_today cua spread key la FAST ke ca khi xin mode khac (browser_pool.py:936-938)
        let rows = f.pool.candidates_for(Mode::Pro, None, NOW).await.unwrap();
        assert_eq!(names(&rows), ["c", "d", "b", "a"]);
    }

    #[tokio::test]
    async fn pick_reserve_20s() {
        let f = fx(&["a", "b", "c"]).await;
        // 3 luong goi lien tiep: moi luong nhan dau danh sach khac nhau (PICK_RESERVE=20 s)
        let r1 = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert_eq!(names(&r1), ["a", "b", "c"]);
        let r2 = f
            .pool
            .candidates_for(Mode::Fast, None, NOW + 1.0)
            .await
            .unwrap();
        // dau = b (a vua duoc chon); phan con lai van co a (Python: candidates_for(start) = head + rest)
        assert_eq!(names(&r2), ["b", "a", "c"]);
        let r3 = f
            .pool
            .candidates_for(Mode::Fast, None, NOW + 2.0)
            .await
            .unwrap();
        assert_eq!(names(&r3), ["c", "a", "b"]);
        // het tai khoan "tuoi" -> lay tren toan bo (Python: `fresh = [...] or out`)
        let r4 = f
            .pool
            .candidates_for(Mode::Fast, None, NOW + 3.0)
            .await
            .unwrap();
        assert_eq!(names(&r4), ["a", "b", "c"]);
        // qua 20 s ke tu lan pick cua b (NOW+1) nhung a vua pick lai o NOW+3 -> b tuoi truoc
        let r5 = f
            .pool
            .candidates_for(Mode::Fast, None, NOW + 21.5)
            .await
            .unwrap();
        assert_eq!(names(&r5), ["b", "a", "c"]);
    }

    #[tokio::test]
    async fn pick_reserve_chi_con_mot_tai_khoan_van_tra_no() {
        let f = fx(&["a"]).await;
        for i in 0..3 {
            let rows = f
                .pool
                .candidates_for(Mode::Fast, None, NOW + f64::from(i))
                .await
                .unwrap();
            assert_eq!(names(&rows), ["a"]);
        }
    }

    // ---- prefer (browser_pool.py:1108-1121) ----

    #[tokio::test]
    async fn prefer_thieu_bao_preferred_unavailable() {
        let f = fx(&["a"]).await;
        let e = f
            .pool
            .candidates_for(Mode::Fast, Some("ghost"), NOW)
            .await
            .unwrap_err();
        assert_eq!(unavailable(&e), "account 'ghost' does not exist");
        assert_eq!(
            e.to_string(),
            "ACCOUNT_UNAVAILABLE: account 'ghost' does not exist"
        );
        assert_eq!(
            e.failure_code(),
            Some(lvc_core::FailureCode::AccountUnavailable)
        );
    }

    #[tokio::test]
    async fn prefer_bi_chan_cung_bao_ly_do() {
        // _hard_blocked (916-919): logged out / scheduling off / cooling -> chan ngay
        let f = fx(&["lo", "off", "cool"]).await;
        f.repo.set_login_ok("lo", false, NOW).await.unwrap();
        f.repo.set_scheduling("off", false).await.unwrap();
        f.repo.set_cooldown_until("cool", NOW + 5.0).await.unwrap();
        for (name, why) in [
            ("lo", "logged out"),
            ("off", "scheduling off"),
            ("cool", "cooling"),
        ] {
            let e = f
                .pool
                .candidates_for(Mode::Pro, Some(name), NOW)
                .await
                .unwrap_err();
            assert_eq!(
                unavailable(&e),
                format!("account '{name}' not available today ({why} for pro)")
            );
        }
    }

    #[tokio::test]
    async fn prefer_force_bo_qua_trang_thai_mem() {
        // PREFER_ACCOUNT_FORCE=1 (config.py:150): rate_limited / quota_blocked / het han muc /
        // credit thap la SUY LUAN mem -> van tra ve tai khoan do de mo trinh duyet gui thu.
        let limits = Limits {
            daily_limit_pro: 1,
            ..Limits::default()
        };
        let f = fx_with(&["rl", "qb", "lim", "cr"], limits, "").await;
        f.repo
            .set_rate_limited_until("rl", NOW + 1.0, "r")
            .await
            .unwrap();
        f.repo
            .set_quota_blocked_until("qb", NOW + 1.0, "q")
            .await
            .unwrap();
        f.repo.set_usage("lim", DAY, Mode::Pro, 1).await.unwrap();
        f.repo.set_credit_balance("cr", Some(0), NOW).await.unwrap();
        for name in ["rl", "qb", "lim", "cr"] {
            let rows = f
                .pool
                .candidates_for(Mode::Pro, Some(name), NOW)
                .await
                .unwrap();
            assert_eq!(names(&rows), [name], "{name}");
        }
        // khong prefer -> ca 4 deu bi loai
        let rows = f.pool.candidates_for(Mode::Pro, None, NOW).await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn prefer_khong_dong_vao_recent_picks() {
        let f = fx(&["a", "b"]).await;
        let rows = f
            .pool
            .candidates_for(Mode::Fast, Some("a"), NOW)
            .await
            .unwrap();
        assert_eq!(names(&rows), ["a"]);
        // prefer khong phai pick_account -> a van "tuoi" cho luong tu dong ke tiep
        let rows = f
            .pool
            .candidates_for(Mode::Fast, None, NOW + 1.0)
            .await
            .unwrap();
        assert_eq!(names(&rows), ["a", "b"]);
    }

    // ---- lease (spec §5.2, browser_pool.py:1132-1138) ----

    #[tokio::test]
    async fn try_lease_ban_tra_none() {
        let f = fx(&["a", "b"]).await;
        let l1 = f.pool.try_lease("a", "").expect("a ranh");
        assert_eq!(l1.name, "a");
        assert!(l1.proxy.is_none());
        assert!(f.pool.try_lease("a", "").is_none(), "a dang ban");
        assert!(f.pool.try_lease("b", "").is_some(), "b khong lien quan");
        drop(l1);
        assert!(f.pool.try_lease("a", "").is_some(), "tha la lay lai duoc");
    }

    #[tokio::test]
    async fn lease_wait_cho_den_khi_tha() {
        let f = fx(&["a"]).await;
        let pool = std::sync::Arc::new(f.pool);
        let held = pool.try_lease("a", "").unwrap();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            drop(held);
        });
        let t0 = std::time::Instant::now();
        let lease = pool
            .lease_wait("a", "", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(lease.name, "a");
        assert!(
            t0.elapsed() >= Duration::from_millis(40),
            "phai thuc su cho"
        );
        assert!(
            pool.try_lease("a", "").is_none(),
            "lease_wait dang giu khoa"
        );
    }

    #[tokio::test]
    async fn lease_wait_het_gio_bao_preferred_unavailable() {
        let f = fx(&["a"]).await;
        let _held = f.pool.try_lease("a", "").unwrap();
        let e = f
            .pool
            .lease_wait("a", "", Duration::from_millis(30))
            .await
            .unwrap_err();
        assert_eq!(unavailable(&e), "account 'a': busy for 0s");
    }

    #[tokio::test]
    async fn lease_mang_proxy_hieu_luc() {
        // lvc_core::effective_proxy: rieng > chung; "direct" chan proxy chung
        let f = fx_with(
            &["chung", "rieng", "direct"],
            Limits::default(),
            "1.2.3.4:8080",
        )
        .await;
        f.repo.set_proxy("rieng", "9.9.9.9:9090").await.unwrap();
        f.repo.set_proxy("direct", "direct").await.unwrap();
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert_eq!(rows.len(), 3);
        for r in &rows {
            let lease = f.pool.try_lease(&r.name, &r.proxy).unwrap();
            let server = lease.proxy.as_ref().map(lvc_core::ProxySpec::server);
            match r.name.as_str() {
                "chung" => assert_eq!(server.as_deref(), Some("http://1.2.3.4:8080")),
                "rieng" => assert_eq!(server.as_deref(), Some("http://9.9.9.9:9090")),
                "direct" => assert_eq!(server, None),
                other => panic!("{other}"),
            }
        }
        // proxy hong -> khong lease (da bi candidates_for loc; day la hang rao phong thu)
        assert!(f.pool.try_lease("chung", "abc").is_none());
        let e = f
            .pool
            .lease_wait("chung", "abc", Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(unavailable(&e).contains("proxy must be"));
    }

    // ---- claim / mark_* (browser_pool.py:425-431, 469-488, 643-666, 1186-1189) ----

    #[tokio::test]
    async fn claim_tang_usage_va_last_used() {
        let f = fx(&["a"]).await;
        f.pool.claim("a", Mode::Pro, NOW).await.unwrap();
        f.pool.claim("a", Mode::Pro, NOW + 1.0).await.unwrap();
        assert_eq!(f.repo.used_today("a", DAY, Mode::Pro).await.unwrap(), 2);
        assert_eq!(f.repo.used_today("a", DAY, Mode::Fast).await.unwrap(), 0);
        let row = f.repo.get("a").await.unwrap().unwrap();
        assert!((row.last_used_at - (NOW + 1.0)).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn mark_daily_limit_dat_used_va_reset_tokyo() {
        let f = fx(&["a", "b"]).await;
        f.pool
            .mark_daily_limit("a", Mode::Pro, "AccountLimited: het luot", NOW)
            .await
            .unwrap();
        // used = daily_limit(pro) = 10 (browser_pool.py:480-483)
        assert_eq!(f.repo.used_today("a", DAY, Mode::Pro).await.unwrap(), 10);
        let row = f.repo.get("a").await.unwrap().unwrap();
        assert!((row.rate_limited_until - RESET).abs() < f64::EPSILON);
        assert_eq!(row.limit_reason, "AccountLimited: het luot");
        assert!((row.last_used_at - NOW).abs() < f64::EPSILON);
        // bi loai khoi moi mode (rate_limited la co chung) cho den moc reset
        let rows = f
            .pool
            .candidates_for(Mode::Fast, None, NOW + 1.0)
            .await
            .unwrap();
        assert_eq!(names(&rows), ["b"]);
        // dung moc reset: het rate_limited + sang ngay moi -> usage ngay moi = 0 -> a tro lai
        let rows = f.pool.candidates_for(Mode::Pro, None, RESET).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            f.repo
                .used_today("a", "2026-09-18", Mode::Pro)
                .await
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn mark_daily_limit_khong_ha_used_da_vuot() {
        let limits = Limits {
            daily_limit_pro: 3,
            ..Limits::default()
        };
        let f = fx_with(&["a"], limits, "").await;
        f.repo.set_usage("a", DAY, Mode::Pro, 5).await.unwrap();
        f.pool
            .mark_daily_limit("a", Mode::Pro, "x", NOW)
            .await
            .unwrap();
        assert_eq!(f.repo.used_today("a", DAY, Mode::Pro).await.unwrap(), 5);
    }

    #[tokio::test]
    async fn mark_logged_out_login_ok_0() {
        let f = fx(&["a", "b"]).await;
        f.pool
            .mark_logged_out("a", "session probe: logged_out", false, NOW)
            .await
            .unwrap();
        let row = f.repo.get("a").await.unwrap().unwrap();
        assert!(!row.login_ok);
        assert!((row.logged_out_at - NOW).abs() < f64::EPSILON);
        assert!((row.login_checked_at - NOW).abs() < f64::EPSILON);
        assert_eq!(row.auto_verify_state, "");
        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert_eq!(names(&rows), ["b"]);
        // revoked (browser_pool.py:663-665): auto_verify_state='revoked', tries=0
        f.pool
            .mark_logged_out("b", "710022002", true, NOW + 1.0)
            .await
            .unwrap();
        let row = f.repo.get("b").await.unwrap().unwrap();
        assert!(!row.login_ok);
        assert_eq!(row.auto_verify_state, "revoked");
        assert_eq!(row.auto_verify_tries, 0);
    }

    #[tokio::test]
    async fn mark_cooldown_va_quota_blocked() {
        let f = fx(&["a", "b"]).await;
        f.pool
            .mark_cooldown("a", COOLDOWN_RISK_CONTROL, NOW)
            .await
            .unwrap();
        let row = f.repo.get("a").await.unwrap().unwrap();
        assert!((row.cooldown_until - (NOW + 1800.0)).abs() < f64::EPSILON);
        f.pool
            .mark_cooldown("a", COOLDOWN_PROFILE_BUSY, NOW)
            .await
            .unwrap();
        let row = f.repo.get("a").await.unwrap().unwrap();
        assert!((row.cooldown_until - (NOW + 300.0)).abs() < f64::EPSILON);

        f.pool
            .mark_quota_blocked("b", "Insufficient credits", NOW)
            .await
            .unwrap();
        let row = f.repo.get("b").await.unwrap().unwrap();
        assert!((row.quota_blocked_until - RESET).abs() < f64::EPSILON);
        assert_eq!(row.quota_reason, "Insufficient credits");
        assert!((row.last_used_at - NOW).abs() < f64::EPSILON);

        let rows = f.pool.candidates_for(Mode::Fast, None, NOW).await.unwrap();
        assert!(rows.is_empty());
        let rows = f
            .pool
            .candidates_for(Mode::Fast, None, RESET)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2, "cooldown 300 s va quota deu het o moc reset");
    }

    #[tokio::test]
    async fn ly_do_cat_300_ky_tu() {
        // browser_pool.py:473, 486 `reason[:300]` — theo ky tu (code point), khong theo byte
        let f = fx(&["a", "b"]).await;
        let long: String = "đ".repeat(400);
        f.pool
            .mark_daily_limit("a", Mode::Fast, &long, NOW)
            .await
            .unwrap();
        f.pool.mark_quota_blocked("b", &long, NOW).await.unwrap();
        assert_eq!(
            f.repo
                .get("a")
                .await
                .unwrap()
                .unwrap()
                .limit_reason
                .chars()
                .count(),
            300
        );
        assert_eq!(
            f.repo
                .get("b")
                .await
                .unwrap()
                .unwrap()
                .quota_reason
                .chars()
                .count(),
            300
        );
    }

    // ---- all_limited_for (browser_pool.py:1013-1019) ----

    #[tokio::test]
    async fn all_limited_for_theo_mode() {
        let limits = Limits {
            daily_limit_pro: 1,
            ..Limits::default()
        };
        let f = fx_with(&["rl", "lim", "cool", "off"], limits, "").await;
        // khong co tai khoan nao dang bat + khong cooling -> false (bool(candidates) rong)
        let empty = fx(&[]).await;
        assert!(!empty.pool.all_limited_for(Mode::Fast, NOW).await.unwrap());

        f.repo
            .set_rate_limited_until("rl", NOW + 1.0, "r")
            .await
            .unwrap();
        f.repo.set_usage("lim", DAY, Mode::Pro, 1).await.unwrap();
        f.repo.set_cooldown_until("cool", NOW + 1.0).await.unwrap(); // bi bo qua khoi mau so
        f.repo.set_scheduling("off", false).await.unwrap(); // bi bo qua khoi mau so
        assert!(f.pool.all_limited_for(Mode::Pro, NOW).await.unwrap());
        // fast: lim con luot fast -> khong phai "tat ca het"
        assert!(!f.pool.all_limited_for(Mode::Fast, NOW).await.unwrap());
        // het cooling -> cool con luot -> false
        assert!(!f.pool.all_limited_for(Mode::Pro, NOW + 1.0).await.unwrap());
    }

    /// Task 17 dung chung `Arc<AccountPool>` giua cac task tokio va giu `Lease` qua `await`.
    #[test]
    fn pool_va_lease_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AccountPool>();
        assert_send_sync::<Lease>();
    }

    #[tokio::test]
    async fn next_limit_reset_tokyo() {
        assert!((AccountPool::next_limit_reset(NOW) - RESET).abs() < f64::EPSILON);
        assert!((AccountPool::next_limit_reset(RESET) - (RESET + 86_400.0)).abs() < f64::EPSILON);
    }
}
