//! `EngineConfig::from_settings` + `Engine::apply_settings` — cau hinh engine tu `AppSettings`
//! (Task 5, P3). Tach khoi `generate.rs` (da rat dai) theo dung yeu cau brief task-5.
//!
//! | Ham | Nguon | Ghi chu |
//! |---|---|---|
//! | [`EngineConfig::from_settings`] | moi (P3) | `accounts_dir`/`output_dir` tinh tu `app_data_dir` khi rong/tuong doi; `executable_override` doc cung bien `LVC_CHROME` nhu [`crate::cli::Args::with_env`] |
//! | [`Engine::apply_settings`] | moi (P3) | CHI `threads` co hieu luc NGAY (`LimiterHandle::set_threads`, ADR-016); `accounts_dir`/`output_dir`/`mode_fallback` nam trong `EngineConfig` bat bien — doi khong co tac dung, chi ghi `warn` de nguoi dung biet phai khoi dong lai engine |

use std::path::{Path, PathBuf};

use lvc_browser::BrowserDriver;
use lvc_core::AppSettings;
use tracing::{info, warn};

use crate::generate::{Engine, EngineConfig};

impl EngineConfig {
    /// Dung cho Tauri command (Task 9): `AppSettings::from_map(repo.get_all().await?)` roi goi ham
    /// nay voi `app_data_dir` (Tauri `AppHandle::path().app_data_dir()`).
    ///
    /// - `accounts_dir`: `s.accounts_dir` neu khong rong, khong thi `app_data_dir/accounts`.
    /// - `downloads_dir` (tu `output_dir`): tuyet doi thi giu nguyen, tuong doi thi noi vao
    ///   `app_data_dir`.
    /// - `logs_dir`: luon `app_data_dir/logs` (khong co khoa settings rieng).
    /// - `mode_fallback`: chep thang tu `s.mode_fallback`.
    /// - `executable_override`: doc `LVC_CHROME` — cung bien moi truong voi
    ///   [`crate::cli::Args::with_env`] (khong co khoa settings rieng cho binary Chrome).
    #[must_use]
    pub fn from_settings(s: &AppSettings, app_data_dir: &Path) -> Self {
        let accounts_dir = if s.accounts_dir.is_empty() {
            app_data_dir.join("accounts")
        } else {
            PathBuf::from(&s.accounts_dir)
        };
        let output_path = PathBuf::from(&s.output_dir);
        let downloads_dir = if output_path.is_absolute() {
            output_path
        } else {
            app_data_dir.join(&s.output_dir)
        };
        let mut cfg = Self::new(accounts_dir, downloads_dir);
        cfg.logs_dir = app_data_dir.join("logs");
        cfg.mode_fallback = s.mode_fallback;
        cfg.executable_override = std::env::var_os("LVC_CHROME").map(PathBuf::from);
        cfg
    }
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Ap cai dat moi cho engine DANG chay (Task 9 goi sau khi nguoi dung luu form cai dat).
    ///
    /// CHI `threads` co hieu luc NGAY LAP TUC, qua `LimiterHandle::set_threads` —
    /// [`crate::limiter::DynamicLimiter::acquire`] tu doc lai gioi han o lan xin permit ke tiep,
    /// KHONG cat job dang chay (dung ADR-016, xem `limiter.rs`).
    ///
    /// `accounts_dir`/`output_dir`/`mode_fallback` nam trong `EngineConfig` (`self.cfg`) — struct
    /// nay BAT BIEN sau khi `Engine::new` chay xong (khong co setter), nen doi cai dat KHONG lam gi
    /// ca; chung chi co hieu luc tu lan `Engine::new` KE TIEP (Tauri command tao engine moi sau khi
    /// luu cai dat). Ham nay ghi `warn` khi cac gia tri do khac `self.cfg` de nguoi dung/nguoi doc
    /// log hieu vi sao cai dat "khong an" ngay — day la ruling cua brief task-5, khong phai thieu
    /// sot.
    pub fn apply_settings(&self, s: &AppSettings) {
        self.handle.set_threads(s.threads);
        info!(
            threads = s.threads,
            "apply_settings: da doi so luong luong dong thoi (co hieu luc ngay)"
        );

        if !s.accounts_dir.is_empty() && Path::new(&s.accounts_dir) != self.cfg.accounts_dir {
            warn!(
                muon = %s.accounts_dir,
                dang_dung = %self.cfg.accounts_dir.display(),
                "apply_settings: accounts_dir da doi nhung KHONG co hieu luc ngay - can khoi dong lai engine"
            );
        }
        if Path::new(&s.output_dir) != self.cfg.downloads_dir {
            warn!(
                muon = %s.output_dir,
                dang_dung = %self.cfg.downloads_dir.display(),
                "apply_settings: output_dir da doi nhung KHONG co hieu luc ngay - can khoi dong lai engine"
            );
        }
        if s.mode_fallback != self.cfg.mode_fallback {
            warn!(
                muon = s.mode_fallback,
                dang_dung = self.cfg.mode_fallback,
                "apply_settings: mode_fallback da doi nhung KHONG co hieu luc ngay - can khoi dong lai engine"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lvc_browser::FakeDriver;
    use lvc_core::{AppSettings, Limits};
    use lvc_store::{AccountRepo, Db};

    use super::*;

    // ---- EngineConfig::from_settings ----

    #[test]
    fn accounts_dir_rong_dung_app_data_dir() {
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.accounts_dir, PathBuf::from("/app/data/accounts"));
    }

    #[test]
    fn accounts_dir_khong_rong_dung_truc_tiep() {
        let s = AppSettings {
            accounts_dir: "D:/tai-khoan".to_string(),
            ..AppSettings::default()
        };
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.accounts_dir, PathBuf::from("D:/tai-khoan"));
    }

    #[test]
    fn output_dir_tuong_doi_noi_vao_app_data_dir() {
        let s = AppSettings::default(); // output_dir = "downloads"
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.downloads_dir, PathBuf::from("/app/data/downloads"));
    }

    #[test]
    fn output_dir_tuyet_doi_giu_nguyen() {
        let s = AppSettings {
            output_dir: "/mnt/videos".to_string(),
            ..AppSettings::default()
        };
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.downloads_dir, PathBuf::from("/mnt/videos"));
    }

    #[test]
    fn logs_dir_luon_la_app_data_dir_logs() {
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.logs_dir, PathBuf::from("/app/data/logs"));
    }

    #[test]
    fn mode_fallback_truyen_thang_qua() {
        let s = AppSettings {
            mode_fallback: false,
            ..AppSettings::default()
        };
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert!(!cfg.mode_fallback);

        let s2 = AppSettings {
            mode_fallback: true,
            ..AppSettings::default()
        };
        let cfg2 = EngineConfig::from_settings(&s2, Path::new("/app/data"));
        assert!(cfg2.mode_fallback);
    }

    #[test]
    fn executable_override_doc_cung_bien_moi_truong_voi_cli() {
        // Khong tu dat/xoa LVC_CHROME o day (Rust 2024: set_var/remove_var la unsafe, doi trang
        // thai global toan tien trinh giua cac test chay song song). Thay vao do kiem tinh chat
        // "doc dung qua bien do" bang cach so sanh voi gia tri MOI TRUONG THUC TE hien co, giong
        // het bieu thuc trong from_settings/Args::with_env — dung ca khi bien co dat tren may CI
        // lan khong.
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(
            cfg.executable_override,
            std::env::var_os("LVC_CHROME").map(PathBuf::from)
        );
    }

    // ---- Engine::apply_settings ----

    async fn engine_with_accounts(names: &[&str]) -> (tempfile::TempDir, Engine<FakeDriver>) {
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
        let driver = Arc::new(FakeDriver::new(vec![]));
        let (engine, _handle) = Engine::new(driver, db, Limits::default(), cfg, 4)
            .await
            .unwrap();
        (dir, engine)
    }

    /// Cot loi cua brief: `apply_settings` doi `threads` phai thay `LimiterHandle::effective()` doi
    /// theo (2 tai khoan trong DB tam -> `effective = min(threads, 2)`).
    #[tokio::test]
    async fn apply_settings_doi_threads_lam_doi_effective() {
        let (_dir, engine) = engine_with_accounts(&["a", "b"]).await;

        let s2 = AppSettings {
            threads: 2,
            ..AppSettings::default()
        };
        engine.apply_settings(&s2);
        assert_eq!(engine.limiter().effective(), 2);

        let s1 = AppSettings {
            threads: 1,
            ..AppSettings::default()
        };
        engine.apply_settings(&s1);
        assert_eq!(engine.limiter().effective(), 1);
    }

    /// `threads` lon hon so tai khoan (2) van bi `effective()` kep boi `accounts` — `apply_settings`
    /// khong tu kep rieng, chi chuyen thang cho `LimiterHandle` (dung nhu Task 15 da chot).
    #[tokio::test]
    async fn apply_settings_threads_vuot_so_tai_khoan_van_bi_kep_boi_accounts() {
        let (_dir, engine) = engine_with_accounts(&["a", "b"]).await;

        let s = AppSettings {
            threads: 10,
            ..AppSettings::default()
        };
        engine.apply_settings(&s);
        assert_eq!(engine.limiter().effective(), 2);
    }

    /// `accounts_dir`/`output_dir`/`mode_fallback` khac `self.cfg`: khong panic, khong doi `cfg`
    /// (bat bien) — chi con duong quan sat duoc la log `warn` (khong assert log o day, chi dam bao
    /// goi an toan va `config()` khong doi).
    #[tokio::test]
    async fn apply_settings_khong_doi_cfg_bat_bien() {
        let (_dir, engine) = engine_with_accounts(&["a"]).await;
        let before = engine.config().clone();

        let s = AppSettings {
            threads: 1,
            accounts_dir: "C:/khac".to_string(),
            output_dir: "C:/khac-nua".to_string(),
            mode_fallback: !before.mode_fallback,
            ..AppSettings::default()
        };
        engine.apply_settings(&s);

        let after = engine.config().clone();
        assert_eq!(before.accounts_dir, after.accounts_dir);
        assert_eq!(before.downloads_dir, after.downloads_dir);
        assert_eq!(before.mode_fallback, after.mode_fallback);
    }
}
