//! Task 18: bo dieu khien MOT video that (`examples/gen_one.rs` la `main` mong goi [`run`] voi
//! `ChromiumDriver`; test o day goi cung [`run`] voi `FakeDriver` — khong mo Chrome).
//!
//! Luong: [`Args::parse_from`] (thuan) → [`run`]: mo/migrate DB (`Db::open`) → dong tai khoan chua co
//! → `AccountRepo::upsert` + `set_browser("chromium")` (DB moi + thu muc profile copy san van chay)
//! → `Engine::new` → `TaskRepo::create` → `Engine::generate_video` (prefer = `--account`, hook do
//! thoi gian tung giai doan qua `on_stage`) → doc lai dong task → [`Summary`] (JSON ra stdout) →
//! `Engine::shutdown` (timeout tong [`SHUTDOWN_TIMEOUT`]).
//!
//! **Khong in prompt** (chi so ky tu), **khong in URL co chu ky** (`video_url_host` =
//! `url_host_path`), khong cookie. `--headed` giu de khop brief nhung khong co tac dung: engine luon
//! mo headed (rang buoc P2 — Dola soi headless).

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use lvc_browser::dola::PollHooks;
use lvc_browser::dola::poll::{STAGE_DOWNLOADING, STAGE_GENERATING};
use lvc_browser::{BrowserDriver, url_host_path};
use lvc_core::{FailureCode, Limits, RawScene, Scene, SceneError};
use lvc_store::{AccountRepo, Db, TaskRepo};
use serde::Serialize;
use tracing::{info, warn};

use crate::{Engine, EngineConfig, EngineError, now_unix};

/// Cot `browser` ghi cho dong tai khoan tu tao (profile copy tu ban Python = Chromium patchright).
pub const DEFAULT_BROWSER: &str = "chromium";
/// Timeout tong cho `Engine::shutdown` (Chromium `CLOSE_WAIT` 30 s/phien).
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(60);

pub const USAGE: &str = "cach dung: gen_one --account <ten> --prompt \"...\" [--mode fast|pro|pro_max|agent] \
[--duration 10] [--ratio 9:16] [--model seedance-2.0] [--ref <path>]... [--headed] [--db lvc.db] \
[--accounts-dir accounts] [--downloads-dir downloads] [--logs-dir logs] [--threads 1]\n\
bien moi truong LVC_CHROME = duong dan chrome.exe (ghi de resolver). CHAY THAT: ton credit Dola.";

/// Tham so dong lenh (brief Task 18). Chuoi `mode`/`duration`/`ratio`/`model` giu tho, kiem qua
/// [`Args::scene`] (`Scene::normalize`, thong diep loi giong Python).
#[derive(Clone, PartialEq, Eq)]
pub struct Args {
    pub account: String,
    pub prompt: String,
    pub mode: String,
    pub duration: String,
    pub ratio: String,
    pub model: String,
    pub refs: Vec<PathBuf>,
    /// Khong tac dung (engine luon headed) — giu de khop brief.
    pub headed: bool,
    pub db: PathBuf,
    pub accounts_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub threads: usize,
    /// `LVC_CHROME` — dat qua [`Args::with_env`] (parse thuan khong doc env).
    pub executable_override: Option<PathBuf>,
}

impl fmt::Debug for Args {
    /// Chinh sach log: KHONG in prompt.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Args")
            .field("account", &self.account)
            .field("prompt_chars", &self.prompt.chars().count())
            .field("mode", &self.mode)
            .field("duration", &self.duration)
            .field("ratio", &self.ratio)
            .field("model", &self.model)
            .field("refs", &self.refs)
            .field("headed", &self.headed)
            .field("db", &self.db)
            .field("accounts_dir", &self.accounts_dir)
            .field("downloads_dir", &self.downloads_dir)
            .field("logs_dir", &self.logs_dir)
            .field("threads", &self.threads)
            .field("executable_override", &self.executable_override)
            .finish()
    }
}

impl Args {
    /// Phan tich `argv` (KHONG gom ten chuong trinh). Thuan — khong doc env/dia.
    ///
    /// # Errors
    /// Chuoi thong diep tieng Viet: thieu `--account`/`--prompt`, co `--x` la, thieu gia tri,
    /// `--threads` khong phai so.
    pub fn parse_from<I, S>(argv: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut out = Self {
            account: String::new(),
            prompt: String::new(),
            mode: "fast".into(),
            duration: "10".into(),
            ratio: "9:16".into(),
            model: "seedance-2.0".into(),
            refs: Vec::new(),
            headed: false,
            db: PathBuf::from("lvc.db"),
            accounts_dir: PathBuf::from("accounts"),
            downloads_dir: PathBuf::from("downloads"),
            logs_dir: PathBuf::from("logs"),
            threads: 1,
            executable_override: None,
        };
        let mut it = argv.into_iter().map(Into::into);
        while let Some(flag) = it.next() {
            if flag == "--headed" {
                out.headed = true;
                continue;
            }
            let mut value = || {
                it.next()
                    .ok_or_else(|| format!("thieu gia tri sau {flag}\n{USAGE}"))
            };
            match flag.as_str() {
                "--account" => out.account = value()?,
                "--prompt" => out.prompt = value()?,
                "--mode" => out.mode = value()?,
                "--duration" => out.duration = value()?,
                "--ratio" => out.ratio = value()?,
                "--model" => out.model = value()?,
                "--ref" => out.refs.push(PathBuf::from(value()?)),
                "--db" => out.db = PathBuf::from(value()?),
                "--accounts-dir" => out.accounts_dir = PathBuf::from(value()?),
                "--downloads-dir" => out.downloads_dir = PathBuf::from(value()?),
                "--logs-dir" => out.logs_dir = PathBuf::from(value()?),
                "--threads" => {
                    let v = value()?;
                    out.threads = v
                        .parse()
                        .map_err(|_| format!("--threads phai la so nguyen duong, nhan {v:?}"))?;
                }
                other => return Err(format!("tham so la: {other}\n{USAGE}")),
            }
        }
        if out.account.trim().is_empty() {
            return Err(format!("thieu --account\n{USAGE}"));
        }
        if out.prompt.trim().is_empty() {
            return Err(format!("thieu --prompt\n{USAGE}"));
        }
        Ok(out)
    }

    /// Doc `LVC_CHROME` → `executable_override` (tach khoi `parse_from` de parse thuan).
    #[must_use]
    pub fn with_env(mut self) -> Self {
        self.executable_override = std::env::var_os("LVC_CHROME").map(PathBuf::from);
        self
    }

    /// `Scene::normalize` tu tham so; `account` = `--account` (prefer, ADR-011).
    ///
    /// # Errors
    /// `SceneError` (thong diep giong Python: mode/ratio/duration/model sai, prompt qua dai...).
    pub fn scene(&self, limits: &Limits) -> Result<Scene, SceneError> {
        let raw = RawScene {
            prompt: Some(self.prompt.clone()),
            ratio: Some(self.ratio.clone()),
            duration: Some(self.duration.clone()),
            model: Some(self.model.clone()),
            mode: Some(self.mode.clone()),
            logo: None,
            reference_images: Some(self.refs.iter().map(|p| p.display().to_string()).collect()),
            account: Some(self.account.clone()),
        };
        Scene::normalize(&raw, limits, None)
    }
}

/// Tong ket in ra stdout (JSON). Moi truong `Option` = `null` khi khong co (that bai som).
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub task_id: String,
    pub account: Option<String>,
    /// Mode THUC TE trong dong task (co the doi vi het luot).
    pub mode: Option<String>,
    pub conversation_id: Option<String>,
    /// `url_host_path(video_url)` — KHONG mang query/chu ky.
    pub video_url_host: Option<String>,
    pub local_path: Option<String>,
    pub bytes: Option<u64>,
    pub watermarked: Option<bool>,
    pub ms_total: u64,
    /// Tu luc bat dau (ke ca cho slot + mo Chrome) den `on_stage("generating")`.
    pub ms_submit: Option<u64>,
    /// `generating` → `downloading`.
    pub ms_poll: Option<u64>,
    /// `downloading` → xong.
    pub ms_download: Option<u64>,
    pub failure_code: Option<String>,
    pub error: Option<String>,
}

impl Summary {
    /// JSON nhieu dong (serde khong the loi voi struct nay — `unwrap_or_default` chi la phong thu).
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Hook do thoi gian: ghi MOC DAU TIEN cua moi stage (`generating` = gui xong, `downloading` = co
/// video). `on_poll` → log `info` moi 30 s; `on_agent_reply` → chi do dai.
pub struct TimingHooks {
    start: Instant,
    marks: Mutex<HashMap<String, Instant>>,
}

impl Default for TimingHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl TimingHooks {
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            marks: Mutex::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    fn mark(&self, stage: &str) -> Option<Duration> {
        self.marks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(stage)
            .map(|t| t.duration_since(self.start))
    }

    /// `(ms_submit, ms_poll, ms_download)` tai `total` (thoi gian tu `start` den luc ket thuc).
    #[must_use]
    pub fn phases(&self, total: Duration) -> (Option<u64>, Option<u64>, Option<u64>) {
        let ms = |d: Duration| u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
        let generating = self.mark(STAGE_GENERATING);
        let dl = self.mark(STAGE_DOWNLOADING);
        let submit = generating.map(ms);
        let poll = match (generating, dl) {
            (Some(g), Some(d)) => Some(ms(d.saturating_sub(g))),
            _ => None,
        };
        let download = dl.map(|d| ms(total.saturating_sub(d)));
        (submit, poll, download)
    }
}

impl PollHooks for TimingHooks {
    fn on_poll(&self, elapsed: Duration) {
        info!(elapsed_s = elapsed.as_secs(), "dang cho video");
    }
    fn on_stage(&self, stage: &str) {
        info!(stage, elapsed_s = self.elapsed().as_secs(), "stage");
        self.marks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(stage.to_string())
            .or_insert_with(Instant::now);
    }
    fn on_agent_reply(&self, text: &str) {
        info!(chars = text.chars().count(), "agent tra loi");
    }
}

/// Loi cua CLI: tham so/Scene sai hoac loi engine/DB. Loi TAO VIDEO khong nam o day — no nam trong
/// [`Summary::error`] (`run` van `Ok`).
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Scene(#[from] SceneError),
    #[error(transparent)]
    Engine(#[from] EngineError),
}

impl From<lvc_store::StoreError> for CliError {
    fn from(e: lvc_store::StoreError) -> Self {
        Self::Engine(EngineError::Store(e))
    }
}

/// Dong tai khoan chua co → tao + `browser = chromium` (DB moi voi thu muc profile copy san).
async fn ensure_account(db: &Db, name: &str) -> Result<(), lvc_store::StoreError> {
    let repo = AccountRepo::new(db);
    if repo.get(name).await?.is_none() {
        info!(
            account = name,
            "chua co dong tai khoan -> tao (browser=chromium)"
        );
        repo.upsert(name).await?;
        repo.set_browser(name, DEFAULT_BROWSER).await?;
    }
    Ok(())
}

fn task_id() -> String {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "unix ms > 0 va < 2^63 trong moi moc thoi gian thuc te; chi de tao id"
    )]
    let ms = (now_unix() * 1000.0) as u64;
    format!("video_{ms:x}")
}

/// Chay 1 video voi `driver` (that hoac gia). `Ok(Summary)` ke ca khi tao video that bai (xem
/// `failure_code`/`error`); `Err` chi khi tham so/Scene sai hoac DB/engine khong khoi tao duoc.
///
/// # Errors
/// [`CliError`].
pub async fn run<D: BrowserDriver + 'static>(
    args: &Args,
    driver: Arc<D>,
) -> Result<Summary, CliError> {
    let limits = Limits::default();
    let scene = args.scene(&limits)?;
    info!(
        account = %args.account,
        prompt_chars = scene.prompt.chars().count(),
        mode = scene.mode.as_str(),
        duration = scene.duration,
        refs = scene.reference_images.len(),
        db = %args.db.display(),
        "gen_one bat dau"
    );
    let db = Db::open(&args.db).await?;
    ensure_account(&db, &args.account).await?;

    let mut cfg = EngineConfig::new(&args.accounts_dir, &args.downloads_dir);
    cfg.logs_dir.clone_from(&args.logs_dir);
    cfg.executable_override
        .clone_from(&args.executable_override);
    let (engine, _handle) = Engine::new(driver, db.clone(), limits, cfg, args.threads).await?;

    let tasks = TaskRepo::new(&db);
    let id = task_id();
    tasks.create(&id, &scene, None).await?;

    let hooks = TimingHooks::new();
    let res = engine.generate_video(&id, &scene, None, &hooks).await;
    let total = hooks.elapsed();
    let (ms_submit, ms_poll, ms_download) = hooks.phases(total);

    let row = tasks.get(&id).await?;
    let (artifact, err) = match res {
        Ok(out) => (Some(out.artifact), None),
        Err(e) => (None, Some(e)),
    };
    let summary = Summary {
        task_id: id,
        account: row.as_ref().and_then(|r| r.account.clone()),
        mode: row.as_ref().map(|r| r.mode.clone()),
        conversation_id: row.as_ref().and_then(|r| r.conversation_id.clone()),
        video_url_host: artifact.as_ref().map(|a| url_host_path(&a.video_url)),
        local_path: artifact
            .as_ref()
            .map(|a| a.local_path.display().to_string()),
        bytes: artifact.as_ref().map(|a| a.bytes),
        watermarked: artifact.as_ref().map(|a| a.watermarked),
        ms_total: u64::try_from(total.as_millis()).unwrap_or(u64::MAX),
        ms_submit,
        ms_poll,
        ms_download,
        failure_code: err
            .as_ref()
            .and_then(EngineError::failure_code)
            .map(FailureCode::as_str)
            .map(str::to_string),
        error: err.as_ref().map(ToString::to_string),
    };

    if tokio::time::timeout(SHUTDOWN_TIMEOUT, engine.shutdown())
        .await
        .is_err()
    {
        warn!(
            secs = SHUTDOWN_TIMEOUT.as_secs(),
            "shutdown qua han — Chrome se do Job Object dong"
        );
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use lvc_browser::dola::{ConversationId, VideoArtifact, poll::NOTE_RAW};
    use lvc_browser::{FakeCall, FakeDriver, FakeStep};
    use lvc_core::{Mode, TaskStatus};
    use lvc_store::{AccountRepo, Db, TaskRepo};

    use super::*;

    fn argv(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parse_toi_thieu_va_mac_dinh() {
        let a = Args::parse_from(argv(&["--account", "acc1", "--prompt", "con meo"])).unwrap();
        assert_eq!(a.account, "acc1");
        assert_eq!(a.prompt, "con meo");
        assert_eq!(a.mode, "fast");
        assert_eq!(a.duration, "10");
        assert_eq!(a.ratio, "9:16");
        assert_eq!(a.model, "seedance-2.0");
        assert!(a.refs.is_empty());
        assert!(!a.headed);
        assert_eq!(a.db, PathBuf::from("lvc.db"));
        assert_eq!(a.accounts_dir, PathBuf::from("accounts"));
        assert_eq!(a.downloads_dir, PathBuf::from("downloads"));
        assert_eq!(a.logs_dir, PathBuf::from("logs"));
        assert_eq!(a.threads, 1);
        assert!(a.executable_override.is_none());
        // Scene: prefer = account, cac truong da chuan hoa
        let sc = a.scene(&Limits::default()).unwrap();
        assert_eq!(sc.account, "acc1");
        assert_eq!(sc.mode, Mode::Fast);
        assert_eq!(sc.duration, 10);
        assert_eq!(sc.ratio, lvc_core::Ratio::R9x16);
        assert_eq!(sc.model, lvc_core::Model::Seedance20);
    }

    #[test]
    fn parse_day_du_va_ref_lap_lai() {
        let a = Args::parse_from(argv(&[
            "--account",
            "acc2",
            "--prompt",
            "p",
            "--mode",
            "pro",
            "--duration",
            "15",
            "--ratio",
            "16:9",
            "--model",
            "seedance-2.5",
            "--ref",
            "a.png",
            "--ref",
            "b.jpg",
            "--headed",
            "--db",
            r"D:\x\lvc.db",
            "--accounts-dir",
            "acc",
            "--downloads-dir",
            "dl",
            "--logs-dir",
            "lg",
            "--threads",
            "3",
        ]))
        .unwrap();
        assert_eq!(a.mode, "pro");
        assert_eq!(a.duration, "15");
        assert_eq!(a.ratio, "16:9");
        assert_eq!(a.model, "seedance-2.5");
        assert_eq!(a.refs, vec![PathBuf::from("a.png"), PathBuf::from("b.jpg")]);
        assert!(a.headed);
        assert_eq!(a.db, PathBuf::from(r"D:\x\lvc.db"));
        assert_eq!(a.accounts_dir, PathBuf::from("acc"));
        assert_eq!(a.downloads_dir, PathBuf::from("dl"));
        assert_eq!(a.logs_dir, PathBuf::from("lg"));
        assert_eq!(a.threads, 3);
        let sc = a.scene(&Limits::default()).unwrap();
        assert_eq!(sc.mode, Mode::Pro);
        assert_eq!(sc.duration, 15);
        assert_eq!(sc.reference_images, vec!["a.png", "b.jpg"]);
    }

    #[test]
    fn parse_loi_ro_rang() {
        let e = Args::parse_from(argv(&["--prompt", "p"])).unwrap_err();
        assert!(e.contains("--account"), "{e}");
        let e = Args::parse_from(argv(&["--account", "a"])).unwrap_err();
        assert!(e.contains("--prompt"), "{e}");
        let e =
            Args::parse_from(argv(&["--account", "a", "--prompt", "p", "--bogus"])).unwrap_err();
        assert!(e.contains("--bogus"), "{e}");
        let e = Args::parse_from(argv(&["--account", "a", "--prompt"])).unwrap_err();
        assert!(e.contains("--prompt"), "{e}");
        let e = Args::parse_from(argv(&["--account", "a", "--prompt", "p", "--threads", "x"]))
            .unwrap_err();
        assert!(e.contains("--threads"), "{e}");
        // mode sai -> loi luc dung Scene (thong diep Python)
        let a = Args::parse_from(argv(&[
            "--account",
            "a",
            "--prompt",
            "p",
            "--mode",
            "turbo",
        ]))
        .unwrap();
        assert!(a.scene(&Limits::default()).is_err());
    }

    /// `Debug` cua `Args` KHONG in prompt (chinh sach log) — chi so ky tu.
    #[test]
    fn debug_args_khong_lo_prompt() {
        let a = Args::parse_from(argv(&["--account", "a", "--prompt", "BI-MAT-XYZ"])).unwrap();
        let d = format!("{a:?}");
        assert!(!d.contains("BI-MAT-XYZ"), "{d}");
        assert!(d.contains("prompt_chars: 10"), "{d}");
    }

    fn artifact(account: &str, conv: &str) -> VideoArtifact {
        VideoArtifact {
            video_url: "https://v.example/x/y.mp4?sig=BIMAT".into(),
            local_path: PathBuf::from("downloads").join("a.mp4"),
            conversation_id: ConversationId(conv.into()),
            account: account.into(),
            watermarked: false,
            bytes: 12_345,
            note: NOTE_RAW,
        }
    }

    /// `run` end-to-end voi `FakeDriver`: mo DB file, tu tao dong tai khoan (browser=chromium), tao
    /// task, chay, tong ket JSON; KHONG mo Chrome, KHONG in prompt/URL ky.
    #[tokio::test]
    async fn run_voi_fake_driver_thanh_cong() {
        let dir = tempfile::tempdir().unwrap();
        let accounts_dir = dir.path().join("accounts");
        std::fs::create_dir_all(accounts_dir.join("acc1")).unwrap();
        let db_path = dir.path().join("lvc.db");
        let mut args =
            Args::parse_from(argv(&["--account", "acc1", "--prompt", "con meo BIMAT"])).unwrap();
        args.db.clone_from(&db_path);
        args.accounts_dir.clone_from(&accounts_dir);
        args.downloads_dir = dir.path().join("dl");
        args.logs_dir = dir.path().join("logs");
        let driver = Arc::new(FakeDriver::new(vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(ConversationId("777".into()))),
            FakeStep::Poll(Ok(artifact("acc1", "777"))),
        ]));

        let summary = run(&args, Arc::clone(&driver)).await.unwrap();
        assert_eq!(summary.account.as_deref(), Some("acc1"));
        assert_eq!(summary.mode.as_deref(), Some("fast"));
        assert_eq!(summary.conversation_id.as_deref(), Some("777"));
        assert_eq!(
            summary.video_url_host.as_deref(),
            Some("https://v.example/x/y.mp4"),
            "khong co query/chu ky"
        );
        assert_eq!(summary.bytes, Some(12_345));
        assert_eq!(summary.watermarked, Some(false));
        assert_eq!(summary.failure_code, None);
        assert_eq!(summary.error, None);
        assert!(summary.task_id.starts_with("video_"));
        // FakeDriver khong goi hook stage -> khong do duoc tung giai doan
        assert_eq!(summary.ms_submit, None);
        assert_eq!(summary.ms_poll, None);
        assert_eq!(summary.ms_download, None);
        let json = summary.to_json();
        assert!(json.contains("\"task_id\""), "{json}");
        assert!(!json.contains("BIMAT"), "khong in prompt/chu ky: {json}");

        // shutdown da dong phien am
        let calls = driver.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[3], FakeCall::Close);

        // DB: dong tai khoan tu tao voi browser=chromium; task Completed
        let db = Db::open(&db_path).await.unwrap();
        let acc = AccountRepo::new(&db).get("acc1").await.unwrap().unwrap();
        assert_eq!(acc.browser, "chromium");
        let row = TaskRepo::new(&db)
            .get(&summary.task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.status, TaskStatus::Completed);
        assert_eq!(row.account.as_deref(), Some("acc1"));
        assert_eq!(row.conversation_id.as_deref(), Some("777"));
    }

    /// Tai khoan khong co thu muc profile -> khong mo trinh duyet, tong ket mang `failure_code` +
    /// error (khong panic, `run` van `Ok`).
    #[tokio::test]
    async fn run_voi_fake_driver_that_bai_co_ma() {
        let dir = tempfile::tempdir().unwrap();
        let mut args = Args::parse_from(argv(&["--account", "ghost", "--prompt", "p"])).unwrap();
        args.db = dir.path().join("lvc.db");
        args.accounts_dir = dir.path().join("accounts");
        args.downloads_dir = dir.path().join("dl");
        args.logs_dir = dir.path().join("logs");
        let driver = Arc::new(FakeDriver::new(vec![]));

        let summary = run(&args, Arc::clone(&driver)).await.unwrap();
        assert_eq!(summary.failure_code.as_deref(), Some("ACCOUNT_UNAVAILABLE"));
        assert!(
            summary
                .error
                .as_deref()
                .unwrap_or("")
                .contains("does not exist"),
            "{:?}",
            summary.error
        );
        assert_eq!(summary.account, None);
        assert_eq!(summary.conversation_id, None);
        assert_eq!(summary.bytes, None);
        assert!(driver.calls.lock().unwrap().is_empty());
    }

    /// `TimingHooks`: moc "generating"/"downloading" -> `ms_submit`/`ms_poll`/`ms_download`.
    #[test]
    fn timing_hooks_tinh_tung_giai_doan() {
        let h = TimingHooks::new();
        assert_eq!(
            h.phases(std::time::Duration::from_millis(500)),
            (None, None, None)
        );
        h.on_stage("generating");
        std::thread::sleep(std::time::Duration::from_millis(5));
        h.on_stage("confirm"); // khong anh huong
        h.on_stage("downloading");
        h.on_stage("generating"); // lan sau KHONG ghi de moc dau
        let (s, p, d) = h.phases(h.elapsed() + std::time::Duration::from_millis(20));
        let (s, p, d) = (s.unwrap(), p.unwrap(), d.unwrap());
        assert!(p >= 5, "poll >= 5 ms: {p}");
        assert!(d >= 20, "download >= 20 ms: {d}");
        assert!(s < 1000, "{s}");
    }
}
