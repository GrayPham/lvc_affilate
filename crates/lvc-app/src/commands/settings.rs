//! Lenh cai dat (Task 9): doc/ghi [`AppSettings`] (cache trong `Ctx::settings` + bang `settings`),
//! chon thu muc/tep qua `tauri-plugin-dialog`. Xem quy uoc `xxx_impl`/`xxx` o dau `commands/mod.rs`
//! — `pick_directory`/`pick_files` KHONG co `xxx_impl` rieng: toan bo than chi la goi plugin (can
//! `AppHandle`), khong con logic nghiep vu nao de tach ra ma test duoc doc lap.

use lvc_browser::BrowserDriver;
use lvc_core::AppSettings;
use lvc_store::SettingsRepo;
use tauri_plugin_dialog::DialogExt;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Loc anh cho `pick_files(images: true)` — khop cac dinh dang `NodeKind::ImageUpload` chap nhan
/// (P3 chi kiem file ton tai, chua xu ly noi dung — xem doc `lvc_engine::dag`).
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

/// Than logic `settings_get` — doc cache trong bo nho (xem doc `Ctx::settings`, KHONG doc lai DB
/// moi lan UI can hien thi).
pub async fn settings_get_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>) -> AppSettings {
    ctx.settings.read().await.clone()
}

#[allow(
    clippy::unused_async,
    reason = "giu async dong bo voi moi lenh IPC khac (quy uoc brief Task 9); than logic khong loi"
)]
#[tauri::command]
pub async fn settings_get(state: tauri::State<'_, AppState>) -> Result<AppSettings, IpcError> {
    Ok(settings_get_impl(&state).await)
}

/// Than logic `settings_set`: chuan hoa qua `to_map() -> from_map()` (bat gia tri sai kieu — vd
/// `threads` vuot tran, `default_duration` khong nam trong danh sach hop le — ve mac dinh + canh
/// bao, xem doc `lvc_core::settings::AppSettings::from_map`) TRUOC khi ghi DB — dam bao gia tri
/// LUU LAI luon la gia tri da chuan hoa (khong bao gio ghi vao DB mot gia tri se bi tu choi khi doc
/// lai lan sau). `Engine::apply_settings` da tu goi `LimiterHandle::set_threads` (xem
/// `lvc_engine::settings`) nen o day KHONG goi rieng nua.
///
/// # Errors
/// `IpcError` ma `STORE` khi ghi bang `settings` that bai.
pub async fn settings_set_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    s: &AppSettings,
) -> Result<Vec<String>, IpcError> {
    let (normalized, warnings) = AppSettings::from_map(&s.to_map());
    let map = normalized.to_map();
    let pairs: Vec<(&str, &str)> = map.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    SettingsRepo::new(&ctx.db).set_many(&pairs).await?;
    *ctx.settings.write().await = normalized.clone();
    ctx.engine.apply_settings(&normalized);
    Ok(warnings)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (AppSettings) - bien gioi IPC, khong the doi sang tham chieu"
)]
#[tauri::command]
pub async fn settings_set(
    state: tauri::State<'_, AppState>,
    s: AppSettings,
) -> Result<Vec<String>, IpcError> {
    settings_set_impl(&state, &s).await
}

/// Mo hop thoai chon THU MUC (`tauri-plugin-dialog`) — chay trong `spawn_blocking` vi
/// `blocking_pick_folder` chan luong toi khi nguoi dung dong hop thoai. `Ok(None)` khi nguoi dung
/// bam Huy (KHONG phai loi).
///
/// # Errors
/// `IpcError` ma `DIALOG_JOIN` khi than `spawn_blocking` panic/bi huy (hiem, loi he thong).
#[tauri::command]
pub async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, IpcError> {
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|e| IpcError::new("DIALOG_JOIN", e.to_string()))?;
    Ok(picked.map(|p| p.to_string()))
}

/// Nhu [`pick_directory`] nhung chon NHIEU TEP; `images = true` -> loc [`IMAGE_EXTENSIONS`] (dung
/// cho node Tai anh len). Huy hop thoai -> `Ok(vec![])`.
///
/// # Errors
/// `IpcError` ma `DIALOG_JOIN` khi than `spawn_blocking` panic/bi huy.
#[tauri::command]
pub async fn pick_files(app: tauri::AppHandle, images: bool) -> Result<Vec<String>, IpcError> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = app.dialog().file();
        if images {
            dialog = dialog.add_filter("images", IMAGE_EXTENSIONS);
        }
        dialog.blocking_pick_files()
    })
    .await
    .map_err(|e| IpcError::new("DIALOG_JOIN", e.to_string()))?;
    Ok(picked
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_support::ctx_with;

    #[tokio::test]
    async fn settings_set_threads_2_khong_canh_bao_effective_phan_anh_dung() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &["acc1", "acc2"]).await;
        let s = AppSettings {
            threads: 2,
            ..AppSettings::default()
        };
        let warnings = settings_set_impl(&ctx, &s).await.unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(ctx.engine.limiter().effective(), 2);

        let map = SettingsRepo::new(&ctx.db).get_all().await.unwrap();
        assert_eq!(map.get("threads").map(String::as_str), Some("2"));
        assert_eq!(map.get("output_dir").map(String::as_str), Some("downloads"));

        let cached = settings_get_impl(&ctx).await;
        assert_eq!(cached.threads, 2);
    }

    #[tokio::test]
    async fn settings_set_duration_sai_mot_canh_bao_gia_tri_luu_da_chuan_hoa() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let s = AppSettings {
            default_duration: 999,
            ..AppSettings::default()
        };
        let warnings = settings_set_impl(&ctx, &s).await.unwrap();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("default_duration"), "{warnings:?}");

        let map = SettingsRepo::new(&ctx.db).get_all().await.unwrap();
        assert_eq!(
            map.get("default_duration").map(String::as_str),
            Some("10"),
            "gia tri sai phai duoc chuan hoa TRUOC khi ghi DB"
        );
        let cached = settings_get_impl(&ctx).await;
        assert_eq!(cached.default_duration, 10);
    }
}
