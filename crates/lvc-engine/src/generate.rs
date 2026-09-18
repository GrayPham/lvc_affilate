//! `Engine::generate_video` — dieu phoi MOT video (Task 17): slot `DynamicLimiter` → chon/xoay tai
//! khoan (`AccountPool`) → phien trinh duyet am (`SessionCache`) hoac mo moi → `submit` → `poll_and
//! _download` → tru luot → ghi `tasks` (`status` / `account` / `conversation_id` / `video_url` /
//! `failure_code`). Port `browser_pool.py:1090-1287` (`BrowserPool.generate_video`) + `server.py:
//! 596-719` (`_run_task`, phan khong thuoc HTTP/API-key/batch) + `video_worker_ui.py:2507-2511`
//! (dong hay giu phien am theo loai loi).
//!
//! | Rust | Python | Ghi chu |
//! |---|---|---|
//! | [`Engine::generate_video`] | `_run_task` `:596-719` | ghi `Failed` + `failure_code` + `error[:500]` cho MOI loi (`:674-712`) |
//! | `run` — `limiter.acquire()` | `limiter.slot(queue_order)` `:1105` | khong FIFO tuyet doi (ruling T15) |
//! | `run` — `pick_account` mot lan | `:632-634` | chi khi khong `prefer` |
//! | `run` — `mode_fallback_chain` + `fit_duration_to_mode` | `:680-698` (v0.55z) | chi xoay mode khi `AllAccountsLimited`; `set_mode_duration` `:684-685` |
//! | `run_mode` — `candidates_for` / `try_lease` / `lease_wait` | `:1108-1136` | prefer → doi khoa (REQ-004), khac → bo qua tai khoan ban |
//! | `run_mode` — kiem lai sau khi co khoa | `:1138-1145` | "State changed while waiting" |
//! | `run_mode` — doi 0.5 s khi moi tai khoan ban | `:1262-1270` (ADR-016) | toi da `busy_wait_max` |
//! | `final_error` | `:1279-1294` | prefer → `PreferredUnavailable`; `ProfileBusy` → `NoFreeProfile`; `LoggedOut` → giu; `all_limited_for` → `AllAccountsLimited` |
//! | `attempt` — `take_or_launch` | `warm_browser_manager.acquire` + `proxy_precheck` `:1146-1149` | `ExecutableNotFound` → loi ngay; `ProfileBusy` → cooldown 300 `:1245-1261`; proxy chet → `ProxyDown` + xoay |
//! | `attempt` — `set_status(Processing)` + `set_account` | `:610`, `on_account` `:625-627` | |
//! | `attempt` — `submit` → `set_conversation` | `on_conversation_id` `:612-619` | |
//! | `attempt` — `poll_and_download` Ok → `claim` | `_claim` `:1186-1189` | roi `set_result` + `set_status(Completed)` (`_finish_task` phan luu; hau ky logo P3) |
//! | `after_dola_error` | `except` `:1192-1261` + `:2507-2511` | bang trong doc ham |
//!
//! **Khong log prompt** (chi so ky tu). **Khong port**: `revoked_pause_reason` (cau dao `REVOKED_STREAK`,
//! P3), `CREDIT_QUOTE_RETRIES` (`:1163-1180` gui lai khi Dola bao gia sai), `on_balance`, resume chat
//! cu (`conversation_id`), batch context, requeue khi `AllAccountsLoggedOut` (`:680-689` — P3),
//! `download_reference_images` (`:628` — `Scene.reference_images` duoc coi la duong dan cuc bo).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lvc_browser::dola::{ConversationId, PollHooks, PollOpts, SubmitRequest, VideoArtifact};
use lvc_browser::{BrowserDriver, BrowserError, DolaError, LaunchOpts};
use lvc_core::{
    BrowserKind, FailureCode, Limits, Mode, Scene, TaskStatus, fit_duration_to_mode,
    mode_fallback_chain, video_timeout,
};
use lvc_store::{AccountRepo, AccountRow, Db, TaskRepo};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::EngineError;
use crate::limiter::{DynamicLimiter, LimiterHandle};
use crate::pool::{AccountPool, COOLDOWN_PROFILE_BUSY, COOLDOWN_RISK_CONTROL, Lease};
use crate::session_cache::SessionCache;

/// config.py:115 `WARM_SESSION_IDLE_TIMEOUT` — 15 phut khong dung moi tat Chrome.
pub const DEFAULT_WARM_IDLE: Duration = Duration::from_secs(900);
/// config.py:197 `BUSY_WAIT_MAX`.
pub const DEFAULT_BUSY_WAIT_MAX: Duration = crate::pool::BUSY_WAIT_MAX;
/// `browser_pool.py:1265` `asyncio.sleep(0.5)` — nhip poll lai khi moi tai khoan deu ban.
pub const BUSY_POLL: Duration = Duration::from_millis(500);
/// server.py:675 v.v. `str(e)[:500]` — cat thong diep loi truoc khi ghi cot `error` (theo ky tu).
pub const ERROR_MAX_CHARS: usize = 500;
/// `browser_pool.py:1281` `str(last_err)[:200]`.
const PREFER_REASON_MAX_CHARS: usize = 200;

/// Unix giay hien tai (`time.time()`), noi DUY NHAT engine doc dong ho (ruling Task 17) — moi ham
/// cua [`AccountPool`] nhan gia tri nay.
#[must_use]
pub fn now_unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Cau hinh engine (brief Task 17 + ruling #6).
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Thu muc chua profile; tai khoan = thu muc con (`accounts/<ten>/`).
    pub accounts_dir: PathBuf,
    /// Thu muc luu video (`_task_out_dir` `server.py:422-440`: video le → downloads cua tool; du an
    /// theo thu muc rieng la P3).
    pub downloads_dir: PathBuf,
    /// `LaunchOpts::executable_override` (env `LVC_CHROME`).
    pub executable_override: Option<PathBuf>,
    /// config `PROXY` — proxy chung; proxy rieng tung tai khoan thang no.
    pub global_proxy: String,
    /// [`DEFAULT_WARM_IDLE`].
    pub warm_idle: Duration,
    /// [`DEFAULT_BUSY_WAIT_MAX`].
    pub busy_wait_max: Duration,
    /// config `MODE_FALLBACK` (server.py:680, mac dinh bat).
    pub mode_fallback: bool,
    /// `LaunchOpts::logs_dir` (anh chan doan `no_conv_*.png`).
    pub logs_dir: PathBuf,
}

impl EngineConfig {
    /// Cau hinh mac dinh voi hai thu muc bat buoc; cac truong khac sua truc tiep.
    pub fn new(accounts_dir: impl Into<PathBuf>, downloads_dir: impl Into<PathBuf>) -> Self {
        Self {
            accounts_dir: accounts_dir.into(),
            downloads_dir: downloads_dir.into(),
            executable_override: None,
            global_proxy: String::new(),
            warm_idle: DEFAULT_WARM_IDLE,
            busy_wait_max: DEFAULT_BUSY_WAIT_MAX,
            mode_fallback: true,
            logs_dir: PathBuf::from("logs"),
        }
    }
}

/// Ket qua [`Engine::generate_video`]: video + tai khoan da dung + mode THUC TE (co the khac
/// `Scene::mode` khi doi mode vi het luot).
#[derive(Debug, Clone)]
pub struct GenerateOutcome {
    pub artifact: VideoArtifact,
    pub account: String,
    pub mode_used: Mode,
}

/// Bo dieu phoi (brief Task 17). Dung chung qua `Arc` giua cac task; moi `generate_video` la mot
/// job doc lap (cung `&self`).
pub struct Engine<D: BrowserDriver + 'static> {
    driver: Arc<D>,
    pool: Arc<AccountPool>,
    limiter: Arc<DynamicLimiter>,
    cache: Arc<SessionCache<D::Session>>,
    tasks: TaskRepo,
    accounts: AccountRepo,
    limits: Limits,
    cfg: EngineConfig,
    /// Ban sao de `refresh_account_count` goi `set_accounts` (ban goc tra cho caller).
    handle: LimiterHandle,
    /// Reaper cua [`SessionCache`]; `shutdown`/`Drop` abort.
    reaper: Mutex<Option<JoinHandle<()>>>,
}

/// Ket qua mot lan thu tren MOT tai khoan (than vong `for a in candidates` `:1128-1261`).
enum Attempt {
    Done(GenerateOutcome),
    /// Python `continue` — thu tai khoan ke tiep; `Some(e)` ghi `last_err`, `None` khong ghi.
    Rotate(Option<EngineError>),
}

/// Ket qua mo phien cho mot tai khoan ([`Engine::open_session`]).
enum Opened<D: BrowserDriver> {
    Session(D::Session),
    /// Nhu [`Attempt::Rotate`].
    Rotate(Option<EngineError>),
}

/// `s.chars().take(max)` — cat theo ky tu (Python cat str theo code point).
fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// `prefer` rong/khoang trang = khong chi dinh (Python `if prefer_account:`).
fn non_empty(s: &str) -> Option<&str> {
    let s = s.trim();
    (!s.is_empty()).then_some(s)
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Tao engine: pool + limiter (`threads`, so tai khoan = so dong `scheduling=1` luc nay —
    /// [`Engine::refresh_account_count`]) + cache phien am + reaper nen. Tra kem
    /// [`LimiterHandle`] de UI doi `threads`/`accounts` luc dang chay (ADR-016).
    ///
    /// # Errors
    /// `Store` khi doc bang tai khoan loi.
    pub async fn new(
        driver: Arc<D>,
        db: Db,
        limits: Limits,
        cfg: EngineConfig,
        threads: usize,
    ) -> Result<(Self, LimiterHandle), EngineError> {
        let pool = Arc::new(AccountPool::new(
            &db,
            limits.clone(),
            cfg.global_proxy.clone(),
            &cfg.accounts_dir,
        ));
        let (limiter, handle) = DynamicLimiter::new(threads, 0);
        let cache = Arc::new(SessionCache::new());
        let reaper = cache.spawn_reaper(Arc::clone(&driver), cfg.warm_idle);
        let engine = Self {
            driver,
            pool,
            limiter,
            cache,
            tasks: TaskRepo::new(&db),
            accounts: AccountRepo::new(&db),
            limits,
            cfg,
            handle: handle.clone(),
            reaper: Mutex::new(Some(reaper)),
        };
        engine.refresh_account_count().await?;
        Ok((engine, handle))
    }

    #[must_use]
    pub fn pool(&self) -> &Arc<AccountPool> {
        &self.pool
    }

    #[must_use]
    pub fn limiter(&self) -> &Arc<DynamicLimiter> {
        &self.limiter
    }

    #[must_use]
    pub fn cache(&self) -> &Arc<SessionCache<D::Session>> {
        &self.cache
    }

    #[must_use]
    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }

    /// Dem lai so tai khoan `scheduling = 1` va bao cho limiter (`LimiterHandle::set_accounts`) —
    /// goi sau khi UI bat/tat lich mot tai khoan. Tra so vua dem.
    ///
    /// # Errors
    /// `Store` khi doc bang tai khoan loi.
    pub async fn refresh_account_count(&self) -> Result<usize, EngineError> {
        let n = self
            .accounts
            .list()
            .await?
            .iter()
            .filter(|r| r.scheduling)
            .count();
        self.handle.set_accounts(n);
        Ok(n)
    }

    /// Tat engine: dung reaper, dong moi phien am (`close_all`). Goi lai vo hai.
    pub async fn shutdown(&self) {
        self.abort_reaper();
        let closed = self.cache.close_all(&*self.driver).await;
        info!(closed, "engine: da tat");
    }

    fn abort_reaper(&self) {
        if let Some(h) = self
            .reaper
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            h.abort();
        }
    }

    /// Tao MOT video cho `task_id` theo `scene` (brief Task 17). `prefer` (ngu canh batch, P3)
    /// thang `Scene::account`; rong = tu chon. `hooks` duoc chuyen thang cho
    /// `poll_and_download` (khong co cot `stage` trong `tasks` → engine khong ghi them).
    ///
    /// Moi loi deu duoc ghi vao dong task (`Failed` + `failure_code` + `error[:500]`,
    /// `server.py:674-712`) TRUOC khi tra ve, de caller co ca `Result` lan dong DB nhat quan.
    ///
    /// # Errors
    /// Xem [`EngineError`]; `failure_code()` cua loi tra ve = cot `failure_code` da ghi.
    pub async fn generate_video(
        &self,
        task_id: &str,
        scene: &Scene,
        prefer: Option<&str>,
        hooks: &dyn PollHooks,
    ) -> Result<GenerateOutcome, EngineError> {
        // server.py:661 `prefer_account=account_pref or None` — tham so thang Scene.account.
        let prefer = prefer
            .and_then(non_empty)
            .or_else(|| non_empty(&scene.account));
        let res = self.run(task_id, scene, prefer, hooks).await;
        match &res {
            Ok(out) => info!(
                task_id,
                account = %out.account,
                mode = out.mode_used.as_str(),
                conversation_id = %out.artifact.conversation_id,
                "video xong"
            ),
            Err(e) => {
                let code = e.failure_code();
                let msg = clip(&e.to_string(), ERROR_MAX_CHARS);
                warn!(task_id, code = ?code.map(FailureCode::as_str), error = %msg, "task that bai");
                if let Err(se) = self
                    .tasks
                    .set_status(task_id, TaskStatus::Failed, code, Some(&msg))
                    .await
                {
                    warn!(task_id, error = %se, "khong ghi duoc trang thai failed");
                }
            }
        }
        res
    }

    /// `_run_task` `:596-672` (phan engine) + `generate_video` `:1105` (slot).
    async fn run(
        &self,
        task_id: &str,
        scene: &Scene,
        prefer: Option<&str>,
        hooks: &dyn PollHooks,
    ) -> Result<GenerateOutcome, EngineError> {
        info!(
            task_id,
            prompt_chars = scene.prompt.chars().count(),
            mode = scene.mode.as_str(),
            duration = scene.duration,
            model = scene.model.as_str(),
            refs = scene.reference_images.len(),
            prefer,
            "bat dau"
        );
        // server.py:632-634: xoay vong tai khoan khi khong chon (MOT lan moi job) — TRUOC khi xin
        // slot, nhu Python (pick o server, slot o pool :1105): 3 job den cung luc nhan 3 diem bat
        // dau khac nhau (PICK_RESERVE) du chi co 2 slot.
        let start = match prefer {
            Some(_) => None,
            None => self.pool.pick_account(scene.mode, now_unix()).await?,
        };
        // browser_pool.py:1105 `async with self.limiter.slot(...)` — giu suot job.
        let _slot = self.limiter.acquire().await;
        // server.py:680 (v0.55z): het luot che do nay → doi sang che do con luot.
        let chain = if self.cfg.mode_fallback {
            mode_fallback_chain(scene.mode, &self.limits)
        } else {
            vec![scene.mode]
        };
        let mut limited_err = None;
        for (i, &mode) in chain.iter().enumerate() {
            // server.py:682: nan thoi luong theo che do moi (ADR-033/034/044).
            let duration = fit_duration_to_mode(scene.duration, mode, scene.model);
            if i > 0 {
                info!(
                    task_id,
                    from = chain[i - 1].as_str(),
                    to = mode.as_str(),
                    duration,
                    "het luot -> doi che do"
                );
                // server.py:684-685
                self.tasks
                    .set_mode_duration(task_id, mode, duration)
                    .await?;
            }
            match self
                .run_mode(
                    task_id,
                    scene,
                    mode,
                    duration,
                    prefer,
                    start.as_deref(),
                    hooks,
                )
                .await
            {
                Ok(out) => return Ok(out),
                // server.py:668-670: chi AllAccountsLimited moi thu che do ke tiep.
                Err(e @ EngineError::AllAccountsLimited { .. }) => limited_err = Some(e),
                Err(e) => return Err(e),
            }
        }
        // server.py:671-672 `raise limited_err`.
        Err(limited_err.unwrap_or(EngineError::AllAccountsLimited { mode: scene.mode }))
    }

    /// `BrowserPool.generate_video` `:1106-1294` cho MOT mode: vong tai khoan + doi khi ban +
    /// loi cuoi.
    #[allow(clippy::too_many_arguments)] // ngu canh mot job: id/scene/mode/duration/prefer/start/hooks
    async fn run_mode(
        &self,
        task_id: &str,
        scene: &Scene,
        mode: Mode,
        duration: u16,
        prefer: Option<&str>,
        start: Option<&str>,
        hooks: &dyn PollHooks,
    ) -> Result<GenerateOutcome, EngineError> {
        // browser_pool.py:1124 busy_deadline
        let busy_deadline = Instant::now() + self.cfg.busy_wait_max;
        let mut last_err: Option<EngineError> = None;
        loop {
            // browser_pool.py:1108-1123 (prefer → [chosen] / PreferredUnavailable; khong → candidates_for)
            let candidates = self
                .pool
                .candidates_for(mode, prefer, start, now_unix())
                .await?;
            let mut busy_skipped = false;
            for row in &candidates {
                let name = row.name.as_str();
                // browser_pool.py:1129-1136: tai khoan ban → bo qua (prefer → DOI khoa, REQ-004)
                let lease = if prefer.is_some() {
                    self.pool
                        .lease_wait(name, &row.proxy, self.cfg.busy_wait_max)
                        .await?
                } else if let Some(l) = self.pool.try_lease(name, &row.proxy) {
                    l
                } else {
                    busy_skipped = true;
                    continue;
                };
                // browser_pool.py:1138-1145: doc lai trang thai SAU khi co khoa (tai khoan truoc
                // co the da chay nhieu phut; luong khac co the vua danh dau tai khoan nay).
                if !self.still_schedulable(mode, prefer, name).await? {
                    info!(account = name, "trang thai doi trong luc cho -> bo qua");
                    continue;
                }
                match self
                    .attempt(task_id, scene, mode, duration, row, &lease, hooks)
                    .await?
                {
                    Attempt::Done(out) => return Ok(out),
                    Attempt::Rotate(e) => {
                        if e.is_some() {
                            last_err = e;
                        }
                    }
                }
                drop(lease);
            }
            // browser_pool.py:1262-1267 (ADR-016): moi tai khoan con luot deu ban → doi roi thu lai
            if busy_skipped && prefer.is_none() && Instant::now() < busy_deadline {
                tokio::time::sleep(BUSY_POLL).await;
                continue;
            }
            // browser_pool.py:1268-1269
            if busy_skipped && last_err.is_none() {
                last_err = Some(EngineError::NoFreeProfile);
            }
            break;
        }
        Err(self.final_error(mode, prefer, last_err).await)
    }

    /// browser_pool.py:1138-1145: `prefer` → `PreferredUnavailable` neu vua bi chan cung (loi noi
    /// len); khong → `false` neu khong con trong danh sach lich duoc ("State changed while waiting").
    async fn still_schedulable(
        &self,
        mode: Mode,
        prefer: Option<&str>,
        name: &str,
    ) -> Result<bool, EngineError> {
        let rows = self
            .pool
            .candidates_for(mode, prefer, None, now_unix())
            .await?;
        Ok(rows.iter().any(|r| r.name == name))
    }

    /// browser_pool.py:1279-1294: loi cuoi khi khong con tai khoan nao de thu.
    async fn final_error(
        &self,
        mode: Mode,
        prefer: Option<&str>,
        last_err: Option<EngineError>,
    ) -> EngineError {
        // :1279-1281 tai khoan chi dinh hong giua chung → bao ro, khong doi tai khoan (ADR-011)
        if let Some(p) = prefer {
            let why = last_err.map_or_else(
                || "busy".to_string(),
                |e| clip(&e.to_string(), PREFER_REASON_MAX_CHARS),
            );
            return EngineError::PreferredUnavailable(format!("account '{p}': {why}"));
        }
        match last_err {
            // :1282-1283 PROFILE_BUSY
            Some(
                EngineError::Browser(BrowserError::ProfileBusy(_))
                | EngineError::Dola(DolaError::Browser(BrowserError::ProfileBusy(_)))
                | EngineError::NoFreeProfile,
            ) => return EngineError::NoFreeProfile,
            // :1284-1286 AllAccountsLoggedOutError → server.py:680-689 LOGGED_OUT (requeue: P3)
            Some(e @ EngineError::Dola(DolaError::LoggedOut { .. })) => return e,
            _ => {}
        }
        // :1290-1293 AllAccountsLimitedError (429); :1294 RuntimeError khong ma → ruling Task 17:
        // candidates rong/khong loi cu the → cung AllAccountsLimited (→ thu mode ke tiep).
        match self.pool.all_limited_for(mode, now_unix()).await {
            Err(e) => e,
            Ok(true) => EngineError::AllAccountsLimited { mode },
            Ok(false) => last_err.unwrap_or(EngineError::AllAccountsLimited { mode }),
        }
    }

    /// `warm_browser_manager.acquire` (`:190-227`) + `proxy_precheck` (`:1146-1149`): phien am
    /// hoac mo Chrome moi cho `row`. `Err` = loi khong xoay; `Ok(Rotate)` = thu tai khoan ke tiep.
    async fn open_session(
        &self,
        row: &AccountRow,
        lease: &Lease,
    ) -> Result<Opened<D>, EngineError> {
        let name = row.name.as_str();
        let browser = match BrowserKind::parse(&row.browser) {
            Ok(b) => b,
            Err(e) => {
                warn!(account = name, error = %e, "bo qua: cot browser khong hop le");
                return Ok(Opened::Rotate(None));
            }
        };
        let opts = LaunchOpts {
            profile_dir: self.cfg.accounts_dir.join(name),
            browser,
            executable_override: self.cfg.executable_override.clone(),
            headless: false,
            proxy: lease.proxy.clone(),
            logs_dir: self.cfg.logs_dir.clone(),
            ..LaunchOpts::default()
        };
        match self.cache.take_or_launch(name, &opts, &*self.driver).await {
            Ok(s) => Ok(Opened::Session(s)),
            // Cau hinh sai (binary khong co) — moi tai khoan deu loi y het: tra ve ngay, khong xoay.
            Err(e @ BrowserError::ExecutableNotFound { .. }) => Err(e.into()),
            // browser_pool.py:1245-1261 (dot 5 BUG-002): profile bi giu → cooldown 300 s, xoay.
            Err(BrowserError::ProfileBusy(why)) => {
                warn!(account = name, why = %why, "profile busy -> cooldown, xoay");
                self.pool
                    .mark_cooldown(name, COOLDOWN_PROFILE_BUSY, now_unix())
                    .await?;
                Ok(Opened::Rotate(Some(EngineError::Browser(
                    BrowserError::ProfileBusy(why),
                ))))
            }
            // browser_pool.py:1146-1149 proxy_precheck (N1-IDEA-001): proxy rieng chet → xoay.
            Err(e) if e.is_proxy_down() => {
                warn!(account = name, error = %e, "proxy chet -> xoay");
                Ok(Opened::Rotate(Some(EngineError::ProxyDown {
                    account: name.to_string(),
                    why: e.to_string(),
                })))
            }
            // Loi mo trinh duyet khac (ruling Task 17: thu tai khoan ke tiep; Python raise).
            Err(e) => {
                warn!(account = name, error = %e, "khong mo duoc trinh duyet -> xoay");
                Ok(Opened::Rotate(Some(e.into())))
            }
        }
    }

    /// Mot lan thu tren tai khoan `row` (dang giu `lease`): mo/tai dung phien → gui → cho video.
    /// `Err` = loi khong xoay (tra ve ngay); `Ok(Rotate)` = thu tai khoan ke tiep.
    #[allow(clippy::too_many_arguments)] // ngu canh mot job (xem run_mode)
    async fn attempt(
        &self,
        task_id: &str,
        scene: &Scene,
        mode: Mode,
        duration: u16,
        row: &AccountRow,
        lease: &Lease,
        hooks: &dyn PollHooks,
    ) -> Result<Attempt, EngineError> {
        let name = row.name.as_str();
        let session = match self.open_session(row, lease).await? {
            Opened::Session(s) => s,
            Opened::Rotate(e) => return Ok(Attempt::Rotate(e)),
        };
        // server.py:610 + on_account :625-627
        self.tasks
            .set_status(task_id, TaskStatus::Processing, None, None)
            .await?;
        self.tasks.set_account(task_id, name).await?;
        info!(
            task_id,
            account = name,
            mode = mode.as_str(),
            duration,
            "gui yeu cau"
        );
        let req = SubmitRequest {
            account: name.to_string(),
            prompt: scene.prompt.clone(),
            mode,
            ratio: Some(scene.ratio),
            duration,
            model: scene.model,
            // server.py:628 download_reference_images — o day coi la duong dan cuc bo (xem doc module).
            reference_images: scene.reference_images.iter().map(PathBuf::from).collect(),
        };
        let conv = match self.driver.submit(&session, &req, &self.limits).await {
            Ok(c) => c,
            Err(e) => return self.after_dola_error(name, mode, e, session).await,
        };
        // on_conversation_id :612-619
        self.tasks.set_conversation(task_id, &conv.0).await?;
        info!(task_id, account = name, conversation_id = %conv, "da co hoi thoai, cho video");
        let popts = PollOpts {
            // video_worker_ui.py:2398 + :2404-2405 (lvc_core::video_timeout gop ca anh tham chieu)
            timeout: Duration::from_secs(video_timeout(
                mode,
                duration,
                !req.reference_images.is_empty(),
                &self.limits,
            )),
            mode,
            duration,
            ignore_videos: std::collections::HashSet::new(),
            ignore_links: std::collections::HashSet::new(),
            // :2503 accept_links=is_pro_like(mode) (tham so chet trong _handle_poll — giu de khop)
            accept_links: mode.is_pro_like(),
        };
        self.wait_video(task_id, name, mode, &conv, &popts, session, hooks)
            .await
    }

    /// `poll_and_download` + ket thuc: Ok → `_claim` (`:1186-1189`) + giu phien + `set_result` +
    /// `Completed`; Err → [`Engine::after_dola_error`].
    #[allow(clippy::too_many_arguments)] // ngu canh mot job (xem run_mode)
    async fn wait_video(
        &self,
        task_id: &str,
        name: &str,
        mode: Mode,
        conv: &ConversationId,
        popts: &PollOpts,
        session: D::Session,
        hooks: &dyn PollHooks,
    ) -> Result<Attempt, EngineError> {
        let polled = self
            .driver
            .poll_and_download(
                &session,
                name,
                conv,
                &self.cfg.downloads_dir,
                popts,
                &self.limits,
                hooks,
            )
            .await;
        match polled {
            Ok(artifact) => {
                // browser_pool.py:1186-1189 _claim + last_used_at
                self.pool.claim(name, mode, now_unix()).await?;
                self.stash(name, session).await;
                // _finish_task :442-480 (phan luu; hau ky logo/H.264 P3)
                self.tasks
                    .set_result(task_id, &artifact.video_url, &artifact.local_path)
                    .await?;
                self.tasks
                    .set_status(task_id, TaskStatus::Completed, None, None)
                    .await?;
                Ok(Attempt::Done(GenerateOutcome {
                    artifact,
                    account: name.to_string(),
                    mode_used: mode,
                }))
            }
            Err(e) => self.after_dola_error(name, mode, e, session).await,
        }
    }

    /// Cac `except` cua `browser_pool.py:1192-1261` + dong/giu phien am (`video_worker_ui.py:
    /// 2507-2511`: chi `LoggedOut`/`ProfileBusy`/`RiskControl`/`AccountLimited` dong; con lai giu):
    ///
    /// | Loi | Danh dau | Phien | Ket qua |
    /// |---|---|---|---|
    /// | `LoggedOut{revoked}` `:1192-1197` (REQ-002) | `mark_logged_out` | dong | xoay |
    /// | `Credit` `:1198-1202` | `mark_quota_blocked` | giu | xoay |
    /// | `AccountLimited` `:1203-1211` | `mark_daily_limit` | dong | xoay |
    /// | `RiskControl` `:1219-1229` | cooldown 1800 s | dong | xoay |
    /// | `Browser(ProfileBusy)` `:1245-1261` | cooldown 300 s | dong | xoay |
    /// | `Browser(proxy chet)` (`proxy_precheck` :1146) | — | dong | xoay (`ProxyDown`) |
    /// | `Timeout` `:1230-1237` | `claim` (Dola van chay, khong gui lai) | giu | loi |
    /// | khac (`:1240-1256` raise) | — | giu neu con ket noi | loi |
    async fn after_dola_error(
        &self,
        name: &str,
        mode: Mode,
        e: DolaError,
        session: D::Session,
    ) -> Result<Attempt, EngineError> {
        let now = now_unix();
        let reason = e.to_string();
        match &e {
            DolaError::LoggedOut { revoked, .. } => {
                warn!(account = name, revoked, error = %reason, "logged out, rotating");
                self.pool
                    .mark_logged_out(name, &reason, *revoked, now)
                    .await?;
                self.close(name, session).await;
                Ok(Attempt::Rotate(Some(e.into())))
            }
            DolaError::Credit(_) => {
                warn!(account = name, error = %reason, "insufficient points, skipping");
                self.pool.mark_quota_blocked(name, &reason, now).await?;
                self.stash(name, session).await;
                Ok(Attempt::Rotate(Some(e.into())))
            }
            DolaError::AccountLimited(_) => {
                warn!(account = name, error = %reason, "reached daily limit, rotating");
                self.pool.mark_daily_limit(name, mode, &reason, now).await?;
                self.close(name, session).await;
                Ok(Attempt::Rotate(Some(e.into())))
            }
            DolaError::RiskControl(_) => {
                warn!(account = name, error = %reason, "risk control (30m cooldown), rotating");
                self.pool
                    .mark_cooldown(name, COOLDOWN_RISK_CONTROL, now)
                    .await?;
                self.close(name, session).await;
                Ok(Attempt::Rotate(Some(e.into())))
            }
            DolaError::Browser(BrowserError::ProfileBusy(_)) => {
                warn!(account = name, error = %reason, "profile busy, cooldown, rotating");
                self.pool
                    .mark_cooldown(name, COOLDOWN_PROFILE_BUSY, now)
                    .await?;
                self.close(name, session).await;
                Ok(Attempt::Rotate(Some(e.into())))
            }
            DolaError::Browser(b) if b.is_proxy_down() => {
                warn!(account = name, error = %reason, "proxy chet -> xoay");
                self.close(name, session).await;
                Ok(Attempt::Rotate(Some(EngineError::ProxyDown {
                    account: name.to_string(),
                    why: reason,
                })))
            }
            DolaError::Timeout(_) => {
                // Da co conversation_id → Dola van tao; tru luot de khong gui lai (mat 2 credit).
                warn!(account = name, error = %reason, "timeout sau khi da gui -> van tru luot");
                self.pool.claim(name, mode, now).await?;
                self.stash(name, session).await;
                Err(e.into())
            }
            _ => {
                self.stash(name, session).await;
                Err(e.into())
            }
        }
    }

    /// Tra phien ve cache neu con ket noi, khong thi dong (`release(close=False)`).
    async fn stash(&self, name: &str, session: D::Session) {
        if self.driver.is_connected(&session) {
            if let Some(old) = self.cache.put(name, session).await {
                warn!(
                    account = name,
                    "cache da co phien cung ten -> dong phien cu"
                );
                self.close(name, old).await;
            }
        } else {
            self.close(name, session).await;
        }
    }

    /// `close_session` best-effort (loi chi `warn`).
    async fn close(&self, name: &str, session: D::Session) {
        if let Err(e) = self.driver.close(session).await {
            warn!(account = name, error = %e, "dong trinh duyet loi (bo qua)");
        }
    }
}

impl<D: BrowserDriver + 'static> Drop for Engine<D> {
    /// Dung reaper khi engine bi drop ma khong `shutdown` (phien am se do Job Object/`Drop` cua
    /// driver xu ly — khong `close` duoc trong `Drop` dong bo).
    fn drop(&mut self) {
        self.abort_reaper();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use lvc_browser::dola::{ConversationId, NoopHooks, VideoArtifact, poll::NOTE_RAW};
    use lvc_browser::{BrowserError, DolaError, FakeCall, FakeDriver, FakeStep};
    use lvc_core::{
        BrowserKind, FailureCode, Limits, Mode, RawScene, Scene, TaskStatus, tokyo_day,
    };
    use lvc_store::{AccountRepo, Db, TaskRepo};

    use super::*;
    use crate::EngineError;
    use crate::pool::{COOLDOWN_PROFILE_BUSY, COOLDOWN_RISK_CONTROL};

    struct Fx {
        dir: tempfile::TempDir,
        driver: Arc<FakeDriver>,
        engine: Engine<FakeDriver>,
        tasks: TaskRepo,
        accounts: AccountRepo,
    }

    impl Fx {
        fn accounts_dir(&self) -> PathBuf {
            self.dir.path().join("accounts")
        }
        fn calls(&self) -> Vec<FakeCall> {
            self.driver.calls.lock().unwrap().clone()
        }
        fn launch(&self, name: &str) -> FakeCall {
            FakeCall::Launch {
                profile_dir: self.accounts_dir().join(name),
                browser: BrowserKind::Chromium,
            }
        }
        async fn task(&self, id: &str, scene: &Scene) {
            self.tasks.create(id, scene, None).await.unwrap();
        }
        async fn row(&self, id: &str) -> lvc_store::TaskRow {
            self.tasks.get(id).await.unwrap().expect("task phai co")
        }
        async fn used(&self, name: &str, mode: Mode) -> i64 {
            self.accounts
                .used_today(name, &tokyo_day(now_unix()), mode)
                .await
                .unwrap()
        }
        async fn account(&self, name: &str) -> lvc_store::AccountRow {
            self.accounts.get(name).await.unwrap().unwrap()
        }
        async fn run(
            &self,
            id: &str,
            scene: &Scene,
            prefer: Option<&str>,
        ) -> Result<GenerateOutcome, EngineError> {
            self.engine
                .generate_video(id, scene, prefer, &NoopHooks)
                .await
        }
    }

    async fn fx_with(
        names: &[&str],
        steps: Vec<FakeStep>,
        limits: Limits,
        tune: impl FnOnce(&mut EngineConfig),
    ) -> Fx {
        let db = Db::open_memory().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let accounts_dir = dir.path().join("accounts");
        let accounts = AccountRepo::new(&db);
        for n in names {
            std::fs::create_dir_all(accounts_dir.join(n)).unwrap();
            accounts.upsert(n).await.unwrap();
        }
        let mut cfg = EngineConfig::new(&accounts_dir, dir.path().join("downloads"));
        cfg.logs_dir = dir.path().join("logs");
        tune(&mut cfg);
        let driver = Arc::new(FakeDriver::new(steps));
        let (engine, _handle) = Engine::new(Arc::clone(&driver), db.clone(), limits, cfg, 4)
            .await
            .unwrap();
        Fx {
            dir,
            driver,
            engine,
            tasks: TaskRepo::new(&db),
            accounts,
        }
    }

    async fn fx(names: &[&str], steps: Vec<FakeStep>) -> Fx {
        fx_with(names, steps, Limits::default(), |_| {}).await
    }

    /// 9:16, 10 s, seedance-2.0, fast, khong anh, khong chi dinh tai khoan.
    fn scene() -> Scene {
        Scene::normalize(
            &RawScene {
                prompt: Some("con meo di bo".into()),
                ..RawScene::default()
            },
            &Limits::default(),
            None,
        )
        .unwrap()
    }

    fn conv(id: &str) -> ConversationId {
        ConversationId(id.to_string())
    }

    fn artifact(account: &str, conv_id: &str) -> VideoArtifact {
        VideoArtifact {
            video_url: format!("https://v.example/{account}.mp4"),
            local_path: PathBuf::from("downloads").join(format!("{account}.mp4")),
            conversation_id: conv(conv_id),
            account: account.to_string(),
            watermarked: false,
            bytes: 10,
            note: NOTE_RAW,
        }
    }

    fn logged_out(account: &str, revoked: bool) -> DolaError {
        DolaError::LoggedOut {
            account: account.into(),
            why: "no sessionid cookie".into(),
            revoked,
        }
    }

    fn submit(account: &str, mode: Mode, duration: u16) -> FakeCall {
        FakeCall::Submit {
            account: account.into(),
            mode,
            duration,
        }
    }

    fn poll(account: &str, conv_id: &str) -> FakeCall {
        FakeCall::Poll {
            account: account.into(),
            conv: conv(conv_id),
        }
    }

    /// Kich ban hanh phuc tren 1 tai khoan: launch -> submit -> poll Ok.
    fn happy(account: &str, conv_id: &str) -> Vec<FakeStep> {
        vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(conv(conv_id))),
            FakeStep::Poll(Ok(artifact(account, conv_id))),
        ]
    }

    fn assert_near(got: f64, want: f64, tol: f64) {
        assert!((got - want).abs() <= tol, "got {got}, want {want} ± {tol}");
    }

    // ---- brief: 9 test ----

    #[tokio::test]
    async fn thanh_cong_claim_usage() {
        let f = fx(&["a", "b"], happy("a", "111")).await;
        f.task("t1", &scene()).await;

        let out = f.run("t1", &scene(), None).await.expect("phai thanh cong");
        assert_eq!(out.account, "a", "pick_account: a (spread key nho nhat)");
        assert_eq!(out.mode_used, Mode::Fast);
        assert_eq!(out.artifact.conversation_id, conv("111"));

        // thu tu goi driver: khong Close — phien giu am trong cache
        assert_eq!(
            f.calls(),
            vec![f.launch("a"), submit("a", Mode::Fast, 10), poll("a", "111")]
        );
        assert_eq!(f.engine.cache().names().await, vec!["a".to_string()]);

        // browser_pool.py:1186-1189 _claim + last_used_at
        assert_eq!(f.used("a", Mode::Fast).await, 1);
        assert_eq!(f.used("b", Mode::Fast).await, 0);
        assert!(f.account("a").await.last_used_at > 0.0);

        // dong task: completed, account, conversation_id, video_url, local_path
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Completed);
        assert_eq!(row.failure_code, None);
        assert_eq!(row.account.as_deref(), Some("a"));
        assert_eq!(row.conversation_id.as_deref(), Some("111"));
        assert_eq!(row.video_url.as_deref(), Some("https://v.example/a.mp4"));
        assert_eq!(
            row.local_path.as_deref(),
            Some(
                PathBuf::from("downloads")
                    .join("a.mp4")
                    .display()
                    .to_string()
                    .as_str()
            )
        );
    }

    #[tokio::test]
    async fn logged_out_xoay_sang_tai_khoan_2() {
        // browser_pool.py:1192-1197 + video_worker_ui.py:2507-2509 (dong phien)
        let mut steps = vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Err(logged_out("a", false))),
        ];
        steps.extend(happy("b", "222"));
        let f = fx(&["a", "b"], steps).await;
        f.task("t1", &scene()).await;

        let out = f.run("t1", &scene(), None).await.expect("b phai chay");
        assert_eq!(out.account, "b");
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                submit("a", Mode::Fast, 10),
                FakeCall::Close,
                f.launch("b"),
                submit("b", Mode::Fast, 10),
                poll("b", "222"),
            ]
        );
        let a = f.account("a").await;
        assert!(!a.login_ok, "mark_logged_out");
        assert_eq!(a.auto_verify_state, "");
        assert_eq!(f.used("a", Mode::Fast).await, 0, "khong tru luot a");
        assert_eq!(f.used("b", Mode::Fast).await, 1);
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Completed);
        assert_eq!(row.account.as_deref(), Some("b"));
        assert_eq!(row.conversation_id.as_deref(), Some("222"));
        assert_eq!(f.engine.cache().names().await, vec!["b".to_string()]);
    }

    #[tokio::test]
    async fn ca_2_het_han_muc_bao_429() {
        // browser_pool.py:1203-1211 mark_daily_limit + dong phien; het ca vi -> :1290-1293
        // AllAccountsLimitedError -> server.py:677-679 failure_code "429". Mode fallback (mac
        // dinh bat) chay het chuoi nhung rate_limited chan MOI mode -> khong mo them trinh duyet.
        let f = fx(
            &["a", "b"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Err(DolaError::AccountLimited("het luot".into()))),
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Err(DolaError::AccountLimited("het luot".into()))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;

        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(matches!(e, EngineError::AllAccountsLimited { .. }), "{e:?}");
        assert_eq!(e.failure_code(), Some(FailureCode::RateLimited));
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                submit("a", Mode::Fast, 10),
                FakeCall::Close,
                f.launch("b"),
                submit("b", Mode::Fast, 10),
                FakeCall::Close,
            ]
        );
        for n in ["a", "b"] {
            let row = f.account(n).await;
            assert!(row.rate_limited_until > now_unix(), "{n} rate_limited");
            assert_eq!(row.limit_reason, "LIMIT: het luot");
            assert_eq!(
                f.used(n, Mode::Fast).await,
                i64::from(Limits::default().daily_limit(Mode::Fast))
            );
        }
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::RateLimited));
        assert!(
            row.error.as_deref().unwrap_or("").starts_with("429: "),
            "{:?}",
            row.error
        );
        assert!(f.engine.cache().is_empty().await);
    }

    #[tokio::test]
    async fn prompt_violation_khong_xoay() {
        // browser_pool.py:1240-1256: loi khong thuoc nhom xoay -> raise; server.py:691-693 ->
        // PROMPT_VIOLATION. Phien KHONG dong (video_worker_ui.py:2507-2511 chi dong 4 loai).
        let f = fx(
            &["a", "b"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Ok(conv("333"))),
                FakeStep::Poll(Err(DolaError::PromptViolation("chinh sach".into()))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;

        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(
            matches!(e, EngineError::Dola(DolaError::PromptViolation(_))),
            "{e:?}"
        );
        assert_eq!(
            f.calls(),
            vec![f.launch("a"), submit("a", Mode::Fast, 10), poll("a", "333")],
            "khong mo tai khoan b, khong Close"
        );
        assert_eq!(f.used("a", Mode::Fast).await, 0, "khong claim");
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::PromptViolation));
        assert_eq!(row.error.as_deref(), Some("PROMPT_VIOLATION: chinh sach"));
        assert_eq!(row.account.as_deref(), Some("a"));
        assert_eq!(row.conversation_id.as_deref(), Some("333"));
        assert_eq!(f.engine.cache().names().await, vec!["a".to_string()]);
    }

    #[tokio::test]
    async fn profile_busy_cooldown_300() {
        // browser_pool.py:1245-1261 (dot 5 BUG-002): profile bi giu -> cooldown
        // PROFILE_BUSY_COOLDOWN (config.py:290 = 300 s), xoay tai khoan.
        let mut steps = vec![FakeStep::Launch(Err(BrowserError::ProfileBusy(
            "SingletonLock".into(),
        )))];
        steps.extend(happy("b", "444"));
        let f = fx(&["a", "b"], steps).await;
        f.task("t1", &scene()).await;

        let t0 = now_unix();
        let out = f.run("t1", &scene(), None).await.expect("b phai chay");
        assert_eq!(out.account, "b");
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                f.launch("b"),
                submit("b", Mode::Fast, 10),
                poll("b", "444")
            ]
        );
        assert_near(
            f.account("a").await.cooldown_until,
            t0 + COOLDOWN_PROFILE_BUSY.as_secs_f64(),
            5.0,
        );
        assert!(f.account("b").await.cooldown_until.abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn timeout_van_claim() {
        // browser_pool.py:1230-1237: da co conversation_id -> Dola van chay -> tru luot, KHONG
        // gui lai, loi noi len (khong ma). Phien giu am.
        let f = fx(
            &["a", "b"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Ok(conv("555"))),
                FakeStep::Poll(Err(DolaError::Timeout(
                    "No video generated within 600s".into(),
                ))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;

        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(
            matches!(e, EngineError::Dola(DolaError::Timeout(_))),
            "{e:?}"
        );
        assert_eq!(f.calls().len(), 3, "khong xoay, khong Close");
        assert_eq!(f.used("a", Mode::Fast).await, 1, "van claim");
        assert!(f.account("a").await.last_used_at > 0.0);
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, None);
        assert_eq!(
            row.error.as_deref(),
            Some("TIMEOUT: No video generated within 600s")
        );
        assert_eq!(row.conversation_id.as_deref(), Some("555"));
        assert_eq!(f.engine.cache().names().await, vec!["a".to_string()]);
    }

    #[tokio::test]
    async fn prefer_account_ban_cho_roi_chay() {
        // browser_pool.py:1133-1136 (dot 7 REQ-004): tai khoan chi dinh dang ban -> DOI khoa,
        // khong xoay sang tai khoan khac (a ranh nhung khong duoc dung).
        let f = fx(&["a", "b"], happy("b", "666")).await;
        f.task("t1", &scene()).await;
        let held = f.engine.pool().try_lease("b", "").expect("b ranh");
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            drop(held);
        });

        let t0 = Instant::now();
        let out = f.run("t1", &scene(), Some("b")).await.expect("b phai chay");
        assert!(
            t0.elapsed() >= Duration::from_millis(100),
            "phai cho khoa ({:?})",
            t0.elapsed()
        );
        assert_eq!(out.account, "b");
        assert_eq!(
            f.calls(),
            vec![f.launch("b"), submit("b", Mode::Fast, 10), poll("b", "666")],
            "khong dung a"
        );
        assert_eq!(f.used("b", Mode::Fast).await, 1);
        assert_eq!(f.row("t1").await.account.as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn fallback_mode_agent_sang_pro_khi_het() {
        // server.py:680-698 (v0.55z): het luot agent (ca 2 tai khoan 1/1) -> AllAccountsLimited
        // -> doi sang pro (chain agent -> pro -> pro_max -> fast), duration nan theo mode moi.
        let limits = Limits {
            daily_limit_agent: 1,
            ..Limits::default()
        };
        let f = fx_with(&["a", "b"], happy("a", "777"), limits, |_| {}).await;
        let day = tokyo_day(now_unix());
        f.accounts
            .set_usage("a", &day, Mode::Agent, 1)
            .await
            .unwrap();
        f.accounts
            .set_usage("b", &day, Mode::Agent, 1)
            .await
            .unwrap();
        let mut sc = scene();
        sc.mode = Mode::Agent;
        sc.duration = 15;
        f.task("t1", &sc).await;

        let out = f.run("t1", &sc, None).await.expect("pro phai chay");
        assert_eq!(out.mode_used, Mode::Pro);
        assert_eq!(out.account, "a");
        assert_eq!(
            f.calls(),
            vec![f.launch("a"), submit("a", Mode::Pro, 15), poll("a", "777")]
        );
        assert_eq!(f.used("a", Mode::Pro).await, 1, "claim dung mode da dung");
        assert_eq!(f.used("a", Mode::Agent).await, 1, "agent khong doi");
        // server.py:684-685 store.update(mode=try_mode, duration=try_duration)
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Completed);
        assert_eq!(row.mode, "pro");
        assert_eq!(row.duration, 15);
    }

    #[tokio::test]
    async fn cache_giu_session_va_reaper_dong_sau_idle() {
        // WarmBrowserManager: phien giu am sau khi xong; cleanup_idle (:159-166) dong sau
        // WARM_SESSION_IDLE_TIMEOUT (config.py:115, o day 100 ms); lan chay sau mo lai.
        let mut steps = happy("a", "888");
        steps.extend(happy("a", "889"));
        let f = fx_with(&["a"], steps, Limits::default(), |cfg| {
            cfg.warm_idle = Duration::from_millis(100);
        })
        .await;
        f.task("t1", &scene()).await;
        f.task("t2", &scene()).await;

        f.run("t1", &scene(), None).await.unwrap();
        assert_eq!(f.engine.cache().names().await, vec!["a".to_string()]);
        assert_eq!(f.calls().len(), 3, "chua Close");

        let t0 = Instant::now();
        while !f.engine.cache().is_empty().await {
            assert!(
                t0.elapsed() < Duration::from_secs(3),
                "reaper khong dong phien idle"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(f.calls().last(), Some(&FakeCall::Close));

        // phien da dong -> lan sau launch lai (seq moi), khong tai dung
        f.run("t2", &scene(), None).await.unwrap();
        let calls = f.calls();
        assert_eq!(calls.len(), 7);
        assert_eq!(calls[4], f.launch("a"));
        assert_eq!(f.used("a", Mode::Fast).await, 2);
    }

    // ---- them theo ruling Task 17 ----

    #[tokio::test]
    async fn executable_not_found_loi_ngay() {
        // Cau hinh sai (executable_override khong ton tai / khong resolve duoc binary) -> khong
        // xoay tai khoan (moi tai khoan deu se loi y het), tra loi ngay, khong ma.
        let f = fx(
            &["a", "b"],
            vec![FakeStep::Launch(Err(BrowserError::ExecutableNotFound {
                kind: BrowserKind::Chromium,
                tried: vec![PathBuf::from(r"C:\khong\co\chrome.exe")],
            }))],
        )
        .await;
        f.task("t1", &scene()).await;

        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(
            matches!(
                e,
                EngineError::Browser(BrowserError::ExecutableNotFound { .. })
            ),
            "{e:?}"
        );
        assert_eq!(f.calls(), vec![f.launch("a")], "khong thu b");
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, None);
        assert!(
            row.error
                .as_deref()
                .unwrap_or("")
                .contains("khong tim thay file chay"),
            "{:?}",
            row.error
        );
        assert!(f.account("a").await.cooldown_until.abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn store_row_failed_co_failure_code() {
        // server.py:674-676 (ADR-011): tai khoan chi dinh khong ton tai -> ACCOUNT_UNAVAILABLE,
        // error = thong diep; khong mo trinh duyet nao.
        let f = fx(&["a"], vec![]).await;
        f.task("t1", &scene()).await;
        let e = f.run("t1", &scene(), Some("ghost")).await.unwrap_err();
        assert!(matches!(e, EngineError::PreferredUnavailable(_)), "{e:?}");
        assert!(f.calls().is_empty());
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::AccountUnavailable));
        assert_eq!(
            row.error.as_deref(),
            Some("ACCOUNT_UNAVAILABLE: account 'ghost' does not exist")
        );
        assert!(row.account.is_none());

        // thong diep loi cat 500 ky tu (server.py:675 `str(e)[:500]`)
        f.task("t2", &scene()).await;
        let long = "x".repeat(700);
        let e = f.run("t2", &scene(), Some(&long)).await.unwrap_err();
        assert!(matches!(e, EngineError::PreferredUnavailable(_)));
        assert_eq!(f.row("t2").await.error.unwrap().chars().count(), 500);
    }

    #[tokio::test]
    async fn risk_control_cooldown_1800_va_xoay() {
        // browser_pool.py:1219-1229: captcha/risk control -> cooldown COOLDOWN_SEC (:26 = 1800 s),
        // dong phien (:1223-1227 close_session), xoay.
        let mut steps = vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Err(DolaError::RiskControl("bdcaptcha".into()))),
        ];
        steps.extend(happy("b", "999"));
        let f = fx(&["a", "b"], steps).await;
        f.task("t1", &scene()).await;

        let t0 = now_unix();
        let out = f.run("t1", &scene(), None).await.expect("b phai chay");
        assert_eq!(out.account, "b");
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                submit("a", Mode::Fast, 10),
                FakeCall::Close,
                f.launch("b"),
                submit("b", Mode::Fast, 10),
                poll("b", "999"),
            ]
        );
        assert_near(
            f.account("a").await.cooldown_until,
            t0 + COOLDOWN_RISK_CONTROL.as_secs_f64(),
            5.0,
        );
        assert_eq!(f.used("a", Mode::Fast).await, 0);
        assert_eq!(f.engine.cache().names().await, vec!["b".to_string()]);
    }

    // ---- cac nhanh con lai cua browser_pool.py:1090-1287 ----

    #[tokio::test]
    async fn credit_het_mark_quota_giu_phien_va_xoay() {
        // browser_pool.py:1198-1202 CreditInsufficientError -> _mark_quota_blocked + xoay;
        // video_worker_ui.py:2507 KHONG dong phien khi het credit (giu am).
        let mut steps = vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Err(DolaError::Credit("Insufficient credits".into()))),
        ];
        steps.extend(happy("b", "10"));
        let f = fx(&["a", "b"], steps).await;
        f.task("t1", &scene()).await;

        let out = f.run("t1", &scene(), None).await.expect("b phai chay");
        assert_eq!(out.account, "b");
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                submit("a", Mode::Fast, 10),
                f.launch("b"),
                submit("b", Mode::Fast, 10),
                poll("b", "10"),
            ],
            "khong Close a"
        );
        let a = f.account("a").await;
        assert!(a.quota_blocked_until > now_unix());
        assert_eq!(a.quota_reason, "CREDIT: Insufficient credits");
        let mut names = f.engine.cache().names().await;
        names.sort();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn poll_logged_out_cung_xoay() {
        // Cac `except` :1192-1261 boc ca vong poll (generate_video worker) — mat phien luc dang
        // cho video cung xoay (chat cu bo, tai khoan ke gui lai).
        let mut steps = vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(conv("11"))),
            FakeStep::Poll(Err(logged_out("a", true))),
        ];
        steps.extend(happy("b", "12"));
        let f = fx(&["a", "b"], steps).await;
        f.task("t1", &scene()).await;

        let out = f.run("t1", &scene(), None).await.expect("b phai chay");
        assert_eq!(out.account, "b");
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                submit("a", Mode::Fast, 10),
                poll("a", "11"),
                FakeCall::Close,
                f.launch("b"),
                submit("b", Mode::Fast, 10),
                poll("b", "12"),
            ]
        );
        let a = f.account("a").await;
        assert!(!a.login_ok);
        assert_eq!(
            a.auto_verify_state, "revoked",
            "revoked=true -> khong auto-verify"
        );
        let row = f.row("t1").await;
        assert_eq!(row.account.as_deref(), Some("b"));
        assert_eq!(row.conversation_id.as_deref(), Some("12"), "chat moi cua b");
    }

    #[tokio::test]
    async fn proxy_down_bo_qua_tai_khoan_roi_bao_proxy_down() {
        // browser_pool.py:1146-1149 proxy_precheck: proxy rieng chet -> ProxyDownError, xoay;
        // ca 2 chet -> loi cuoi = ProxyDown (ma PROXY_DOWN).
        let nav = || BrowserError::Navigation {
            url_host_path: "https://www.dola.com/chat".into(),
            net_error: "net::ERR_PROXY_CONNECTION_FAILED".into(),
        };
        let mut steps = vec![FakeStep::Launch(Err(nav()))];
        steps.extend(happy("b", "13"));
        let f = fx(&["a", "b"], steps).await;
        f.task("t1", &scene()).await;
        let out = f.run("t1", &scene(), None).await.expect("b phai chay");
        assert_eq!(out.account, "b");
        assert_eq!(f.calls().len(), 4);
        assert!(
            f.account("a").await.cooldown_until.abs() < f64::EPSILON,
            "proxy chet khong cooldown"
        );

        let f = fx(
            &["a", "b"],
            vec![FakeStep::Launch(Err(nav())), FakeStep::Launch(Err(nav()))],
        )
        .await;
        f.task("t1", &scene()).await;
        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(
            matches!(&e, EngineError::ProxyDown { account, .. } if account == "b"),
            "{e:?}"
        );
        let row = f.row("t1").await;
        assert_eq!(row.failure_code, Some(FailureCode::ProxyDown));
        assert!(
            row.error
                .as_deref()
                .unwrap_or("")
                .starts_with("PROXY_DOWN: b: "),
            "{:?}",
            row.error
        );
    }

    #[tokio::test]
    async fn tat_ca_ban_doi_0_5s_roi_chay() {
        // browser_pool.py:1262-1267 (dot 11, ADR-016): moi tai khoan con luot deu dang ban ->
        // DOI 0.5 s roi thu lai (khong fail ngay).
        let f = fx_with(&["a", "b"], happy("a", "14"), Limits::default(), |cfg| {
            cfg.busy_wait_max = Duration::from_secs(5);
        })
        .await;
        f.task("t1", &scene()).await;
        let la = f.engine.pool().try_lease("a", "").unwrap();
        let lb = f.engine.pool().try_lease("b", "").unwrap();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(700)).await;
            drop(la);
            drop(lb);
        });

        let t0 = Instant::now();
        let out = f
            .run("t1", &scene(), None)
            .await
            .expect("phai chay sau khi ranh");
        assert!(
            t0.elapsed() >= Duration::from_millis(600),
            "phai doi ({:?})",
            t0.elapsed()
        );
        assert_eq!(out.account, "a");
        assert_eq!(f.calls().len(), 3);
    }

    #[tokio::test]
    async fn tat_ca_ban_qua_busy_wait_max_bao_profile_busy() {
        // browser_pool.py:1268-1269: qua BUSY_WAIT_MAX -> "all schedulable accounts busy" ->
        // o day NoFreeProfile (ma PROFILE_BUSY), khong mo trinh duyet nao.
        let f = fx_with(&["a", "b"], vec![], Limits::default(), |cfg| {
            cfg.busy_wait_max = Duration::from_millis(200);
        })
        .await;
        f.task("t1", &scene()).await;
        let _la = f.engine.pool().try_lease("a", "").unwrap();
        let _lb = f.engine.pool().try_lease("b", "").unwrap();

        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(matches!(e, EngineError::NoFreeProfile), "{e:?}");
        assert!(f.calls().is_empty());
        let row = f.row("t1").await;
        assert_eq!(row.status, TaskStatus::Failed);
        assert_eq!(row.failure_code, Some(FailureCode::ProfileBusy));
    }

    #[tokio::test]
    async fn mode_fallback_tat_bao_429_dung_mode_goc() {
        // server.py:680 MODE_FALLBACK=0 -> chain = [mode]; mode/duration trong DB khong doi.
        let f = fx_with(
            &["a"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Err(DolaError::AccountLimited("x".into()))),
            ],
            Limits::default(),
            |cfg| cfg.mode_fallback = false,
        )
        .await;
        f.task("t1", &scene()).await;
        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(
            matches!(e, EngineError::AllAccountsLimited { mode: Mode::Fast }),
            "{e:?}"
        );
        let row = f.row("t1").await;
        assert_eq!(row.mode, "fast");
        assert_eq!(row.duration, 10);
        assert_eq!(row.failure_code, Some(FailureCode::RateLimited));
    }

    #[tokio::test]
    async fn khong_tai_khoan_nao_lich_duoc_bao_429() {
        // Ruling Task 17: candidates rong (khong co tai khoan / deu tat lich) -> AllAccountsLimited
        // (Python :1294 RuntimeError "No available accounts" khong ma).
        let f = fx(&["a"], vec![]).await;
        f.accounts.set_scheduling("a", false).await.unwrap();
        f.task("t1", &scene()).await;
        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(matches!(e, EngineError::AllAccountsLimited { .. }), "{e:?}");
        assert!(f.calls().is_empty());
        assert_eq!(
            f.row("t1").await.failure_code,
            Some(FailureCode::RateLimited)
        );
    }

    #[tokio::test]
    async fn tat_ca_logged_out_bao_logged_out() {
        // browser_pool.py:1284-1286 AllAccountsLoggedOutError -> server.py:680-689 (het
        // REQUEUE_MAX) failure_code LOGGED_OUT. Requeue la viec cua P3; engine tra loi LoggedOut.
        let f = fx(
            &["a", "b"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Err(logged_out("a", false))),
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Err(logged_out("b", false))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;
        let e = f.run("t1", &scene(), None).await.unwrap_err();
        assert!(
            matches!(e, EngineError::Dola(DolaError::LoggedOut { .. })),
            "{e:?}"
        );
        assert_eq!(f.row("t1").await.failure_code, Some(FailureCode::LoggedOut));
        assert!(f.engine.cache().is_empty().await, "ca 2 phien da dong");
    }

    #[tokio::test]
    async fn prefer_hong_giua_chung_bao_account_unavailable() {
        // browser_pool.py:1279-1281: tai khoan chi dinh mat phien -> PreferredAccountUnavailable,
        // KHONG doi sang a (ADR-011).
        let f = fx(
            &["a", "b"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Err(logged_out("b", false))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;
        let e = f.run("t1", &scene(), Some("b")).await.unwrap_err();
        match &e {
            EngineError::PreferredUnavailable(msg) => {
                assert!(msg.starts_with("account 'b': LOGGED_OUT: b: "), "{msg}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            f.calls().len(),
            3,
            "launch b, submit b, close — khong thu a"
        );
        assert!(!f.account("b").await.login_ok);
        assert_eq!(
            f.row("t1").await.failure_code,
            Some(FailureCode::AccountUnavailable)
        );
    }

    #[tokio::test]
    async fn scene_account_la_prefer() {
        // server.py:661 prefer_account=account_pref (tu Scene.account); tham so `prefer` (batch
        // context) thang Scene.account.
        let f = fx(&["a", "b"], happy("b", "15")).await;
        let mut sc = scene();
        sc.account = "b".into();
        f.task("t1", &sc).await;
        let out = f.run("t1", &sc, None).await.unwrap();
        assert_eq!(out.account, "b");
        assert_eq!(f.calls()[0], f.launch("b"));
    }

    #[tokio::test]
    async fn tai_dung_phien_am_lan_hai_khong_launch() {
        let f = fx(
            &["a"],
            vec![
                FakeStep::Launch(Ok(())),
                FakeStep::Submit(Ok(conv("16"))),
                FakeStep::Poll(Ok(artifact("a", "16"))),
                FakeStep::Submit(Ok(conv("17"))),
                FakeStep::Poll(Ok(artifact("a", "17"))),
            ],
        )
        .await;
        f.task("t1", &scene()).await;
        f.task("t2", &scene()).await;
        f.run("t1", &scene(), None).await.unwrap();
        f.run("t2", &scene(), None).await.unwrap();
        assert_eq!(
            f.calls(),
            vec![
                f.launch("a"),
                submit("a", Mode::Fast, 10),
                poll("a", "16"),
                submit("a", Mode::Fast, 10),
                poll("a", "17"),
            ]
        );
        assert_eq!(f.used("a", Mode::Fast).await, 2);
    }

    #[tokio::test]
    async fn shutdown_dong_moi_phien_am() {
        let f = fx(&["a"], happy("a", "18")).await;
        f.task("t1", &scene()).await;
        f.run("t1", &scene(), None).await.unwrap();
        assert_eq!(f.engine.cache().len().await, 1);
        f.engine.shutdown().await;
        assert!(f.engine.cache().is_empty().await);
        assert_eq!(f.calls().last(), Some(&FakeCall::Close));
        // goi lai vo hai
        f.engine.shutdown().await;
    }

    #[tokio::test]
    async fn refresh_account_count_dem_scheduling() {
        let f = fx(&["a", "b"], vec![]).await;
        // luc tao: 2 tai khoan bat lich, threads=4 -> effective 2
        assert_eq!(f.engine.limiter().effective(), 2);
        f.accounts.set_scheduling("b", false).await.unwrap();
        assert_eq!(f.engine.refresh_account_count().await.unwrap(), 1);
        assert_eq!(f.engine.limiter().effective(), 1);
        f.accounts.set_scheduling("b", true).await.unwrap();
        f.accounts.upsert("c").await.unwrap();
        assert_eq!(f.engine.refresh_account_count().await.unwrap(), 3);
        assert_eq!(f.engine.limiter().effective(), 3);
    }

    #[tokio::test]
    async fn browser_khong_hop_le_bo_qua_tai_khoan() {
        // Cot `browser` khong parse duoc (BrowserKind::parse loi) -> warn + xoay (ruling).
        let f = fx(&["a", "b"], happy("b", "19")).await;
        f.accounts.set_browser("a", "netscape").await.unwrap();
        f.task("t1", &scene()).await;
        let out = f.run("t1", &scene(), None).await.unwrap();
        assert_eq!(out.account, "b");
        assert_eq!(f.calls()[0], f.launch("b"));
    }

    #[tokio::test]
    async fn limiter_threads_1_hai_job_chay_tuan_tu() {
        // browser_pool.py:1105 slot giu suot job: threads=1 -> job 2 doi job 1 xong; pick_account
        // (server.py:633, TRUOC slot) cho 2 job 2 diem bat dau khac nhau (PICK_RESERVE).
        let mut steps = happy("a", "20");
        steps.extend(happy("b", "21"));
        let db = Db::open_memory().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let accounts_dir = dir.path().join("accounts");
        let accounts = AccountRepo::new(&db);
        for n in ["a", "b"] {
            std::fs::create_dir_all(accounts_dir.join(n)).unwrap();
            accounts.upsert(n).await.unwrap();
        }
        let driver = Arc::new(FakeDriver::new(steps));
        let cfg = EngineConfig::new(&accounts_dir, dir.path().join("downloads"));
        let (engine, _h) = Engine::new(Arc::clone(&driver), db.clone(), Limits::default(), cfg, 1)
            .await
            .unwrap();
        let engine = Arc::new(engine);
        let tasks = TaskRepo::new(&db);
        tasks.create("t1", &scene(), None).await.unwrap();
        tasks.create("t2", &scene(), None).await.unwrap();
        assert_eq!(engine.limiter().effective(), 1);

        let e1 = Arc::clone(&engine);
        let j1 =
            tokio::spawn(async move { e1.generate_video("t1", &scene(), None, &NoopHooks).await });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let e2 = Arc::clone(&engine);
        let j2 =
            tokio::spawn(async move { e2.generate_video("t2", &scene(), None, &NoopHooks).await });
        let o1 = j1.await.unwrap().unwrap();
        let o2 = j2.await.unwrap().unwrap();
        assert_eq!(o1.account, "a");
        assert_eq!(
            o2.account, "b",
            "job 2 bat dau tu tai khoan khac (PICK_RESERVE)"
        );
        let calls = driver.calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec![
                FakeCall::Launch {
                    profile_dir: accounts_dir.join("a"),
                    browser: BrowserKind::Chromium,
                },
                submit("a", Mode::Fast, 10),
                poll("a", "20"),
                FakeCall::Launch {
                    profile_dir: accounts_dir.join("b"),
                    browser: BrowserKind::Chromium,
                },
                submit("b", Mode::Fast, 10),
                poll("b", "21"),
            ],
            "tuan tu, khong xen ke"
        );
        assert_eq!(engine.limiter().active(), 0, "slot da tra");
    }

    #[test]
    fn engine_config_mac_dinh() {
        let cfg = EngineConfig::new("accounts", "downloads");
        assert_eq!(cfg.accounts_dir, PathBuf::from("accounts"));
        assert_eq!(cfg.downloads_dir, PathBuf::from("downloads"));
        assert!(cfg.executable_override.is_none());
        assert_eq!(cfg.global_proxy, "");
        assert_eq!(cfg.warm_idle, Duration::from_secs(900));
        assert_eq!(cfg.busy_wait_max, Duration::from_secs(600));
        assert!(cfg.mode_fallback);
        assert_eq!(cfg.logs_dir, PathBuf::from("logs"));
    }

    #[test]
    fn now_unix_hop_ly() {
        let t = now_unix();
        // sau 2026-01-01 va truoc 2100
        assert!(t > 1_767_225_600.0 && t < 4_102_444_800.0, "{t}");
    }

    #[test]
    fn engine_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Engine<FakeDriver>>();
        assert_send_sync::<GenerateOutcome>();
    }
}
