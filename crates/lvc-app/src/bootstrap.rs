//! Khoi dong tang duoi TRUOC khi Tauri quan ly state (P3 Task 8). [`build_engine`] la ham THUAN
//! async, generic tren `D: BrowserDriver` — test duoc bang `lvc_browser::FakeDriver` + thu muc tam
//! (khong can Chrome that, khong can runtime Tauri). [`run_bootstrap`] la lop mong goi no voi
//! [`AppDriver`] that (kieu cfg-switch theo feature `fake-driver`, xem `state.rs`) + `AppHandle`,
//! roi `app.manage(AppState)` — khong test duoc o day.
//!
//! Task 12a (P3, dev khong ton credit): [`make_driver`] khoi tao [`AppDriver`] — ban thuong
//! (`ChromiumDriver`, TON CREDIT that) hoac, khi crate nay build voi
//! `cargo tauri dev --features fake-driver`, `lvc_browser::fake::FakeDriver::endless_ok` (kich ban
//! "moi submit Ok, poll Ok sau [`FAKE_DRIVER_POLL_DELAY`]"). `run_bootstrap` log mot dong
//! `tracing::warn!` THAT to khi feature bat, de KHONG ai nham tuong dang goi Dola that.

use std::path::Path;
use std::sync::Arc;

use dashmap::{DashMap, DashSet};
use lvc_browser::BrowserDriver;
#[cfg(not(feature = "fake-driver"))]
use lvc_browser::chromium::ChromiumDriver;
use lvc_core::{AppSettings, Limits};
use lvc_engine::{Engine, EngineConfig, EngineError, LimiterHandle, RequeueReport, now_unix};
use lvc_store::{Db, SettingsRepo, StoreError};
use tauri::Manager;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::paths::{db_path, downloads_dir, logs_dir};
use crate::state::{AppDriver, AppState};

/// Task 12a: `poll_and_download` cua che do gia lap (feature `fake-driver`) "cho" chung nay
/// truoc khi tra video gia — khop kich ban ruling ("poll Ok sau 3 s"). Ban that (`ChromiumDriver`)
/// khong dung hang nay.
#[cfg(feature = "fake-driver")]
const FAKE_DRIVER_POLL_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

/// Task 12a: khoi tao [`AppDriver`] that su — cfg-switch theo feature `fake-driver` cua crate
/// nay (bien dich, KHONG phai co/tat luc chay). Xem doc dau module.
#[cfg(feature = "fake-driver")]
fn make_driver() -> AppDriver {
    lvc_browser::fake::FakeDriver::endless_ok(FAKE_DRIVER_POLL_DELAY)
}

/// Xem doc ban `#[cfg(feature = "fake-driver")]` o tren — day la nhanh mac dinh (ban that).
#[cfg(not(feature = "fake-driver"))]
fn make_driver() -> AppDriver {
    ChromiumDriver
}

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
    // P4a (ADR-R014): composition root — dang ky moi node module.
    engine.set_registry(Arc::new(crate::registry::build_registry()));

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
/// [`AppDriver`] tu [`make_driver`] (that hoac gia lap tuy feature `fake-driver`) va thu muc du
/// lieu Tauri, log canh bao cai dat/ket qua requeue, roi `app.manage(AppState { .. })`. Goi tu
/// `lib.rs` trong `.setup()`.
///
/// # Errors
/// Xem [`RunBootstrapError`].
pub async fn run_bootstrap(app: &tauri::AppHandle) -> Result<(), RunBootstrapError> {
    let base = crate::paths::app_data_dir(app)?;
    // Task 12a: canh bao THAT TO ngay dau log khi dang o che do gia lap — de khong ai nham
    // tuong ung dung dang goi Dola that (dev, khong ton credit).
    #[cfg(feature = "fake-driver")]
    tracing::warn!("CHE DO GIA LAP — FakeDriver, khong goi Dola");
    let driver = Arc::new(make_driver());
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
        runs_failed = requeue.runs_failed.len(),
        "requeue_on_start hoan tat"
    );

    let vault = Arc::new(lvc_vault::Vault::new(&db, lvc_vault::default_box()));
    // P4d (ADR-R014): cap nguon API key cho dich vu sinh text cua engine (node script_gen) — engine
    // resolve TEN credential -> key qua Vault. Bi mat KHONG bao gio ra event/log/UI.
    engine.set_text_secret_resolver(Arc::new(crate::textgen::VaultTextResolver::new(
        Arc::clone(&vault),
    )));
    app.manage(AppState {
        engine,
        vault,
        handle,
        db,
        settings: RwLock::new(settings),
        runs: Arc::new(DashMap::new()),
        resuming: Arc::new(DashSet::new()),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeDriver;
    use lvc_core::{FailureCode, RawScene, Scene, TaskStatus};
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

    /// Fix-round 1 (final-review C1 + I6): `build_engine` phai tu `requeue_on_start` that su khi DB
    /// da co task orphan tu truoc — khong chi `lvc-engine` tu no. Seed 2 task `processing`: `a`
    /// KHONG `conversation_id`, `b` CO `conversation_id` — ca hai gio deu thanh `Failed`/`RESTART`
    /// (P3 khong co bo tieu thu `queued`), chi khac o cho `b` GIU `conversation_id`/`account` de co
    /// the "Tiep tuc" — truoc khi goi `build_engine` lan hai, mo phong app bi tat/crash giua chung
    /// roi khoi dong lai.
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
        tasks.set_account("b", "acc1").await.unwrap();
        tasks.set_conversation("b", "42").await.unwrap();

        drop(boot_setup);

        let driver = Arc::new(FakeDriver::new(vec![]));
        let boot = build_engine(dir.path(), driver).await.unwrap();

        assert_eq!(boot.requeue.requeued, vec!["a".to_string()]);
        assert_eq!(boot.requeue.resumable, vec!["b".to_string()]);
        assert!(
            boot.requeue.runs_failed.is_empty(),
            "khong co run nao duoc tao trong test nay"
        );

        let row_a = TaskRepo::new(&boot.db).get("a").await.unwrap().unwrap();
        assert_eq!(
            row_a.status,
            TaskStatus::Failed,
            "P3 khong co bo tieu thu queued -> a cung thanh Failed/RESTART"
        );
        assert_eq!(row_a.failure_code, Some(FailureCode::Restart));

        let row_b = TaskRepo::new(&boot.db).get("b").await.unwrap().unwrap();
        assert_eq!(row_b.status, TaskStatus::Failed);
        assert_eq!(row_b.failure_code, Some(FailureCode::Restart));
        assert_eq!(
            row_b.conversation_id.as_deref(),
            Some("42"),
            "giu de Tiep tuc"
        );
        assert_eq!(row_b.account.as_deref(), Some("acc1"), "giu tai khoan");
    }
}
