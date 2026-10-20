//! Lệnh kho tài khoản AI (P4c) — bọc `lvc_vault::Vault`. Bí mật CHỈ đi VÀO (`credential_add`/
//! `credential_update`); ra ngoài chỉ [`CredentialView`] đã che. Xem quy ước `xxx_impl`/`xxx` ở
//! đầu `commands/mod.rs`.

use std::path::PathBuf;

use lvc_browser::BrowserDriver;
use lvc_core::BrowserEngine;
use lvc_vault::{AddCredential, CredentialView, VerifyOutcome};

use super::login::platform::login_url;
use super::{Ctx, IpcError};
use crate::state::AppState;

/// # Errors
/// `IpcError` khi truy vấn/giải mã lỗi.
pub async fn credential_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    provider: Option<String>,
) -> Result<Vec<CredentialView>, IpcError> {
    Ok(ctx.vault.list(provider.as_deref()).await?)
}

#[tauri::command]
pub async fn credential_list(
    state: tauri::State<'_, AppState>,
    provider: Option<String>,
) -> Result<Vec<CredentialView>, IpcError> {
    credential_list_impl(&state, provider).await
}

/// Thân `credential_add` — `secret` đi VÀO, không bao giờ trả lại nguyên văn.
///
/// # Errors
/// `IpcError` (`CRED_*`) khi provider/kind/label/secret không hợp lệ hoặc lỗi mã hoá/lưu.
pub async fn credential_add_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    req: AddCredential,
) -> Result<CredentialView, IpcError> {
    Ok(ctx.vault.add(&req).await?)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn credential_add(
    state: tauri::State<'_, AppState>,
    provider: String,
    kind: String,
    label: String,
    secret: String,
    meta: Option<serde_json::Value>,
    proxy: Option<String>,
) -> Result<CredentialView, IpcError> {
    let req = AddCredential {
        provider,
        kind,
        label,
        secret,
        meta: meta.unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
        proxy: proxy.unwrap_or_default(),
    };
    credential_add_impl(&state, req).await
}

/// # Errors
/// `IpcError` khi ghi lỗi.
pub async fn credential_update_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    secret: Option<String>,
    label: Option<String>,
    proxy: Option<String>,
    enabled: Option<bool>,
) -> Result<(), IpcError> {
    if let Some(s) = secret.filter(|s| !s.is_empty()) {
        ctx.vault.update_secret(id, &s).await?;
    }
    if let Some(l) = label {
        ctx.vault.set_label(id, &l).await?;
    }
    if let Some(p) = proxy {
        ctx.vault.set_proxy(id, &p).await?;
    }
    if let Some(on) = enabled {
        ctx.vault.set_enabled(id, on).await?;
    }
    Ok(())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn credential_update(
    state: tauri::State<'_, AppState>,
    id: String,
    secret: Option<String>,
    label: Option<String>,
    proxy: Option<String>,
    enabled: Option<bool>,
) -> Result<(), IpcError> {
    credential_update_impl(&state, &id, secret, label, proxy, enabled).await
}

/// Dat TAG LOAI NODE cho mot credential (`"video"` | `"image"` | `"script"`) — node AI chi lay tai
/// khoan/key co dung tag (ban ban giao UI). Gia tri la bi loai bo o tang vault.
///
/// # Errors
/// `IpcError` `CRED_NOT_FOUND` khi `id` khong co; `STORE` khi ghi loi.
pub async fn credential_set_node_types_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    node_types: &[String],
) -> Result<(), IpcError> {
    ctx.vault.set_node_types(id, node_types).await?;
    Ok(())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn credential_set_node_types(
    state: tauri::State<'_, AppState>,
    id: String,
    node_types: Vec<String>,
) -> Result<(), IpcError> {
    credential_set_node_types_impl(&state, &id, &node_types).await
}

/// Đặt ngưỡng "hạn mức/ngày" cho một `api_key` (`None`/`0` = không giới hạn). Màn API key.
///
/// # Errors
/// `IpcError` `STORE` khi ghi lỗi.
pub async fn credential_set_daily_limit_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    limit: Option<i64>,
) -> Result<(), IpcError> {
    ctx.vault.set_daily_limit(id, limit).await?;
    Ok(())
}

#[tauri::command]
pub async fn credential_set_daily_limit(
    state: tauri::State<'_, AppState>,
    id: String,
    limit: Option<i64>,
) -> Result<(), IpcError> {
    credential_set_daily_limit_impl(&state, &id, limit).await
}

/// Map kết quả probe đăng nhập `MuseAI` (`Ok(true)`=đã đăng nhập, `Ok(false)`=chưa, `Err`=hạ tầng)
/// → [`VerifyOutcome`]. Thuần (test được không cần trình duyệt). `expired` khớp UI → "Cần đăng nhập
/// lại" (`app/src/pages/AccountsPage.tsx`).
fn museai_login_status(probe: Result<bool, String>) -> VerifyOutcome {
    match probe {
        Ok(true) => VerifyOutcome {
            status: "ok".to_string(),
            detail: "da dang nhap muse.ai".to_string(),
        },
        Ok(false) => VerifyOutcome {
            status: "expired".to_string(),
            detail: "chua dang nhap / phien het han — bam Dang nhap lai".to_string(),
        },
        Err(e) => VerifyOutcome {
            status: "unknown".to_string(),
            detail: e,
        },
    }
}

/// Thân `credential_verify` — `api_key` (openai/gemini) gọi list-models bằng key; `MuseAI` (cookie)
/// mở profile thật probe `muse.ai` (có `gatewayUrl` = đã đăng nhập) qua
/// [`lvc_engine::museai::probe_login`]; provider khác → `unknown`. Luôn ghi `status` sau khi kiểm.
///
/// # Errors
/// `IpcError` khi không tìm thấy hoặc lỗi ghi trạng thái.
pub async fn credential_verify_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<VerifyOutcome, IpcError> {
    let (provider, profile_dir, _proxy, engine) = ctx.vault.profile_dir_and_proxy(id).await?;
    if provider == "museai" {
        let outcome = if engine == "firefox_portable" {
            VerifyOutcome {
                status: "unknown".to_string(),
                detail: "tai khoan Firefox — mo trinh duyet kiem tra bang tay (khong co CDP)"
                    .to_string(),
            }
        } else if let Some(dir) = profile_dir {
            let probe = ctx
                .engine
                .probe_museai_login(std::path::Path::new(&dir))
                .await;
            museai_login_status(probe)
        } else {
            VerifyOutcome {
                status: "unknown".to_string(),
                detail: "chua co profile — bam Dang nhap lai de dan cookie".to_string(),
            }
        };
        ctx.vault.set_status(id, &outcome.status).await?;
        return Ok(outcome);
    }
    Ok(ctx.vault.verify(id).await?)
}

#[tauri::command]
pub async fn credential_verify(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<VerifyOutcome, IpcError> {
    credential_verify_impl(&state, &id).await
}

/// Thân `credential_open_viewer` — "Mở trình duyệt" cho tài khoản/credential (Google/Gemini/
/// Facebook/Apple/ChatGPT/MuseAI...) ĐÃ có profile thật (xem `Phần A`: tiêm cookie → profile Chrome
/// qua `lvc_engine::profiles::provision_profile`, ghi `meta.profile_dir`) — chưa có profile (vd
/// credential chỉ dán cookie trước khi có Phần A, hoặc API key không có browser) thì báo rõ
/// `PROFILE_NOT_FOUND` thay vì mở một profile RỖNG (gây hiểu lầm "đã đăng nhập mà sao trống").
///
/// # Errors
/// - `CRED_NOT_FOUND`: không có credential nào với `id` này.
/// - `PROFILE_NOT_FOUND`: credential này chưa có `meta.profile_dir`.
/// - `INVALID_PROXY`/`BROWSER`/`IO`: từ [`lvc_engine::Engine::open_profile_viewer`].
pub async fn credential_open_viewer_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<(), IpcError> {
    let (provider, profile_dir, proxy, engine) = ctx.vault.profile_dir_and_proxy(id).await?;
    let Some(profile_dir) = profile_dir else {
        return Err(IpcError::new(
            "PROFILE_NOT_FOUND",
            "tài khoản này chưa có profile trình duyệt (chưa tiêm cookie thành công) — thêm lại cookie rồi thử lại",
        ));
    };
    let profile_dir = PathBuf::from(profile_dir);
    // Engine Firefox: mở firefox.exe trên profile (không CDP). Chromium (mặc định): đường cũ.
    if BrowserEngine::parse(&engine).unwrap_or_default() == BrowserEngine::FirefoxPortable {
        return Ok(ctx.engine.open_firefox_viewer(&profile_dir)?);
    }
    Ok(ctx
        .engine
        .open_profile_viewer(&profile_dir, login_url(&provider), &proxy)
        .await?)
}

#[tauri::command]
pub async fn credential_open_viewer(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    credential_open_viewer_impl(&state, &id).await
}

/// # Errors
/// `IpcError` khi xoá lỗi.
pub async fn credential_delete_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<bool, IpcError> {
    Ok(ctx.vault.delete(id).await?)
}

#[tauri::command]
pub async fn credential_delete(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<bool, IpcError> {
    credential_delete_impl(&state, &id).await
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeStep;

    use super::*;
    use crate::commands::test_support::ctx_with;

    #[test]
    fn museai_login_status_map_dung() {
        assert_eq!(museai_login_status(Ok(true)).status, "ok");
        assert_eq!(museai_login_status(Ok(false)).status, "expired");
        let e = museai_login_status(Err("chromium loi".to_string()));
        assert_eq!(e.status, "unknown");
        assert_eq!(e.detail, "chromium loi");
    }

    #[tokio::test]
    async fn credential_verify_museai_firefox_tra_unknown_khong_mo_trinh_duyet() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let req = AddCredential {
            provider: "museai".into(),
            kind: "session".into(),
            label: "ff".into(),
            secret: "cookie-tho".into(),
            meta: serde_json::json!({
                "mode": "cookie", "engine": "firefox_portable", "profile_dir": "/tmp/x"
            }),
            proxy: String::new(),
        };
        let view = credential_add_impl(&ctx, req).await.unwrap();
        let out = credential_verify_impl(&ctx, &view.id).await.unwrap();
        assert_eq!(out.status, "unknown", "Firefox khong co CDP -> unknown");
        let after = ctx.vault.list(Some("museai")).await.unwrap();
        assert_eq!(after[0].status, "unknown", "status phai duoc ghi vao DB");
    }

    #[tokio::test]
    async fn credential_verify_museai_chua_co_profile_tra_unknown() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let req = AddCredential {
            provider: "museai".into(),
            kind: "session".into(),
            label: "noprof".into(),
            secret: "cookie-tho".into(),
            meta: serde_json::json!({ "mode": "cookie" }),
            proxy: String::new(),
        };
        let view = credential_add_impl(&ctx, req).await.unwrap();
        let out = credential_verify_impl(&ctx, &view.id).await.unwrap();
        assert_eq!(out.status, "unknown");
        assert!(out.detail.contains("profile"), "{}", out.detail);
    }

    #[tokio::test]
    async fn credential_open_viewer_impl_chua_co_profile_bao_profile_not_found() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let req = AddCredential {
            provider: "google".into(),
            kind: "session".into(),
            label: "Gmail acc1".into(),
            secret: "cookie-tho".into(),
            meta: serde_json::json!({ "mode": "cookie" }),
            proxy: String::new(),
        };
        let view = credential_add_impl(&ctx, req).await.unwrap();
        let err = credential_open_viewer_impl(&ctx, &view.id)
            .await
            .unwrap_err();
        assert_eq!(err.code, "PROFILE_NOT_FOUND");
    }

    #[tokio::test]
    async fn credential_open_viewer_impl_khong_ton_tai_bao_cred_not_found() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = credential_open_viewer_impl(&ctx, "ghost")
            .await
            .unwrap_err();
        assert_eq!(err.code, "CRED_NOT_FOUND");
    }

    #[tokio::test]
    async fn credential_open_viewer_impl_co_profile_mo_duoc() {
        let (dir, ctx, _driver) = ctx_with(vec![FakeStep::Launch(Ok(()))], &[]).await;
        let profile_dir = dir
            .path()
            .join("accounts")
            .join(".logins")
            .join("google")
            .join("acc1");
        let req = AddCredential {
            provider: "google".into(),
            kind: "session".into(),
            label: "Gmail acc1".into(),
            secret: "cookie-tho".into(),
            meta: serde_json::json!({
                "mode": "cookie",
                "profile_dir": profile_dir.to_string_lossy(),
            }),
            proxy: String::new(),
        };
        let view = credential_add_impl(&ctx, req).await.unwrap();
        credential_open_viewer_impl(&ctx, &view.id).await.unwrap();
    }

    #[tokio::test]
    async fn them_liet_ke_kiem_xoa_khong_lo_secret() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let req = AddCredential {
            provider: "openai".into(),
            kind: "api_key".into(),
            label: "key chinh".into(),
            secret: "sk-super-secret-9999".into(),
            meta: serde_json::json!({}),
            proxy: String::new(),
        };
        let view = credential_add_impl(&ctx, req).await.unwrap();
        assert_eq!(view.secret_hint, "••••9999");

        let list = credential_list_impl(&ctx, Some("openai".into()))
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        let json = serde_json::to_string(&list).unwrap();
        assert!(!json.contains("super-secret"), "IPC lo secret: {json}");

        credential_update_impl(
            &ctx,
            &view.id,
            None,
            Some("doi ten".into()),
            None,
            Some(false),
        )
        .await
        .unwrap();
        let after = credential_list_impl(&ctx, None).await.unwrap();
        assert_eq!(after[0].label, "doi ten");
        assert!(!after[0].enabled);

        assert!(credential_delete_impl(&ctx, &view.id).await.unwrap());
        assert!(credential_list_impl(&ctx, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn provider_sai_tra_ma_on_dinh() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let req = AddCredential {
            provider: "khong-co".into(),
            kind: "api_key".into(),
            label: "x".into(),
            secret: "k".into(),
            meta: serde_json::json!({}),
            proxy: String::new(),
        };
        let err = credential_add_impl(&ctx, req).await.unwrap_err();
        assert_eq!(err.code, "CRED_BAD_PROVIDER");
    }
}
