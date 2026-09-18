//! Khoi dong tang duoi TRUOC khi Tauri quan ly state (P3 Task 8). [`build_engine`] la ham THUAN
//! async, generic tren `D: BrowserDriver` — test duoc bang `lvc_browser::FakeDriver` + thu muc tam
//! (khong can Chrome that, khong can runtime Tauri). [`run_bootstrap`] la lop mong goi no voi
//! `ChromiumDriver` that + `AppHandle`, roi `app.manage(AppState)` — khong test duoc o day.

use std::path::Path;
use std::sync::Arc;

use dashmap::DashMap;
use lvc_browser::BrowserDriver;
use lvc_browser::chromium::ChromiumDriver;
use lvc_core::{AppSettings, Limits};
use lvc_engine::{Engine, EngineConfig, EngineError, LimiterHandle, RequeueReport, now_unix};
use lvc_store::{Db, SettingsRepo, StoreError};
use tauri::Manager;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::paths::{db_path, downloads_dir, logs_dir};
use crate::state::AppState;

/// Loi khoi dong tang duoi (`build_engine`).
#[derive(Debug, thiserror::Error)]
pub enum BootError {
    /// Tao thu muc (`logs/`/`downloads/`/goc) that bai.
    #[error("khong tao duoc thu muc du lieu ung dung: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Engine(#[from] EngineError),
}

/// Ket qua [`build_engine`] — du moi thu de `run_bootstrap` dung `app.manage(AppState { .. })`.
pub struct Bootstrapped<D: BrowserDriver + 'static> {
    pub engine: Arc<Engine<D>>,
    pub handle: LimiterHandle,
    pub db: Db,
    pub settings: AppSettings,
    /// Canh bao tu `AppSettings::from_map` (khoa co gia tri sai, da roi ve mac dinh) — KHONG phai
    /// loi (engine van chay duoc voi mac dinh), Task 9 hien cho nguoi dung biet.
    pub warnings: Vec<String>,
    pub requeue: RequeueReport,
}

/// Khoi tao tang duoi tai `base` (Tauri `app_data_dir`):
/// 1. Tao `logs/`/`downloads/` (idempotent — goi lai lan hai khong loi, khong xoa du lieu cu).
/// 2. Mo + migrate DB tai `base/lvc.db` (`Db::open` da chay migration ben trong).
/// 3. Doc bang `settings` (`SettingsRepo::get_all`) -> `AppSettings::from_map` (mac dinh + canh
///    bao khi khoa nao do sai, khong panic).
/// 4. Tao `Engine` (`Limits::default()`, `threads` tu cai dat).
/// 5. `requeue_on_start` — chia orphan (task `processing` khi tat/crash) thanh requeue-ngay vs
///    ung-vien-resume; ham nay KHONG tu goi `Engine::resume` (Task 7/9 quyet dinh).
///
/// # Errors
/// [`BootError::Io`] khi khong tao duoc thu muc; [`BootError::Store`] khi mo/migrate DB hoac doc
/// cai dat loi; [`BootError::Engine`] khi `Engine::new`/`requeue_on_start` loi.
pub async fn build_engine<D: BrowserDriver + 'static>(
    base: &Path,
    driver: Arc<D>,
) -> Result<Bootstrapped<D>, BootError> {
    std::fs::create_dir_all(base)?;
    std::fs::create_dir_all(logs_dir(base))?;
    std::fs::create_dir_all(downloads_dir(base))?;

    let db = Db::open(&db_path(base)).await?;

    let map = SettingsRepo::new(&db).get_all().await?;
    let (settings, warnings) = AppSettings::from_map(&map);

    let cfg = EngineConfig::from_settings(&settings, base);
    let (engine, handle) =
        Engine::new(driver, db.clone(), Limits::default(), cfg, settings.threads).await?;
    let engine = Arc::new(engine);

    let requeue = engine.requeue_on_start(now_unix()).await?;

    Ok(Bootstrapped {
        engine,
        handle,
        db,
        settings,
        warnings,
        requeue,
    })
}

/// Loi cua [`run_bootstrap`] — boc [`BootError`] hoac loi Tauri (khong xac dinh duoc
/// `app_data_dir`).
#[derive(Debug, thiserror::Error)]
pub enum RunBootstrapError {
    #[error(transparent)]
    Boot(#[from] BootError),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

/// Lop mong (khong test duoc o day — can `AppHandle` that): goi [`build_engine`] voi
/// `ChromiumDriver` that va thu muc du lieu Tauri, log canh bao cai dat/ket qua requeue, roi
/// `app.manage(AppState { .. })`. Goi tu `lib.rs` trong `.setup()`.
///
/// # Errors
/// Xem [`RunBootstrapError`].
pub async fn run_bootstrap(app: &tauri::AppHandle) -> Result<(), RunBootstrapError> {
    let base = crate::paths::app_data_dir(app)?;
    let driver = Arc::new(ChromiumDriver);
    let Bootstrapped {
        engine,
        handle,
        db,
        settings,
        warnings,
        requeue,
    } = build_engine(&base, driver).await?;

    for w in &warnings {
        warn!(warning = %w, "cai dat: gia tri khong hop le trong bang settings, da dung mac dinh");
    }
    info!(
        requeued = requeue.requeued.len(),
        resumable = requeue.resumable.len(),
        "requeue_on_start hoan tat"
    );

    app.manage(AppState {
        engine,
        handle,
        db,
        settings: RwLock::new(settings),
        runs: DashMap::new(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeDriver;
    use lvc_core::{RawScene, Scene, TaskStatus};
    use lvc_store::TaskRepo;

    use super::*;

    /// 9:16, 10s, seedance-2.0, fast — canh toi thieu de tao task (giong `lvc_engine::generate::tests::scene`
    /// nhung `pub(crate)` o do nen phai tu dung `Scene::normalize` o day).
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

    #[tokio::test]
    async fn tao_thu_muc_db_settings_mac_dinh_va_requeue_rong() {
        let dir = tempfile::tempdir().unwrap();
        let driver = Arc::new(FakeDriver::new(vec![]));

        let boot = build_engine(dir.path(), driver).await.unwrap();

        assert!(dir.path().join("lvc.db").is_file(), "phai tao file DB");
        assert!(dir.path().join("logs").is_dir(), "phai tao logs/");
        assert!(dir.path().join("downloads").is_dir(), "phai tao downloads/");
        assert_eq!(
            boot.settings,
            AppSettings::default(),
            "bang settings rong -> mac dinh"
        );
        assert!(boot.warnings.is_empty());
        assert!(boot.requeue.requeued.is_empty());
        assert!(boot.requeue.resumable.is_empty());
        assert_eq!(
            boot.engine.limiter().effective(),
            1,
            "chua co tai khoan nao -> kep toi thieu 1 (DynamicLimiter::effective)"
        );
    }

    /// Goi lan hai tren CUNG thu muc: khong loi (dir/DB da co), van doc lai dung cai dat.
    #[tokio::test]
    async fn goi_lan_hai_tai_su_dung_db_va_thu_muc_da_co() {
        let dir = tempfile::tempdir().unwrap();

        let driver1 = Arc::new(FakeDriver::new(vec![]));
        let boot1 = build_engine(dir.path(), driver1).await.unwrap();
        SettingsRepo::new(&boot1.db)
            .set("threads", "5")
            .await
            .unwrap();
        drop(boot1);

        let driver2 = Arc::new(FakeDriver::new(vec![]));
        let boot2 = build_engine(dir.path(), driver2).await.unwrap();
        assert_eq!(boot2.settings.threads, 5, "phai doc lai cai dat da luu");
        assert!(boot2.warnings.is_empty());
    }

    /// Khoa cai dat sai gia tri -> mac dinh + canh bao, KHONG loi ca ham.
    #[tokio::test]
    async fn cai_dat_sai_ve_mac_dinh_kem_canh_bao() {
        let dir = tempfile::tempdir().unwrap();
        let driver = Arc::new(FakeDriver::new(vec![]));
        let boot_setup = build_engine(dir.path(), Arc::clone(&driver)).await.unwrap();
        SettingsRepo::new(&boot_setup.db)
            .set("threads", "khong-phai-so")
            .await
            .unwrap();
        drop(boot_setup);

        let boot = build_engine(dir.path(), driver).await.unwrap();
        assert_eq!(boot.settings.threads, 1, "gia tri sai -> mac dinh");
        assert_eq!(boot.warnings.len(), 1);
        assert!(boot.warnings[0].contains("threads"), "{:?}", boot.warnings);
    }

    /// I1 (fix-round review task-8): `build_engine` phai tu `requeue_on_start` that su khi DB da co
    /// task orphan tu truoc — khong chi `lvc-engine` tu no. Seed 2 task `processing`: `a` KHONG
    /// `conversation_id` (phai requeue ve `queued` ngay) va `b` CO `conversation_id` (chi liet ke
    /// resumable, KHONG bi dong) truoc khi goi `build_engine` lan hai — mo phong app bi tat/crash
    /// giua chung roi khoi dong lai.
    #[tokio::test]
    async fn build_engine_requeue_orphan_co_va_khong_conv() {
        let dir = tempfile::tempdir().unwrap();

        let driver_setup = Arc::new(FakeDriver::new(vec![]));
        let boot_setup = build_engine(dir.path(), driver_setup).await.unwrap();

        let tasks = TaskRepo::new(&boot_setup.db);
        tasks.create("a", &scene(), None).await.unwrap();
        tasks
            .set_status("a", TaskStatus::Processing, None, None)
            .await
            .unwrap();

        tasks.create("b", &scene(), None).await.unwrap();
        tasks
            .set_status("b", TaskStatus::Processing, None, None)
            .await
            .unwrap();
        tasks.set_conversation("b", "42").await.unwrap();

        drop(boot_setup);

        let driver = Arc::new(FakeDriver::new(vec![]));
        let boot = build_engine(dir.path(), driver).await.unwrap();

        assert_eq!(boot.requeue.requeued, vec!["a".to_string()]);
        assert_eq!(boot.requeue.resumable, vec!["b".to_string()]);

        let row_a = TaskRepo::new(&boot.db).get("a").await.unwrap().unwrap();
        assert_eq!(
            row_a.status,
            TaskStatus::Queued,
            "a phai ve queued sau requeue"
        );
    }
}
