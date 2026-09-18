//! `EngineConfig::from_settings` + `Engine::apply_settings` — cau hinh engine tu `AppSettings`
//! (Task 5, P3). Tach khoi `generate.rs` (da rat dai) theo dung yeu cau brief task-5.
//!
//! | Ham | Nguon | Ghi chu |
//! |---|---|---|
//! | [`EngineConfig::from_settings`] | moi (P3) | doc `LVC_CHROME` roi giao cho [`EngineConfig::from_settings_with`] |
//! | [`EngineConfig::from_settings_with`] | moi (P3, review vong 1 M2) | thuan — nhan `executable_override` qua tham so, test duoc khong dung `std::env::set_var` |
//! | [`resolve_dir`]/[`resolve_dirs`] | moi (P3, review vong 1 I1/I2/M3) | MOT logic giai duong dan duy nhat dung chung boi `from_settings_with` VA `settings_diff` — rong (sau trim)/tuong doi deu noi vao `app_data_dir`, tuyet doi giu nguyen |
//! | [`settings_diff`] | moi (P3, review vong 1 I1) | so sanh THUAN, tra ten truong da doi; `apply_settings` dung de log `warn` |
//! | [`Engine::apply_settings`] | moi (P3) | CHI `threads` co hieu luc NGAY (`LimiterHandle::set_threads`, ADR-016); `accounts_dir`/`output_dir`/`mode_fallback` nam trong `EngineConfig` bat bien — doi khong co tac dung, chi ghi `warn` (qua `settings_diff`) de nguoi dung biet phai khoi dong lai engine |
//!
//! ## Review vong 1 (task-5-review.md) — sua gi
//!
//! - **I1** (`output_dir` bao doi moi lan luu du khong doi gi): ban cu so sanh chuoi THO
//!   `s.output_dir` (vd `"downloads"`, tuong doi) voi `self.cfg.downloads_dir` (DA duoc
//!   `from_settings` giai thanh duong dan TUYET DOI) — hai ben khong bao gio bang nhau du gia tri
//!   logic khong doi, nen `warn!` no o MOI lan goi. Sua: `EngineConfig` them `app_data_dir:
//!   Option<PathBuf>` (`from_settings`/`from_settings_with` gan `Some`, `new()` giu `None`);
//!   `settings_diff` GIAI LAI `s` bang dung [`resolve_dirs`] (ham `from_settings_with` cung dung)
//!   roi moi so sanh voi `self.cfg` — cung logic ca hai chieu nen khong con lech gia.
//! - **I2** (`output_dir` rong → roi thang vao `app_data_dir`, lan voi `accounts/`/`logs/`/DB):
//!   `resolve_dir` coi chuoi RONG (sau `trim`) giong het truong hop khoa vang mat trong
//!   `AppSettings::from_map` — tra thu muc con mac dinh (`"downloads"`), khong `.join("")`.
//! - **M3** (`accounts_dir` tuong doi khong duoc noi vao `app_data_dir` nhu `output_dir`, se giai
//!   theo CWD tien trinh — khong doan truoc duoc trong Tauri): `resolve_dir` dung CHUNG logic cho
//!   ca hai (rong → thu muc con mac dinh; tuyet doi → giu nguyen; tuong doi → noi vao
//!   `app_data_dir`).
//! - **M2** (test bien moi truong tu so sanh voi chinh bieu thuc san xuat dung, khong bao gio
//!   chay nhanh `Some`): tach `from_settings` (doc `LVC_CHROME`, mong) khoi `from_settings_with`
//!   (thuan, nhan `Option<PathBuf>` — test truyen `Some(..)` truc tiep, khong can `set_var`).
//! - **M1** (JSON wire string chua duoc "ghim"): xem test moi trong `lvc-core/src/settings.rs`
//!   (`serde_json::to_value` doi chieu tung khoa).

use std::path::{Path, PathBuf};

use lvc_browser::BrowserDriver;
use lvc_core::AppSettings;
use tracing::{info, warn};

use crate::generate::{Engine, EngineConfig};

/// Giai MOT duong dan cai dat: rong (sau `trim`) → `app_data_dir/<default_subdir>`; tuyet doi →
/// giu nguyen; tuong doi (khac rong) → noi vao `app_data_dir` (review vong 1 I2/M3 — `accounts_dir`
/// va `output_dir` gio dung CHUNG mot quy tac, chi khac ten thu muc con mac dinh).
fn resolve_dir(raw: &str, app_data_dir: &Path, default_subdir: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return app_data_dir.join(default_subdir);
    }
    let p = PathBuf::from(trimmed);
    if p.is_absolute() {
        p
    } else {
        app_data_dir.join(trimmed)
    }
}

/// `(accounts_dir, downloads_dir)` — dung CHUNG boi [`EngineConfig::from_settings_with`] va
/// [`settings_diff`] (review vong 1 I1: chinh vi hai noi truoc day tinh khac nhau ma sinh canh bao
/// gia).
fn resolve_dirs(s: &AppSettings, app_data_dir: &Path) -> (PathBuf, PathBuf) {
    (
        resolve_dir(&s.accounts_dir, app_data_dir, "accounts"),
        resolve_dir(&s.output_dir, app_data_dir, "downloads"),
    )
}

/// So sanh THUAN (khong I/O, khong log): `cfg` dang chay so voi `s` MUON ap — tra ten cac truong
/// (dung ten khoa `settings`) THUC SU khac nhau. `accounts_dir`/`output_dir` chi so sanh duoc khi
/// `cfg.app_data_dir` la `Some` (engine tao qua `EngineConfig::from_settings`/`from_settings_with`)
/// — khong co gia tri do (`EngineConfig::new` truc tiep, vd CLI `gen_one`) thi KHONG the giai lai
/// `s` cho ra duong dan tuong duong de doi chieu, nen bo qua LANG LE (review vong 1 I1, ruling
/// controller). `mode_fallback` khong can duong dan nen luon so sanh duoc.
fn settings_diff(cfg: &EngineConfig, s: &AppSettings) -> Vec<&'static str> {
    let mut diffs = Vec::new();
    if let Some(app_data_dir) = &cfg.app_data_dir {
        let (accounts_dir, downloads_dir) = resolve_dirs(s, app_data_dir);
        if accounts_dir != cfg.accounts_dir {
            diffs.push("accounts_dir");
        }
        if downloads_dir != cfg.downloads_dir {
            diffs.push("output_dir");
        }
    }
    if s.mode_fallback != cfg.mode_fallback {
        diffs.push("mode_fallback");
    }
    diffs
}

impl EngineConfig {
    /// Dung cho Tauri command (Task 9): `AppSettings::from_map(repo.get_all().await?)` roi goi ham
    /// nay voi `app_data_dir` (Tauri `AppHandle::path().app_data_dir()`). Doc `LVC_CHROME` tu moi
    /// truong that (mong — xem [`EngineConfig::from_settings_with`] cho ban thuan/test duoc).
    #[must_use]
    pub fn from_settings(s: &AppSettings, app_data_dir: &Path) -> Self {
        Self::from_settings_with(
            s,
            app_data_dir,
            std::env::var_os("LVC_CHROME").map(PathBuf::from),
        )
    }

    /// Ban THUAN cua [`EngineConfig::from_settings`] (review vong 1 M2): nhan
    /// `executable_override` qua tham so thay vi tu doc bien moi truong — test duoc nhanh `Some`
    /// LAN `None` ma khong can `std::env::set_var` (Rust 2024: `unsafe`, doi trang thai global toan
    /// tien trinh, de flaky giua cac test chay song song).
    ///
    /// - `accounts_dir`/`downloads_dir` (tu `output_dir`): xem [`resolve_dirs`] — rong (sau
    ///   `trim`) → thu muc con mac dinh cua `app_data_dir`; tuyet doi → giu nguyen; tuong doi →
    ///   noi vao `app_data_dir`.
    /// - `logs_dir`: luon `app_data_dir/logs` (khong co khoa settings rieng).
    /// - `mode_fallback`: chep thang tu `s.mode_fallback`.
    /// - `app_data_dir`: luu lai (`Some`) de `Engine::apply_settings` (qua [`settings_diff`]) doi
    ///   chieu duoc chinh xac sau nay — day la phan sua cua I1.
    #[must_use]
    pub fn from_settings_with(
        s: &AppSettings,
        app_data_dir: &Path,
        executable_override: Option<PathBuf>,
    ) -> Self {
        let (accounts_dir, downloads_dir) = resolve_dirs(s, app_data_dir);
        let mut cfg = Self::new(accounts_dir, downloads_dir);
        cfg.logs_dir = app_data_dir.join("logs");
        cfg.mode_fallback = s.mode_fallback;
        cfg.executable_override = executable_override;
        cfg.app_data_dir = Some(app_data_dir.to_path_buf());
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
    /// luu cai dat). Ham nay ghi `warn` CHO TUNG truong THUC SU khac (qua [`settings_diff`] — review
    /// vong 1 I1: khong con so sanh tho gay canh bao gia) de nguoi dung/nguoi doc log hieu vi sao
    /// cai dat "khong an" ngay — day la ruling cua brief task-5, khong phai thieu sot.
    pub fn apply_settings(&self, s: &AppSettings) {
        self.handle.set_threads(s.threads);
        info!(
            threads = s.threads,
            "apply_settings: da doi so luong luong dong thoi (co hieu luc ngay)"
        );

        for field in settings_diff(&self.cfg, s) {
            warn!(
                field,
                "apply_settings: {field} da doi nhung KHONG co hieu luc ngay - can khoi dong lai engine"
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

    // ---- EngineConfig::from_settings / from_settings_with ----

    #[test]
    fn accounts_dir_rong_dung_app_data_dir() {
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.accounts_dir, PathBuf::from("/app/data/accounts"));
    }

    #[test]
    fn accounts_dir_khong_rong_tuyet_doi_giu_nguyen() {
        let s = AppSettings {
            accounts_dir: "D:/tai-khoan".to_string(),
            ..AppSettings::default()
        };
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.accounts_dir, PathBuf::from("D:/tai-khoan"));
    }

    /// M3 (review vong 1): `accounts_dir` TUONG DOI (khong phai duong dan tuyet doi kieu o dia
    /// `D:/...`) phai noi vao `app_data_dir`, giong het `output_dir` — CWD tien trinh Tauri khong
    /// doan truoc duoc nen khong duoc de tuong doi "troi noi".
    #[test]
    fn accounts_dir_tuong_doi_noi_vao_app_data_dir() {
        let s = AppSettings {
            accounts_dir: "tai-khoan-rieng".to_string(),
            ..AppSettings::default()
        };
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.accounts_dir, PathBuf::from("/app/data/tai-khoan-rieng"));
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

    /// I2 (review vong 1): `output_dir` RONG (sau `trim`) — khac khoa VANG MAT (do `AppSettings::
    /// from_map` da tra mac dinh "downloads" cho truong hop do) — phai ve thu muc con `downloads`
    /// cua `app_data_dir`, KHONG duoc `.join("")` roi thanh chinh `app_data_dir` (lan voi
    /// `accounts/`/`logs/`/file DB).
    #[test]
    fn output_dir_rong_sau_trim_dung_thu_muc_con_downloads() {
        for raw in ["", "   "] {
            let s = AppSettings {
                output_dir: raw.to_string(),
                ..AppSettings::default()
            };
            let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
            assert_eq!(
                cfg.downloads_dir,
                PathBuf::from("/app/data/downloads"),
                "raw={raw:?}"
            );
        }
    }

    /// Doi xung voi I2: `accounts_dir` rong sau `trim` (vd chi khoang trang) cung phai ve thu muc
    /// con `accounts`, khong phai `app_data_dir` tran.
    #[test]
    fn accounts_dir_rong_sau_trim_dung_thu_muc_con_accounts() {
        let s = AppSettings {
            accounts_dir: "   ".to_string(),
            ..AppSettings::default()
        };
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.accounts_dir, PathBuf::from("/app/data/accounts"));
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

    /// M2 (review vong 1): `from_settings_with` thuan, nhan `executable_override` qua tham so —
    /// nhanh `Some` gio kiem duoc THUC SU (khong con tautology so sanh voi chinh bieu thuc san
    /// xuat dung), khong dung `std::env::set_var`.
    #[test]
    fn from_settings_with_executable_override_some_duoc_gan_dung() {
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings_with(
            &s,
            Path::new("/app/data"),
            Some(PathBuf::from("C:/chrome/chrome.exe")),
        );
        assert_eq!(
            cfg.executable_override,
            Some(PathBuf::from("C:/chrome/chrome.exe"))
        );
    }

    #[test]
    fn from_settings_with_executable_override_none_giu_none() {
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings_with(&s, Path::new("/app/data"), None);
        assert_eq!(cfg.executable_override, None);
    }

    #[test]
    fn from_settings_luu_app_data_dir_cho_settings_diff() {
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, Path::new("/app/data"));
        assert_eq!(cfg.app_data_dir, Some(PathBuf::from("/app/data")));
    }

    #[test]
    fn engine_config_new_khong_co_app_data_dir() {
        let cfg = EngineConfig::new("accounts", "downloads");
        assert_eq!(cfg.app_data_dir, None);
    }

    // ---- settings_diff (review vong 1 I1) ----

    #[test]
    fn settings_diff_rong_khi_ap_dung_cai_dat_khong_doi() {
        let app_data_dir = Path::new("/app/data");
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, app_data_dir);
        assert!(
            settings_diff(&cfg, &s).is_empty(),
            "output_dir mac dinh tuong doi (\"downloads\") KHONG duoc bao doi khi ap lai chinh no"
        );
    }

    #[test]
    fn settings_diff_goi_dung_ten_truong_khi_thuc_su_doi() {
        let app_data_dir = Path::new("/app/data");
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, app_data_dir);

        let doi_accounts = AppSettings {
            accounts_dir: "noi-khac".to_string(),
            ..s.clone()
        };
        assert_eq!(settings_diff(&cfg, &doi_accounts), vec!["accounts_dir"]);

        let doi_output = AppSettings {
            output_dir: "noi-khac-nua".to_string(),
            ..s.clone()
        };
        assert_eq!(settings_diff(&cfg, &doi_output), vec!["output_dir"]);

        let doi_mode_fallback = AppSettings {
            mode_fallback: !s.mode_fallback,
            ..s.clone()
        };
        assert_eq!(
            settings_diff(&cfg, &doi_mode_fallback),
            vec!["mode_fallback"]
        );
    }

    /// `EngineConfig::new` truc tiep (CLI `gen_one`) khong co `app_data_dir` — khong the giai lai
    /// `s` de doi chieu thu muc nen bo qua LANG LE; `mode_fallback` khong can duong dan nen van so
    /// sanh binh thuong.
    #[test]
    fn settings_diff_bo_qua_thu_muc_khi_khong_co_app_data_dir() {
        let cfg = EngineConfig::new("accounts", "downloads");
        let s = AppSettings {
            accounts_dir: "hoan-toan-khac".to_string(),
            output_dir: "cung-khac".to_string(),
            ..AppSettings::default()
        };
        assert!(settings_diff(&cfg, &s).is_empty());

        let s2 = AppSettings {
            mode_fallback: !cfg.mode_fallback,
            ..AppSettings::default()
        };
        assert_eq!(settings_diff(&cfg, &s2), vec!["mode_fallback"]);
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

    /// Engine dung qua `EngineConfig::from_settings` (co `app_data_dir`) roi ap LAI CHINH cai dat
    /// da dung de tao no: khong duoc co canh bao (day chinh la ca I1 review vong 1 — truoc khi sua,
    /// `output_dir` mac dinh tuong doi se bao doi o day). Kiem tiep noi (khong chi don vi
    /// `settings_diff`) bang `Engine` that qua `apply_settings`.
    #[tokio::test]
    async fn apply_settings_qua_engine_that_khong_bao_doi_khi_ap_lai_chinh_no() {
        let db = Db::open_memory().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let s = AppSettings::default();
        let cfg = EngineConfig::from_settings(&s, dir.path());
        std::fs::create_dir_all(cfg.accounts_dir.join("a")).unwrap();
        AccountRepo::new(&db).upsert("a").await.unwrap();
        let driver = Arc::new(FakeDriver::new(vec![]));
        let (engine, _handle) = Engine::new(driver, db, Limits::default(), cfg.clone(), 4)
            .await
            .unwrap();

        assert!(settings_diff(engine.config(), &s).is_empty());
        engine.apply_settings(&s); // khong panic; khong co canh bao (kiem gian tiep qua settings_diff o tren)
    }
}
