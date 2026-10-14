//! (A) Nạp cookie/phiên vào kho credentials theo NỀN TẢNG — người dùng mang phiên đã đăng nhập của
//! chính mình, qua FILE (`login_import_cookie_files`, nhiều file cùng lúc) hoặc dán trực tiếp MỘT
//! đoạn (`login_cookie_text`). Mỗi mục = một entry `session`. Một file lỗi KHÔNG dừng cả lô. Đường
//! HỢP LỆ: không đăng nhập tự động, không né bot.
//!
//! Cả hai lệnh đều TIÊM cookie vào một profile Chromium THẬT qua
//! [`lvc_engine::Engine::provision_profile`] (tổng quát hoá pipeline Dola —
//! `lvc_engine::accounts::Engine::import_cookies` — cho MỌI nền tảng khác) rồi lưu `meta.profile_dir`
//! tham chiếu profile đó, để sau này "Mở trình duyệt" (`credential_open_viewer`) dùng lại được.

use std::path::Path;

use lvc_browser::BrowserDriver;
use lvc_vault::{AddCredential, CredentialView};
use tracing::warn;

use crate::commands::{Ctx, IpcError};
use crate::state::AppState;

/// Tiêm `cookie_text` vào một profile Chrome thật ([`lvc_engine::Engine::provision_profile`]) rồi
/// dựng `meta` cho credential. Lỗi `BROWSER`/`IO`/`INVALID_PROXY` (trình duyệt/máy lỗi) KHÔNG chặn lưu
/// credential — chỉ cảnh báo, `meta` thiếu `profile_dir` (phương án lùi: chỉ lưu cookie thô, giống
/// hành vi TRƯỚC khi có Phần A — "Mở trình duyệt" sẽ báo `PROFILE_NOT_FOUND` cho tới khi dán lại).
/// Lỗi `COOKIE_INVALID` (không đọc được cookie nào từ nội dung đã dán) LÀ lỗi CỨNG — trả thẳng cho
/// caller, không lưu một credential rác vào kho.
async fn provision_meta<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    provider: &str,
    label: &str,
    cookie_text: &str,
    proxy: &str,
) -> Result<serde_json::Value, IpcError> {
    match ctx
        .engine
        .provision_profile(provider, label, cookie_text, proxy)
        .await
    {
        Ok(dir) => Ok(serde_json::json!({
            "mode": "cookie",
            "profile_dir": dir.to_string_lossy(),
        })),
        Err(e) if e.code() == "COOKIE_INVALID" => Err(e.into()),
        Err(e) => {
            warn!(
                provider,
                label,
                error = %e,
                "khong tiem duoc cookie vao profile that - chi luu cookie tho (Mo trinh duyet se chua dung duoc toi khi dan lai)"
            );
            Ok(serde_json::json!({ "mode": "cookie" }))
        }
    }
}

/// Thân `login_cookie_text` — (A) dán MỘT đoạn cookie (không qua file) cho nền tảng bất kỳ KHÁC
/// Dola (Dola có pipeline riêng — `account_import_cookies`): tiêm vào profile thật rồi lưu
/// credential kind `session`. Dùng bởi `AddAccountDialog` (phương thức "Dán cookie").
///
/// # Errors
/// - `COOKIE_INVALID`: không đọc được cookie nào từ `cookie_text`.
/// - `CRED_*`: `provider`/`kind`/`label`/`secret` không hợp lệ (xem [`lvc_vault::Vault::add`]).
pub async fn login_cookie_text_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    provider: &str,
    label: &str,
    cookie_text: &str,
    proxy: &str,
) -> Result<CredentialView, IpcError> {
    let meta = provision_meta(ctx, provider, label, cookie_text, proxy).await?;
    let req = AddCredential {
        provider: provider.to_string(),
        kind: "session".to_string(),
        label: label.to_string(),
        secret: cookie_text.to_string(),
        meta,
        proxy: proxy.to_string(),
    };
    Ok(ctx.vault.add(&req).await?)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn login_cookie_text(
    state: tauri::State<'_, AppState>,
    provider: String,
    label: String,
    cookie_text: String,
    proxy: String,
) -> Result<CredentialView, IpcError> {
    login_cookie_text_impl(&state, &provider, &label, &cookie_text, &proxy).await
}

/// Kết quả nạp MỘT file trong lô.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginImportItem {
    pub label: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// Nhãn an toàn từ tên file (bỏ đuôi).
fn label_from_path(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if stem.trim().is_empty() {
        "cookie".to_string()
    } else {
        stem
    }
}

/// Đọc nhiều file cookie (server-side) và thêm vào kho theo `provider` (kind `session`). Trả kết quả
/// từng file.
///
/// # Errors
/// Không lỗi cả lô (mỗi file có `ok`/`error` riêng).
pub async fn login_import_cookie_files_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    provider: &str,
    paths: &[String],
    proxy: &str,
) -> Result<Vec<LoginImportItem>, IpcError> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let label = label_from_path(path);
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => {
                out.push(LoginImportItem {
                    label,
                    ok: false,
                    error: Some(format!("đọc file lỗi: {e}")),
                });
                continue;
            }
        };
        let meta = match provision_meta(ctx, provider, &label, &content, proxy).await {
            Ok(m) => m,
            Err(e) => {
                out.push(LoginImportItem {
                    label,
                    ok: false,
                    error: Some(e.message),
                });
                continue;
            }
        };
        let req = AddCredential {
            provider: provider.to_string(),
            kind: "session".to_string(),
            label: label.clone(),
            secret: content,
            meta,
            proxy: proxy.to_string(),
        };
        match ctx.vault.add(&req).await {
            Ok(_) => out.push(LoginImportItem {
                label,
                ok: true,
                error: None,
            }),
            Err(e) => out.push(LoginImportItem {
                label,
                ok: false,
                error: Some(IpcError::from(e).message),
            }),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn login_import_cookie_files(
    state: tauri::State<'_, AppState>,
    provider: String,
    paths: Vec<String>,
    proxy: String,
) -> Result<Vec<LoginImportItem>, IpcError> {
    login_import_cookie_files_impl(&state, &provider, &paths, &proxy).await
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeStep;

    use super::*;
    use crate::commands::test_support::ctx_with;

    const COOKIE_TEXT: &str = ".google.com\tTRUE\t/\tTRUE\t1800000000\tSID\tabc123\n";

    #[tokio::test]
    async fn login_cookie_text_impl_thanh_cong_luu_profile_dir_trong_meta() {
        let (dir, ctx, _driver) = ctx_with(vec![FakeStep::Launch(Ok(()))], &[]).await;
        let view = login_cookie_text_impl(&ctx, "google", "Gmail acc1", COOKIE_TEXT, "")
            .await
            .unwrap();
        assert_eq!(view.provider, "google");
        assert_eq!(view.kind, "session");
        let expected_dir = dir
            .path()
            .join("accounts")
            .join(".logins")
            .join("google")
            .join("Gmail_acc1");
        assert_eq!(
            view.meta["profile_dir"],
            expected_dir.to_string_lossy().to_string()
        );
    }

    #[tokio::test]
    async fn login_cookie_text_impl_cookie_rong_khong_luu_credential() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = login_cookie_text_impl(&ctx, "google", "acc1", "khong-phai-cookie", "")
            .await
            .unwrap_err();
        assert_eq!(err.code, "COOKIE_INVALID");
        assert!(ctx.vault.list(None).await.unwrap().is_empty());
    }

    /// Phần A — lỗi trình duyệt (`BROWSER`/`PROFILE_BUSY`) KHÔNG được chặn lưu cookie: vẫn lưu
    /// credential, chỉ thiếu `profile_dir` trong `meta` (phương án lùi, xem doc `provision_meta`).
    #[tokio::test]
    async fn login_cookie_text_impl_loi_trinh_duyet_van_luu_credential_thieu_profile_dir() {
        let (_dir, ctx, _driver) = ctx_with(
            vec![FakeStep::Launch(Err(
                lvc_browser::BrowserError::ProfileBusy("dang ban".into()),
            ))],
            &[],
        )
        .await;
        let view = login_cookie_text_impl(&ctx, "google", "acc1", COOKIE_TEXT, "")
            .await
            .unwrap();
        assert_eq!(view.meta.get("profile_dir"), None);
    }

    #[tokio::test]
    async fn login_import_cookie_files_impl_tiem_profile_cho_tung_file() {
        let (dir, ctx, _driver) = ctx_with(
            vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))],
            &[],
        )
        .await;
        let f1 = dir.path().join("a.txt");
        let f2 = dir.path().join("b.txt");
        tokio::fs::write(&f1, COOKIE_TEXT).await.unwrap();
        tokio::fs::write(&f2, COOKIE_TEXT).await.unwrap();
        let items = login_import_cookie_files_impl(
            &ctx,
            "facebook",
            &[
                f1.to_string_lossy().to_string(),
                f2.to_string_lossy().to_string(),
            ],
            "",
        )
        .await
        .unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.ok), "{items:?}");
        let list = ctx.vault.list(Some("facebook")).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(
            list.iter().all(|v| v.meta.get("profile_dir").is_some()),
            "{list:?}"
        );
    }
}
