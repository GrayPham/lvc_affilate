//! `AppSettings` — cai dat nguoi dung tu bang `settings` (P3 Task 3, `lvc-store::settings`).
//!
//! Bang `settings` la key/value chuoi tho (giu nguyen ten cot cua ban Python `browser_pool.py` de
//! DB da di cu van doc duoc). [`AppSettings::from_map`] doc map do, tu ap mac dinh + kiem hop le cho
//! TUNG truong doc lap: gia tri sai KHONG lam hong ca ban ghi — chi rieng truong do quay ve mac dinh
//! + mot dong canh bao trong `Vec` tra ve (khong panic, khong mat cai dat cac truong khac).
//!
//! Khoa khong nam trong danh sach 9 khoa duoi day (vd `browser_mode`, `hub_browser` cua ban Python)
//! bi bo qua lang le.
//!
//! Task 9 (Tauri command, P3): `AppSettings::from_map(repo.get_all().await?)` →
//! `EngineConfig::from_settings` / `Engine::apply_settings` (`lvc-engine::settings`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::mode::Mode;
use crate::scene::{DEFAULT_DURATION, DURATIONS};
use crate::value::{Model, Ratio};

/// Tran tren `threads` — khong co trong ban Python (UI P3 phai chan nguoi dung go so qua lon lam
/// treo may); `LimiterHandle` (lvc-engine) van tu kep rieng `[1, 100]`, day la tran o tang cai dat.
pub const THREADS_MAX: usize = 64;

const THREADS_DEFAULT: usize = 1;
const SILENT_MODE_DEFAULT: bool = false;
const MODE_FALLBACK_DEFAULT: bool = true;
const ACCOUNTS_DIR_DEFAULT: &str = "";
const OUTPUT_DIR_DEFAULT: &str = "downloads";

/// Kenh chay Dola (P4b): `Api` = goi API app truc tiep (ky X-Gorgon/Ladon/Argus, KHONG trinh duyet);
/// `Browser` = backend trinh duyet headless (P2/P3). Mac dinh `Api`; loi ky → engine tu fallback
/// sang `Browser`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DolaChannel {
    /// Goi API app Android truc tiep (nhanh, khong Chromium).
    #[default]
    Api,
    /// Trinh duyet headless dung phien dang nhap that.
    Browser,
}

impl DolaChannel {
    /// Chuoi wire (`"api"`/`"browser"`) — dung cho bang `settings` + hop dong UI.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Browser => "browser",
        }
    }

    /// Parse tu chuoi (khong phan biet hoa/thuong). Sai → `Err`.
    ///
    /// # Errors
    /// Chuoi khong phai `"api"`/`"browser"`.
    pub fn parse(s: &str) -> Result<Self, ()> {
        match s.trim().to_lowercase().as_str() {
            "api" => Ok(Self::Api),
            "browser" => Ok(Self::Browser),
            _ => Err(()),
        }
    }
}

/// Cai dat toan cuc, go tu bang `settings` — xem tai lieu module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Khoa `threads` — so luong luong tao video dong thoi, 1..=[`THREADS_MAX`].
    pub threads: usize,
    /// Khoa `silent_mode` — Python luu "1"/"0".
    pub silent_mode: bool,
    /// Khoa `mode_fallback` — config.py `MODE_FALLBACK`: het luot mode nay thi tu xoay sang mode
    /// con luot (xem `lvc_core::mode_fallback_chain`).
    pub mode_fallback: bool,
    /// Khoa `accounts_dir` — rong = de `EngineConfig::from_settings` tu chon (`app_data_dir/accounts`).
    pub accounts_dir: String,
    /// Khoa `output_dir` — tuong doi thi `EngineConfig::from_settings` noi vao `app_data_dir`.
    pub output_dir: String,
    /// Khoa `default_mode`.
    pub default_mode: Mode,
    /// Khoa `default_model`.
    pub default_model: Model,
    /// Khoa `default_ratio`.
    pub default_ratio: Ratio,
    /// Khoa `default_duration`.
    pub default_duration: u16,
    /// Khoa `dola_channel` — kenh chay Dola (`api`/`browser`), mac dinh `api` (P4b).
    pub dola_channel: DolaChannel,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            threads: THREADS_DEFAULT,
            silent_mode: SILENT_MODE_DEFAULT,
            mode_fallback: MODE_FALLBACK_DEFAULT,
            accounts_dir: ACCOUNTS_DIR_DEFAULT.to_string(),
            output_dir: OUTPUT_DIR_DEFAULT.to_string(),
            default_mode: Mode::default(),
            default_model: Model::default(),
            default_ratio: Ratio::default(),
            default_duration: DEFAULT_DURATION,
            dola_channel: DolaChannel::default(),
        }
    }
}

/// Ghi mot dong canh bao dung dinh dang chung: `"<khoa>: '<raw>' khong hop le, dung <mac dinh>"`.
fn invalid(warnings: &mut Vec<String>, key: &str, raw: &str, default: impl std::fmt::Display) {
    warnings.push(format!("{key}: '{raw}' khong hop le, dung {default}"));
}

fn parse_threads(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> usize {
    match map.get("threads") {
        None => THREADS_DEFAULT,
        Some(raw) => match raw.trim().parse::<usize>() {
            Ok(0) | Err(_) => {
                invalid(warnings, "threads", raw, THREADS_DEFAULT);
                THREADS_DEFAULT
            }
            Ok(n) if n > THREADS_MAX => {
                warnings.push(format!(
                    "threads: {n} vuot gioi han {THREADS_MAX}, dung {THREADS_MAX}"
                ));
                THREADS_MAX
            }
            Ok(n) => n,
        },
    }
}

/// `1/0/true/false/yes/no`, khong phan biet hoa/thuong (Python luu "1"/"0", form UI co the gui
/// "true"/"false"/"yes"/"no").
fn parse_bool(
    map: &HashMap<String, String>,
    key: &str,
    default: bool,
    warnings: &mut Vec<String>,
) -> bool {
    match map.get(key) {
        None => default,
        Some(raw) => match raw.trim().to_lowercase().as_str() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => {
                invalid(warnings, key, raw, default);
                default
            }
        },
    }
}

fn get_string(map: &HashMap<String, String>, key: &str, default: &str) -> String {
    map.get(key).cloned().unwrap_or_else(|| default.to_string())
}

fn parse_mode(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> Mode {
    match map.get("default_mode") {
        None => Mode::default(),
        Some(raw) => Mode::parse(raw).unwrap_or_else(|_| {
            invalid(warnings, "default_mode", raw, Mode::default().as_str());
            Mode::default()
        }),
    }
}

fn parse_model(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> Model {
    match map.get("default_model") {
        None => Model::default(),
        Some(raw) => Model::parse(raw).unwrap_or_else(|_| {
            invalid(warnings, "default_model", raw, Model::default().as_str());
            Model::default()
        }),
    }
}

fn parse_ratio(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> Ratio {
    match map.get("default_ratio") {
        None => Ratio::default(),
        Some(raw) => Ratio::parse(raw).unwrap_or_else(|_| {
            invalid(warnings, "default_ratio", raw, Ratio::default().as_str());
            Ratio::default()
        }),
    }
}

fn parse_dola_channel(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> DolaChannel {
    match map.get("dola_channel") {
        None => DolaChannel::default(),
        Some(raw) => DolaChannel::parse(raw).unwrap_or_else(|()| {
            invalid(warnings, "dola_channel", raw, DolaChannel::default().as_str());
            DolaChannel::default()
        }),
    }
}

fn parse_duration(map: &HashMap<String, String>, warnings: &mut Vec<String>) -> u16 {
    match map.get("default_duration") {
        None => DEFAULT_DURATION,
        Some(raw) => {
            let parsed = raw
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|d| DURATIONS.contains(d));
            if let Some(d) = parsed {
                d
            } else {
                invalid(warnings, "default_duration", raw, DEFAULT_DURATION);
                DEFAULT_DURATION
            }
        }
    }
}

fn bool_str(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}

impl AppSettings {
    /// Doc tu map key/value tho cua bang `settings` (`lvc-store::settings::SettingsRepo::get_all`).
    /// Khoa thieu → mac dinh, KHONG canh bao. Khoa co nhung gia tri sai → mac dinh + mot dong trong
    /// `Vec` tra ve. Khoa khong thuoc 9 khoa da biet bi bo qua lang le.
    ///
    /// Khong dung `Result`: mot khoa sai khong duoc lam hong toan bo cai dat — day la lua chon co
    /// chu dinh cua brief Task 5, khac voi cac ham `parse` khac trong crate nay (Mode/Model/Ratio)
    /// von tra `Result` vi dung o tang tung truong don le (TXT/CLI).
    #[must_use]
    #[allow(
        clippy::implicit_hasher,
        reason = "khoa cua bang settings co dinh la String/String, khong can generic hoa BuildHasher"
    )]
    pub fn from_map(map: &HashMap<String, String>) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let settings = Self {
            threads: parse_threads(map, &mut warnings),
            silent_mode: parse_bool(map, "silent_mode", SILENT_MODE_DEFAULT, &mut warnings),
            mode_fallback: parse_bool(map, "mode_fallback", MODE_FALLBACK_DEFAULT, &mut warnings),
            accounts_dir: get_string(map, "accounts_dir", ACCOUNTS_DIR_DEFAULT),
            output_dir: get_string(map, "output_dir", OUTPUT_DIR_DEFAULT),
            default_mode: parse_mode(map, &mut warnings),
            default_model: parse_model(map, &mut warnings),
            default_ratio: parse_ratio(map, &mut warnings),
            default_duration: parse_duration(map, &mut warnings),
            dola_channel: parse_dola_channel(map, &mut warnings),
        };
        (settings, warnings)
    }

    /// Nguoc lai [`AppSettings::from_map`] — luon ra chuoi HOP LE (bool ghi "1"/"0" nhu ban Python,
    /// khong phai "true"/"false").
    #[must_use]
    #[allow(
        clippy::implicit_hasher,
        reason = "khoa cua bang settings co dinh la String/String, khong can generic hoa BuildHasher"
    )]
    pub fn to_map(&self) -> HashMap<String, String> {
        HashMap::from([
            ("threads".to_string(), self.threads.to_string()),
            (
                "silent_mode".to_string(),
                bool_str(self.silent_mode).to_string(),
            ),
            (
                "mode_fallback".to_string(),
                bool_str(self.mode_fallback).to_string(),
            ),
            ("accounts_dir".to_string(), self.accounts_dir.clone()),
            ("output_dir".to_string(), self.output_dir.clone()),
            (
                "default_mode".to_string(),
                self.default_mode.as_str().to_string(),
            ),
            (
                "default_model".to_string(),
                self.default_model.as_str().to_string(),
            ),
            (
                "default_ratio".to_string(),
                self.default_ratio.as_str().to_string(),
            ),
            (
                "default_duration".to_string(),
                self.default_duration.to_string(),
            ),
            (
                "dola_channel".to_string(),
                self.dola_channel.as_str().to_string(),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn mac_dinh_dung_nhu_brief() {
        let d = AppSettings::default();
        assert_eq!(d.threads, 1);
        assert!(!d.silent_mode);
        assert!(d.mode_fallback);
        assert_eq!(d.accounts_dir, "");
        assert_eq!(d.output_dir, "downloads");
        assert_eq!(d.default_mode, Mode::Fast);
        assert_eq!(d.default_model, Model::Seedance20);
        assert_eq!(d.default_ratio, Ratio::R9x16);
        assert_eq!(d.default_duration, 10);
        assert_eq!(d.dola_channel, DolaChannel::Api);
    }

    #[test]
    fn dola_channel_parse_va_sai_ve_mac_dinh() {
        for (raw, want) in [("api", DolaChannel::Api), ("browser", DolaChannel::Browser),
                            ("API", DolaChannel::Api), ("Browser", DolaChannel::Browser)] {
            let m = map(&[("dola_channel", raw)]);
            let (s, warnings) = AppSettings::from_map(&m);
            assert_eq!(s.dola_channel, want, "raw={raw}");
            assert!(warnings.is_empty(), "raw={raw} {warnings:?}");
        }
        let (s, warnings) = AppSettings::from_map(&map(&[("dola_channel", "xyz")]));
        assert_eq!(s.dola_channel, DolaChannel::Api);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("dola_channel"), "{warnings:?}");
    }

    #[test]
    fn map_rong_ra_mac_dinh_khong_canh_bao() {
        let (s, warnings) = AppSettings::from_map(&HashMap::new());
        assert_eq!(s, AppSettings::default());
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn map_day_du_dung_gia_tri_khong_canh_bao() {
        let m = map(&[
            ("threads", "8"),
            ("silent_mode", "1"),
            ("mode_fallback", "0"),
            ("accounts_dir", "D:/acc"),
            ("output_dir", "D:/out"),
            ("default_mode", "pro"),
            ("default_model", "seedance-2.5"),
            ("default_ratio", "16:9"),
            ("default_duration", "15"),
        ]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(s.threads, 8);
        assert!(s.silent_mode);
        assert!(!s.mode_fallback);
        assert_eq!(s.accounts_dir, "D:/acc");
        assert_eq!(s.output_dir, "D:/out");
        assert_eq!(s.default_mode, Mode::Pro);
        assert_eq!(s.default_model, Model::Seedance25);
        assert_eq!(s.default_ratio, Ratio::R16x9);
        assert_eq!(s.default_duration, 15);
    }

    #[test]
    fn threads_sai_ve_mac_dinh_co_canh_bao_dung_chu() {
        let m = map(&[("threads", "abc")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.threads, 1);
        assert_eq!(warnings, vec!["threads: 'abc' khong hop le, dung 1"]);
    }

    #[test]
    fn threads_0_ve_mac_dinh_co_canh_bao() {
        let m = map(&[("threads", "0")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.threads, 1);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn threads_am_khong_parse_duoc_ve_mac_dinh() {
        let m = map(&[("threads", "-5")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.threads, 1);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn threads_vuot_tran_kep_ve_64_co_canh_bao() {
        let m = map(&[("threads", "1000")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.threads, THREADS_MAX);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("64"), "{warnings:?}");

        let m2 = map(&[("threads", "64")]);
        let (s2, w2) = AppSettings::from_map(&m2);
        assert_eq!(s2.threads, 64);
        assert!(w2.is_empty(), "64 la gioi han, van hop le");
    }

    #[test]
    fn bool_nhieu_cach_viet_khong_phan_biet_hoa_thuong() {
        for (raw, want) in [
            ("1", true),
            ("0", false),
            ("true", true),
            ("false", false),
            ("TRUE", true),
            ("FALSE", false),
            ("yes", true),
            ("no", false),
            ("Yes", true),
            ("No", false),
        ] {
            let m = map(&[("silent_mode", raw)]);
            let (s, warnings) = AppSettings::from_map(&m);
            assert_eq!(s.silent_mode, want, "raw={raw}");
            assert!(warnings.is_empty(), "raw={raw} warnings={warnings:?}");
        }
    }

    #[test]
    fn bool_sai_ve_mac_dinh_co_canh_bao() {
        let m = map(&[("mode_fallback", "khong-biet")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert!(s.mode_fallback, "mac dinh cua mode_fallback la true");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("mode_fallback"), "{warnings:?}");
    }

    #[test]
    fn mode_model_ratio_sai_deu_ve_mac_dinh_co_canh_bao_rieng() {
        let m = map(&[
            ("default_mode", "xyz"),
            ("default_model", "3.0"),
            ("default_ratio", "21:9"),
        ]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.default_mode, Mode::Fast);
        assert_eq!(s.default_model, Model::Seedance20);
        assert_eq!(s.default_ratio, Ratio::R9x16);
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.starts_with("default_mode:")));
        assert!(warnings.iter().any(|w| w.starts_with("default_model:")));
        assert!(warnings.iter().any(|w| w.starts_with("default_ratio:")));
    }

    #[test]
    fn duration_khong_trong_danh_sach_ve_mac_dinh_co_canh_bao() {
        let m = map(&[("default_duration", "20")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.default_duration, 10);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn duration_khong_phai_so_ve_mac_dinh_co_canh_bao() {
        let m = map(&[("default_duration", "muoi")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.default_duration, 10);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn duration_30_hop_le() {
        let m = map(&[("default_duration", "30")]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.default_duration, 30);
        assert!(warnings.is_empty());
    }

    #[test]
    fn khoa_la_bi_bo_qua_lang_le() {
        let m = map(&[
            ("threads", "3"),
            ("browser_mode", "chromium"),
            ("hub_browser", "acc1"),
        ]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.threads, 3);
        assert_eq!(
            s,
            AppSettings {
                threads: 3,
                ..AppSettings::default()
            }
        );
        assert!(
            warnings.is_empty(),
            "khoa la khong duoc gay canh bao: {warnings:?}"
        );
    }

    #[test]
    fn to_map_ghi_bool_dang_1_0_khong_phai_true_false() {
        let s = AppSettings {
            silent_mode: true,
            mode_fallback: false,
            ..AppSettings::default()
        };
        let m = s.to_map();
        assert_eq!(m.get("silent_mode").map(String::as_str), Some("1"));
        assert_eq!(m.get("mode_fallback").map(String::as_str), Some("0"));
    }

    #[test]
    fn roundtrip_to_map_roi_from_map_bang_gia_tri_goc() {
        let original = AppSettings {
            threads: 12,
            silent_mode: true,
            mode_fallback: false,
            accounts_dir: "C:/acc".to_string(),
            output_dir: "C:/out".to_string(),
            default_mode: Mode::Agent,
            default_model: Model::Seedance25,
            default_ratio: Ratio::R1x1,
            default_duration: 30,
            dola_channel: DolaChannel::Browser,
        };
        let m = original.to_map();
        let (roundtripped, warnings) = AppSettings::from_map(&m);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(roundtripped, original);
    }

    #[test]
    fn roundtrip_tu_mac_dinh() {
        let original = AppSettings::default();
        let (roundtripped, warnings) = AppSettings::from_map(&original.to_map());
        assert!(warnings.is_empty());
        assert_eq!(roundtripped, original);
    }

    /// M1 (review vong 1, task-5-review.md Minor #3): ghim DUNG chuoi wire cua enum, khong chi
    /// ten truong camelCase — mot doi Serialize cua Mode/Model/Ratio lam sai hop dong chuoi phia
    /// UI se khong bi bat neu chi kiem field-name + roundtrip (Rust-side van bang nhau du chuoi
    /// doi, mien Deserialize doi theo).
    #[test]
    fn serde_json_pin_wire_strings_mac_dinh() {
        let v = serde_json::to_value(AppSettings::default()).unwrap();
        assert_eq!(v["threads"], serde_json::json!(1));
        assert_eq!(v["defaultMode"], serde_json::json!("fast"));
        assert_eq!(v["defaultModel"], serde_json::json!("seedance-2.0"));
        assert_eq!(v["defaultRatio"], serde_json::json!("9:16"));
        assert_eq!(v["defaultDuration"], serde_json::json!(10));
        assert_eq!(v["dolaChannel"], serde_json::json!("api"));
    }

    #[test]
    fn serde_json_camel_case_va_roundtrip() {
        let s = AppSettings {
            threads: 4,
            default_mode: Mode::Pro,
            ..AppSettings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"defaultMode\""), "{json}");
        assert!(json.contains("\"silentMode\""), "{json}");
        assert!(!json.contains("default_mode"), "phai la camelCase: {json}");
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
