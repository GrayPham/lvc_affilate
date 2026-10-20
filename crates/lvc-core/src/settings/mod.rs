//! `AppSettings` — cài đặt người dùng từ bảng `settings` (P3 Task 3, `lvc-store::settings`).
//!
//! Bảng `settings` là key/value chuỗi thô (giữ nguyên tên cột của bản Python `browser_pool.py` để
//! DB đã di cư vẫn đọc được). [`AppSettings::from_map`] đọc map đó, tự áp mặc định + kiểm hợp lệ
//! cho TỪNG trường độc lập: giá trị sai KHÔNG làm hỏng cả bản ghi — chỉ riêng trường đó quay về
//! mặc định + một dòng cảnh báo trong `Vec` trả về (không panic, không mất cài đặt trường khác).
//!
//! Khoá không nằm trong danh sách khoá đã biết bị bỏ qua lặng lẽ.
//!
//! ## Chia file (luật chia module trong `CLAUDE.md`)
//! File này CHỈ ghép lại; mỗi NHÓM cài đặt nằm ở một file riêng, thêm nhóm = thêm một file + vài
//! dòng ở đây:
//!
//! | File | Nhóm trên UI | Khoá |
//! |---|---|---|
//! | [`storage`] | Lưu trữ | `subdir_pattern`, `filename_template`, `duplicate_policy` |
//! | [`run`] | Chạy tác vụ | `on_error`, `rest_between`, `missing_account` |
//! | [`channel`] | Dola | `dola_channel` |
//! | [`parse`] | — | hàm đọc giá trị thô dùng chung |
//!
//! Task 9 (Tauri command, P3): `AppSettings::from_map(repo.get_all().await?)` →
//! `EngineConfig::from_settings` / `Engine::apply_settings` (`lvc-engine::settings`).

mod channel;
mod parse;
mod run;
mod storage;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::mode::Mode;
use crate::scene::{DEFAULT_DURATION, DURATIONS};
use crate::value::{Model, Ratio};
use parse::{bool_str, get_string, invalid, parse_bool, parse_enum};

pub use channel::DolaChannel;
pub use run::{MISSING_ACCOUNT_WAIT_SECS, MissingAccount, OnError, REST_CHOICES, REST_DEFAULT};
pub use storage::{
    DuplicatePolicy, FILENAME_TEMPLATE_DEFAULT, FILENAME_TOKENS, FileNameVars, SubdirPattern,
    render_filename, slug,
};

/// Trần trên `threads` — không có trong bản Python (UI P3 phải chặn người dùng gõ số quá lớn làm
/// treo máy); `LimiterHandle` (lvc-engine) vẫn tự kẹp riêng `[1, 100]`, đây là trần ở tầng cài đặt.
pub const THREADS_MAX: usize = 64;

const THREADS_DEFAULT: usize = 1;
/// Phần A (run-robustness): số node đồ thị chạy song song. Mặc định 1 = tuần tự như trước.
const NODE_THREADS_DEFAULT: usize = 1;
const SILENT_MODE_DEFAULT: bool = false;
const MODE_FALLBACK_DEFAULT: bool = true;
const ACCOUNTS_DIR_DEFAULT: &str = "";
const OUTPUT_DIR_DEFAULT: &str = "downloads";

/// Cài đặt toàn cục, gõ từ bảng `settings` — xem tài liệu module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Khoá `threads` — số lượng luồng tạo video đồng thời, 1..=[`THREADS_MAX`].
    pub threads: usize,
    /// Khoá `node_threads` (Phần A) — số NODE đồ thị chạy song song, 1..=[`THREADS_MAX`]; 1 = tuần
    /// tự. Khác `threads` (gate từng job video); đây gate ở tầng node của scheduler.
    pub node_threads: usize,
    /// Khoá `silent_mode` — Python lưu "1"/"0".
    pub silent_mode: bool,
    /// Khoá `mode_fallback` — config.py `MODE_FALLBACK`: hết lượt mode này thì tự xoay sang mode
    /// còn lượt (xem `lvc_core::mode_fallback_chain`).
    pub mode_fallback: bool,
    /// Khoá `accounts_dir` — rỗng = để `EngineConfig::from_settings` tự chọn (`app_data_dir/accounts`).
    pub accounts_dir: String,
    /// Khoá `output_dir` — tương đối thì `EngineConfig::from_settings` nối vào `app_data_dir`.
    pub output_dir: String,
    /// Khoá `default_mode`.
    pub default_mode: Mode,
    /// Khoá `default_model`.
    pub default_model: Model,
    /// Khoá `default_ratio`.
    pub default_ratio: Ratio,
    /// Khoá `default_duration`.
    pub default_duration: u16,
    /// Khoá `dola_channel` — kênh chạy Dola (`api`/`browser`), mặc định `api` (P4b).
    pub dola_channel: DolaChannel,

    // --- Nhóm "Lưu trữ" (xem [`storage`]) ---
    /// Khoá `subdir_pattern` — tạo thư mục con theo gì dưới [`AppSettings::output_dir`].
    pub subdir_pattern: SubdirPattern,
    /// Khoá `filename_template` — mẫu tên file, xem [`FILENAME_TOKENS`].
    pub filename_template: String,
    /// Khoá `duplicate_policy` — khi trùng tên file thì làm gì.
    pub duplicate_policy: DuplicatePolicy,

    // --- Nhóm "Chạy tác vụ" (xem [`run`]) ---
    /// Khoá `on_error` — một workflow trong hàng chờ lỗi thì bỏ qua hay dừng cả hàng chờ.
    pub on_error: OnError,
    /// Khoá `rest_between` — nghỉ bao nhiêu GIÂY giữa hai workflow, một trong [`REST_CHOICES`].
    pub rest_between: u16,
    /// Khoá `missing_account` — tới lượt mà không có tài khoản dùng được thì bỏ qua hay chờ.
    pub missing_account: MissingAccount,

    // --- Nhóm "API key" ---
    /// Khoá `api_key_auto_rotate` — bật thì khi một `api_key` hết hạn mức/bị quota, tự chọn key
    /// khác cùng nhà cung cấp còn hạn mức (xem `lvc-app` textgen resolver). Mặc định tắt.
    pub api_key_auto_rotate: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            threads: THREADS_DEFAULT,
            node_threads: NODE_THREADS_DEFAULT,
            silent_mode: SILENT_MODE_DEFAULT,
            mode_fallback: MODE_FALLBACK_DEFAULT,
            accounts_dir: ACCOUNTS_DIR_DEFAULT.to_string(),
            output_dir: OUTPUT_DIR_DEFAULT.to_string(),
            default_mode: Mode::default(),
            default_model: Model::default(),
            default_ratio: Ratio::default(),
            default_duration: DEFAULT_DURATION,
            dola_channel: DolaChannel::default(),
            subdir_pattern: SubdirPattern::default(),
            filename_template: FILENAME_TEMPLATE_DEFAULT.to_string(),
            duplicate_policy: DuplicatePolicy::default(),
            on_error: OnError::default(),
            rest_between: REST_DEFAULT,
            missing_account: MissingAccount::default(),
            api_key_auto_rotate: false,
        }
    }
}

/// Parse một khoá "số luồng" trong [1, `THREADS_MAX`]; 0/sai → `default`, vượt trần → `THREADS_MAX`.
/// Dùng chung cho `threads` và `node_threads` (Phần A).
fn parse_thread_count(
    map: &HashMap<String, String>,
    key: &str,
    default: usize,
    warnings: &mut Vec<String>,
) -> usize {
    match map.get(key) {
        None => default,
        Some(raw) => match raw.trim().parse::<usize>() {
            Ok(0) | Err(_) => {
                invalid(warnings, key, raw, default);
                default
            }
            Ok(n) if n > THREADS_MAX => {
                warnings.push(format!(
                    "{key}: {n} vuot gioi han {THREADS_MAX}, dung {THREADS_MAX}"
                ));
                THREADS_MAX
            }
            Ok(n) => n,
        },
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

impl AppSettings {
    /// Đọc từ map key/value thô của bảng `settings` (`lvc-store::settings::SettingsRepo::get_all`).
    /// Khoá thiếu → mặc định, KHÔNG cảnh báo. Khoá có nhưng giá trị sai → mặc định + một dòng trong
    /// `Vec` trả về. Khoá không thuộc danh sách đã biết bị bỏ qua lặng lẽ.
    ///
    /// Không dùng `Result`: một khoá sai không được làm hỏng toàn bộ cài đặt — đây là lựa chọn có
    /// chủ định của brief Task 5, khác các hàm `parse` khác trong crate này (Mode/Model/Ratio) vốn
    /// trả `Result` vì dùng ở tầng từng trường đơn lẻ (TXT/CLI).
    #[must_use]
    #[allow(
        clippy::implicit_hasher,
        reason = "khoa cua bang settings co dinh la String/String, khong can generic hoa BuildHasher"
    )]
    pub fn from_map(map: &HashMap<String, String>) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let settings = Self {
            threads: parse_thread_count(map, "threads", THREADS_DEFAULT, &mut warnings),
            node_threads: parse_thread_count(
                map,
                "node_threads",
                NODE_THREADS_DEFAULT,
                &mut warnings,
            ),
            silent_mode: parse_bool(map, "silent_mode", SILENT_MODE_DEFAULT, &mut warnings),
            mode_fallback: parse_bool(map, "mode_fallback", MODE_FALLBACK_DEFAULT, &mut warnings),
            accounts_dir: get_string(map, "accounts_dir", ACCOUNTS_DIR_DEFAULT),
            output_dir: get_string(map, "output_dir", OUTPUT_DIR_DEFAULT),
            default_mode: parse_enum(
                map,
                "default_mode",
                Mode::default(),
                Mode::default().as_str(),
                Mode::parse,
                &mut warnings,
            ),
            default_model: parse_enum(
                map,
                "default_model",
                Model::default(),
                Model::default().as_str(),
                Model::parse,
                &mut warnings,
            ),
            default_ratio: parse_enum(
                map,
                "default_ratio",
                Ratio::default(),
                Ratio::default().as_str(),
                Ratio::parse,
                &mut warnings,
            ),
            default_duration: parse_duration(map, &mut warnings),
            dola_channel: channel::parse_dola_channel(map, &mut warnings),
            subdir_pattern: storage::parse_subdir_pattern(map, &mut warnings),
            filename_template: storage::parse_filename_template(map, &mut warnings),
            duplicate_policy: storage::parse_duplicate_policy(map, &mut warnings),
            on_error: run::parse_on_error(map, &mut warnings),
            rest_between: run::parse_rest_between(map, &mut warnings),
            missing_account: run::parse_missing_account(map, &mut warnings),
            api_key_auto_rotate: parse_bool(map, "api_key_auto_rotate", false, &mut warnings),
        };
        (settings, warnings)
    }

    /// Ngược lại [`AppSettings::from_map`] — luôn ra chuỗi HỢP LỆ (bool ghi "1"/"0" như bản Python,
    /// không phải "true"/"false").
    #[must_use]
    #[allow(
        clippy::implicit_hasher,
        reason = "khoa cua bang settings co dinh la String/String, khong can generic hoa BuildHasher"
    )]
    pub fn to_map(&self) -> HashMap<String, String> {
        HashMap::from([
            ("threads".to_string(), self.threads.to_string()),
            ("node_threads".to_string(), self.node_threads.to_string()),
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
            (
                "subdir_pattern".to_string(),
                self.subdir_pattern.as_str().to_string(),
            ),
            (
                "filename_template".to_string(),
                self.filename_template.clone(),
            ),
            (
                "duplicate_policy".to_string(),
                self.duplicate_policy.as_str().to_string(),
            ),
            ("on_error".to_string(), self.on_error.as_str().to_string()),
            ("rest_between".to_string(), self.rest_between.to_string()),
            (
                "missing_account".to_string(),
                self.missing_account.as_str().to_string(),
            ),
            (
                "api_key_auto_rotate".to_string(),
                bool_str(self.api_key_auto_rotate).to_string(),
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
        assert_eq!(d.subdir_pattern, SubdirPattern::WorkflowDate);
        assert_eq!(d.filename_template, "{workflow}_{gio}_{stt}");
        assert_eq!(d.duplicate_policy, DuplicatePolicy::Number);
        assert_eq!(d.on_error, OnError::Skip);
        assert_eq!(d.rest_between, 30);
        assert_eq!(d.missing_account, MissingAccount::Skip);
    }

    #[test]
    fn dola_channel_parse_va_sai_ve_mac_dinh() {
        for (raw, want) in [
            ("api", DolaChannel::Api),
            ("browser", DolaChannel::Browser),
            ("API", DolaChannel::Api),
            ("Browser", DolaChannel::Browser),
        ] {
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
    fn nhom_luu_tru_va_chay_tac_vu_doc_dung() {
        let m = map(&[
            ("subdir_pattern", "date"),
            ("filename_template", "{ngay}-{stt}"),
            ("duplicate_policy", "overwrite"),
            ("on_error", "stop"),
            ("rest_between", "180"),
            ("missing_account", "wait"),
        ]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(s.subdir_pattern, SubdirPattern::Date);
        assert_eq!(s.filename_template, "{ngay}-{stt}");
        assert_eq!(s.duplicate_policy, DuplicatePolicy::Overwrite);
        assert_eq!(s.on_error, OnError::Stop);
        assert_eq!(s.rest_between, 180);
        assert_eq!(s.missing_account, MissingAccount::Wait);
    }

    #[test]
    fn mot_khoa_moi_sai_khong_lam_hong_khoa_khac() {
        let m = map(&[
            ("subdir_pattern", "khong-biet"),
            ("rest_between", "45"),
            ("threads", "4"),
        ]);
        let (s, warnings) = AppSettings::from_map(&m);
        assert_eq!(s.threads, 4, "khoa dung phai GIU nguyen");
        assert_eq!(s.subdir_pattern, SubdirPattern::WorkflowDate);
        assert_eq!(s.rest_between, 30);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn filename_template_rong_ve_mac_dinh_co_canh_bao() {
        let (s, warnings) = AppSettings::from_map(&map(&[("filename_template", "   ")]));
        assert_eq!(s.filename_template, FILENAME_TEMPLATE_DEFAULT);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("filename_template"), "{warnings:?}");
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
            node_threads: 4,
            silent_mode: true,
            mode_fallback: false,
            accounts_dir: "C:/acc".to_string(),
            output_dir: "C:/out".to_string(),
            default_mode: Mode::Agent,
            default_model: Model::Seedance25,
            default_ratio: Ratio::R1x1,
            default_duration: 30,
            dola_channel: DolaChannel::Browser,
            subdir_pattern: SubdirPattern::None,
            filename_template: "{tai_khoan}_{stt}".to_string(),
            duplicate_policy: DuplicatePolicy::Skip,
            on_error: OnError::Stop,
            rest_between: 60,
            missing_account: MissingAccount::Wait,
            api_key_auto_rotate: true,
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

    /// M1 (review vòng 1, task-5-review.md Minor #3): ghim ĐÚNG chuỗi wire của enum, không chỉ tên
    /// trường camelCase — một đổi Serialize của Mode/Model/Ratio làm sai hợp đồng chuỗi phía UI sẽ
    /// không bị bắt nếu chỉ kiểm field-name + roundtrip (Rust-side vẫn bằng nhau dù chuỗi đổi,
    /// miễn Deserialize đổi theo).
    #[test]
    fn serde_json_pin_wire_strings_mac_dinh() {
        let v = serde_json::to_value(AppSettings::default()).unwrap();
        assert_eq!(v["threads"], serde_json::json!(1));
        assert_eq!(v["nodeThreads"], serde_json::json!(1));
        assert_eq!(v["defaultMode"], serde_json::json!("fast"));
        assert_eq!(v["defaultModel"], serde_json::json!("seedance-2.0"));
        assert_eq!(v["defaultRatio"], serde_json::json!("9:16"));
        assert_eq!(v["defaultDuration"], serde_json::json!(10));
        assert_eq!(v["dolaChannel"], serde_json::json!("api"));
        assert_eq!(v["subdirPattern"], serde_json::json!("workflow_date"));
        assert_eq!(v["duplicatePolicy"], serde_json::json!("number"));
        assert_eq!(v["onError"], serde_json::json!("skip"));
        assert_eq!(v["restBetween"], serde_json::json!(30));
        assert_eq!(v["missingAccount"], serde_json::json!("skip"));
    }

    /// Phần A: `node_threads` mặc định 1; parse từ map + `to_map` (key `node_threads`).
    #[test]
    fn node_threads_default_parse_to_map() {
        assert_eq!(AppSettings::default().node_threads, 1);
        let mut map = std::collections::HashMap::new();
        map.insert("node_threads".to_string(), "3".to_string());
        let (s, _w) = AppSettings::from_map(&map);
        assert_eq!(s.node_threads, 3);
        assert_eq!(
            s.to_map().get("node_threads").map(String::as_str),
            Some("3")
        );
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
        assert!(json.contains("\"filenameTemplate\""), "{json}");
        assert!(!json.contains("default_mode"), "phai la camelCase: {json}");
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
