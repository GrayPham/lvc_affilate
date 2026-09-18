//! Lenh tai khoan (Task 10): liet ke + trang thai, nhap cookie, kiem tra dang nhap, bat/tat lich,
//! sua proxy rieng, xoa, nhap du lieu ban Python cu. Xem quy uoc `xxx_impl`/`xxx` o dau
//! `commands/mod.rs`. Than logic THAT SU nam het o `lvc_engine::accounts::Engine::{..}` (Task 10,
//! crate `lvc-engine`) — moi ham o day chi goi thang qua `?` (doi `AccountError`/`EngineError`
//! sang `IpcError` bang hai `impl From` o `commands/mod.rs`), khong lap lai logic nghiep vu.

use std::path::Path;

use lvc_browser::BrowserDriver;
use lvc_engine::{AccountStatus, ImportCookiesReport, LegacyImportReport, VerifyReport, now_unix};

use super::{Ctx, IpcError};
use crate::state::AppState;

/// # Errors
/// `IpcError` ma `STORE` khi doc bang `accounts_meta`/`usage` loi.
pub async fn account_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
) -> Result<Vec<AccountStatus>, IpcError> {
    Ok(ctx.engine.list_with_status(now_unix()).await?)
}

#[tauri::command]
pub async fn account_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<AccountStatus>, IpcError> {
    account_list_impl(&state).await
}

/// # Errors
/// `IpcError` ma `INVALID_NAME`/`INVALID_PROXY`/`COOKIE_INVALID`/`ACCOUNT_BUSY`/`ACCOUNT_EXISTS`/
/// `COOKIE_NOT_LOGGED_IN`/`BROWSER`/`STORE`/`IO` — xem `lvc_engine::accounts::AccountError::code`
/// (`ACCOUNT_BUSY`: Minor 4 review fix-round 1, ten MOI dang bi mot lan goi khac thue cung luc).
pub async fn account_import_cookies_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    name: &str,
    cookie_text: &str,
    proxy: &str,
) -> Result<ImportCookiesReport, IpcError> {
    Ok(ctx.engine.import_cookies(name, cookie_text, proxy).await?)
}

#[tauri::command]
pub async fn account_import_cookies(
    state: tauri::State<'_, AppState>,
    name: String,
    cookie_text: String,
    proxy: String,
) -> Result<ImportCookiesReport, IpcError> {
    account_import_cookies_impl(&state, &name, &cookie_text, &proxy).await
}

/// # Errors
/// `IpcError` ma `INVALID_NAME`/`ACCOUNT_NOT_FOUND`/`INVALID_PROXY`/`PROFILE_BUSY`/`BROWSER`/`STORE`.
pub async fn account_verify_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    name: &str,
) -> Result<VerifyReport, IpcError> {
    Ok(ctx.engine.verify(name).await?)
}

#[tauri::command]
pub async fn account_verify(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<VerifyReport, IpcError> {
    account_verify_impl(&state, &name).await
}

/// # Errors
/// `IpcError` ma `STORE`/`ENGINE` khi ghi DB hoac dem lai so tai khoan cho limiter loi.
pub async fn account_set_scheduling_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    name: &str,
    on: bool,
) -> Result<(), IpcError> {
    Ok(ctx.engine.set_scheduling(name, on).await?)
}

#[tauri::command]
pub async fn account_set_scheduling(
    state: tauri::State<'_, AppState>,
    name: String,
    on: bool,
) -> Result<(), IpcError> {
    account_set_scheduling_impl(&state, &name, on).await
}

/// # Errors
/// `IpcError` ma `INVALID_PROXY` khi sai dinh dang; `STORE` khi ghi DB loi.
pub async fn account_set_proxy_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    name: &str,
    proxy: &str,
) -> Result<(), IpcError> {
    Ok(ctx.engine.set_proxy(name, proxy).await?)
}

#[tauri::command]
pub async fn account_set_proxy(
    state: tauri::State<'_, AppState>,
    name: String,
    proxy: String,
) -> Result<(), IpcError> {
    account_set_proxy_impl(&state, &name, &proxy).await
}

/// `false` = khong co gi de xoa (khong dong DB lan khong thu muc — idempotent, giong
/// `workflow_delete_impl`).
///
/// # Errors
/// `IpcError` ma `ACCOUNT_BUSY`/`PROFILE_BUSY` khi tai khoan dang co tac vu/phien dung; `STORE`/
/// `ENGINE` khi xoa dong hoac dem lai so tai khoan loi.
pub async fn account_delete_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    name: &str,
) -> Result<bool, IpcError> {
    Ok(ctx.engine.delete(name).await?)
}

#[tauri::command]
pub async fn account_delete(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<bool, IpcError> {
    account_delete_impl(&state, &name).await
}

/// # Errors
/// `IpcError` ma `STORE`/`IO`/`ENGINE` — xem `lvc_engine::accounts::Engine::import_legacy`.
pub async fn legacy_import_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    python_dir: &Path,
) -> Result<LegacyImportReport, IpcError> {
    Ok(ctx.engine.import_legacy(python_dir).await?)
}

#[tauri::command]
pub async fn legacy_import(
    state: tauri::State<'_, AppState>,
    python_dir: String,
) -> Result<LegacyImportReport, IpcError> {
    legacy_import_impl(&state, Path::new(&python_dir)).await
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeStep;
    use lvc_core::SessionState;

    use super::*;
    use crate::commands::test_support::ctx_with;

    const COOKIE_TEXT: &str = ".dola.com\tTRUE\t/\tTRUE\t1800000000\tsessionid\tabc123\n";

    #[tokio::test]
    async fn account_list_impl_tra_ve_tai_khoan_da_seed() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &["acc1", "acc2"]).await;
        let list = account_list_impl(&ctx).await.unwrap();
        let mut names: Vec<&str> = list.iter().map(|s| s.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["acc1", "acc2"]);
    }

    #[tokio::test]
    async fn account_import_cookies_impl_logged_in_tra_report() {
        let (_dir, ctx, _driver) = ctx_with(vec![FakeStep::Launch(Ok(()))], &[]).await;
        let report = account_import_cookies_impl(&ctx, "acc_new", COOKIE_TEXT, "")
            .await
            .unwrap();
        assert_eq!(report.name, "acc_new");
        assert_eq!(report.sessionid_expires_at, Some(1_800_000_000));
    }

    #[tokio::test]
    async fn account_import_cookies_impl_cookie_sai_bao_ma_dung() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = account_import_cookies_impl(&ctx, "acc_new", "rac", "")
            .await
            .unwrap_err();
        assert_eq!(err.code, "COOKIE_INVALID");
    }

    #[tokio::test]
    async fn account_verify_impl_khong_ton_tai_bao_ma_dung() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = account_verify_impl(&ctx, "ghost").await.unwrap_err();
        assert_eq!(err.code, "ACCOUNT_NOT_FOUND");
    }

    #[tokio::test]
    async fn account_verify_impl_logged_out_ghi_db() {
        let (_dir, ctx, _driver) = ctx_with(
            vec![FakeStep::LaunchWithState(Ok(SessionState::LoggedOut))],
            &["acc1"],
        )
        .await;
        let report = account_verify_impl(&ctx, "acc1").await.unwrap();
        assert_eq!(report.login_ok, Some(false));
    }

    #[tokio::test]
    async fn account_set_scheduling_impl_lat_duoc() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &["acc1"]).await;
        account_set_scheduling_impl(&ctx, "acc1", false)
            .await
            .unwrap();
        let list = account_list_impl(&ctx).await.unwrap();
        assert!(!list[0].scheduling);
    }

    #[tokio::test]
    async fn account_set_proxy_impl_sai_bao_ma_dung() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &["acc1"]).await;
        let err = account_set_proxy_impl(&ctx, "acc1", "khong-hop-le")
            .await
            .unwrap_err();
        assert_eq!(err.code, "INVALID_PROXY");
    }

    #[tokio::test]
    async fn account_delete_impl_xoa_tra_true_lan_hai_tra_false() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &["acc1"]).await;
        assert!(account_delete_impl(&ctx, "acc1").await.unwrap());
        assert!(!account_delete_impl(&ctx, "acc1").await.unwrap());
    }

    /// C1 (review fix-round 1, Critical): ten nguy hiem phai ra ma `INVALID_NAME` qua het lop IPC —
    /// khong chi kiem o `lvc-engine`, ma ca duong `xxx_impl` thuc su UI se goi.
    #[tokio::test]
    async fn account_delete_impl_ten_khong_hop_le_bao_ma_dung() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &["sibling"]).await;
        let err = account_delete_impl(&ctx, "..").await.unwrap_err();
        assert_eq!(err.code, "INVALID_NAME");
    }

    #[tokio::test]
    async fn legacy_import_impl_thu_muc_rong_khong_loi() {
        let (dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let legacy_dir = dir.path().join("khong-co-gi-o-day");
        let report = legacy_import_impl(&ctx, &legacy_dir).await.unwrap();
        assert_eq!(report.tasks, 0);
        assert_eq!(report.profiles_copied, 0);
        assert_eq!(report.skipped.len(), 2);
    }
}
