//! Lệnh kho tài khoản AI (P4c) — bọc `lvc_vault::Vault`. Bí mật CHỈ đi VÀO (`credential_add`/
//! `credential_update`); ra ngoài chỉ [`CredentialView`] đã che. Xem quy ước `xxx_impl`/`xxx` ở
//! đầu `commands/mod.rs`.

use lvc_browser::BrowserDriver;
use lvc_vault::{AddCredential, CredentialView, VerifyOutcome};

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

/// Thân `credential_verify` — với `api_key` (openai/gemini) gọi list-models bằng key rồi ghi
/// `status`; provider khác trả `unknown` (kiểm bằng trình duyệt để sau).
///
/// # Errors
/// `IpcError` khi không tìm thấy hoặc lỗi ghi trạng thái.
pub async fn credential_verify_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<VerifyOutcome, IpcError> {
    Ok(ctx.vault.verify(id).await?)
}

#[tauri::command]
pub async fn credential_verify(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<VerifyOutcome, IpcError> {
    credential_verify_impl(&state, &id).await
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
    use super::*;
    use crate::commands::test_support::ctx_with;

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
