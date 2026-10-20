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
use lvc_core::BrowserEngine;
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
    engine: BrowserEngine,
) -> Result<serde_json::Value, IpcError> {
    // Engine Firefox: ghi cookie thang vao cookies.sqlite + proxy vao user.js (khong CDP — xem
    // plan). Mat cookie KHONG CHAN luu credential (phuong an lui giong Chromium): chi canh bao, meta
    // thieu profile_dir. Cookie rong (COOKIE_INVALID) la loi cung.
    if engine == BrowserEngine::FirefoxPortable {
        return match ctx
            .engine
            .provision_firefox_profile(provider, label, cookie_text, proxy)
            .await
        {
            Ok(dir) => Ok(serde_json::json!({
                "mode": "cookie",
                "engine": "firefox_portable",
                "profile_dir": dir.to_string_lossy(),
            })),
            Err(e) if e.code() == "COOKIE_INVALID" => Err(e.into()),
            Err(e) => {
                warn!(provider, label, error = %e, "khong tiem duoc cookie vao profile Firefox - chi luu cookie tho");
                Ok(serde_json::json!({ "mode": "cookie", "engine": "firefox_portable" }))
            }
        };
    }
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

/// Tag "Dùng cho node" MẶC ĐỊNH theo nhà cung cấp — để cột "Dùng cho node" không TRỐNG ngay khi
/// vừa thêm tài khoản cookie (người dùng vẫn sửa lại được qua `credential_set_node_types`). Giá trị
/// khớp tag hợp lệ của [`lvc_vault::Vault::set_node_types`] (`"video"`/`"image"`/`"script"`).
fn default_node_types(provider: &str) -> Vec<&'static str> {
    match provider {
        "museai" => vec!["video", "image"],
        "dola" => vec!["video"],
        _ => Vec::new(),
    }
}

/// Chèn `node_types` mặc định vào `meta` (object) nếu CHƯA có khoá đó — không ghi đè lựa chọn người
/// dùng. Trả lại `meta` đã bổ sung.
fn with_default_node_types(mut meta: serde_json::Value, provider: &str) -> serde_json::Value {
    let defaults = default_node_types(provider);
    if !defaults.is_empty() {
        if let Some(obj) = meta.as_object_mut() {
            obj.entry("node_types".to_string())
                .or_insert_with(|| serde_json::json!(defaults));
        }
    }
    meta
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
    engine: BrowserEngine,
) -> Result<CredentialView, IpcError> {
    let meta = provision_meta(ctx, provider, label, cookie_text, proxy, engine).await?;
    let req = AddCredential {
        provider: provider.to_string(),
        kind: "session".to_string(),
        label: label.to_string(),
        secret: cookie_text.to_string(),
        meta: with_default_node_types(meta, provider),
        proxy: proxy.to_string(),
    };
    Ok(ctx.vault.add(&req).await?)
}

/// Thân `login_cookie_relogin` — "Đăng nhập lại" cho MỘT credential cookie đã tồn tại: tiêm cookie
/// MỚI vào ĐÚNG profile cũ (giữ `provider`/`label`/`proxy`/`engine`), rồi ghi đè secret + `meta`
/// (`profile_dir` mới) nhưng GIỮ `node_types` người dùng đã đặt. Trả view đã cập nhật.
///
/// # Errors
/// - `NOT_FOUND`: không có credential `id`.
/// - `COOKIE_INVALID`: `cookie_text` không đọc được cookie nào.
/// - `CRED_*`/`VAULT_*`: lỗi ghi lại secret/meta.
pub async fn login_cookie_relogin_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    cookie_text: &str,
) -> Result<CredentialView, IpcError> {
    let view =
        ctx.vault.get_view(id).await?.ok_or_else(|| {
            IpcError::new("NOT_FOUND", "khong tim thay tai khoan de dang nhap lai")
        })?;
    // Proxy THÔ (chưa che) + engine lấy từ meta — provision cần proxy thật, không phải bản che.
    let (_provider, _dir, proxy, engine_str) = ctx.vault.profile_dir_and_proxy(id).await?;
    let engine =
        lvc_core::BrowserEngine::parse(&engine_str).unwrap_or(lvc_core::BrowserEngine::Chromium);
    let meta = provision_meta(
        ctx,
        &view.provider,
        &view.label,
        cookie_text,
        &proxy,
        engine,
    )
    .await?;
    // Giữ node_types người dùng đã đặt (nếu có); không thì rơi về mặc định theo provider.
    let meta = match view.meta.get("node_types") {
        Some(nt) => {
            let mut m = meta;
            if let Some(obj) = m.as_object_mut() {
                obj.insert("node_types".to_string(), nt.clone());
            }
            m
        }
        None => with_default_node_types(meta, &view.provider),
    };
    ctx.vault.update_secret_meta(id, cookie_text, &meta).await?;
    ctx.vault
        .get_view(id)
        .await?
        .ok_or_else(|| IpcError::new("NOT_FOUND", "tai khoan bien mat sau khi dang nhap lai"))
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
    engine: String,
) -> Result<CredentialView, IpcError> {
    let engine = BrowserEngine::parse(&engine)
        .map_err(|e| IpcError::new("INVALID_ENGINE", e.to_string()))?;
    login_cookie_text_impl(&state, &provider, &label, &cookie_text, &proxy, engine).await
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn login_cookie_relogin(
    state: tauri::State<'_, AppState>,
    id: String,
    cookie_text: String,
) -> Result<CredentialView, IpcError> {
    login_cookie_relogin_impl(&state, &id, &cookie_text).await
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
        // Nap tu FILE giu nguyen engine Chromium (luong nay chua co chon engine tren UI).
        let meta = match provision_meta(
            ctx,
            provider,
            &label,
            &content,
            proxy,
            BrowserEngine::Chromium,
        )
        .await
        {
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
            meta: with_default_node_types(meta, provider),
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

/// DÁN HÀNG LOẠT: mỗi dòng một tài khoản `email|...|cookies` (tự tách cookie + nhãn, xem
/// `lvc_core::cookie::extract_bulk_cookie_lines`). Mỗi dòng tiêm vào profile riêng + lưu credential
/// `session`. Một dòng lỗi KHÔNG dừng cả lô (có `ok`/`error` riêng).
///
/// # Errors
/// `IpcError` mã `NO_COOKIE_LINES` khi không tách được dòng cookie nào.
pub async fn login_cookie_text_bulk_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    provider: &str,
    text: &str,
    proxy: &str,
    engine: BrowserEngine,
) -> Result<Vec<LoginImportItem>, IpcError> {
    let lines = lvc_core::cookie::extract_bulk_cookie_lines(text);
    if lines.is_empty() {
        return Err(IpcError::new(
            "NO_COOKIE_LINES",
            "khong tach duoc tai khoan nao (moi dong: email|...|cookies, hoac chi cookie)",
        ));
    }
    let mut out = Vec::with_capacity(lines.len());
    for (label, cookie_str) in lines {
        match login_cookie_text_impl(ctx, provider, &label, &cookie_str, proxy, engine).await {
            Ok(_) => out.push(LoginImportItem {
                label,
                ok: true,
                error: None,
            }),
            Err(e) => out.push(LoginImportItem {
                label,
                ok: false,
                error: Some(e.message),
            }),
        }
    }
    Ok(out)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn login_cookie_text_bulk(
    state: tauri::State<'_, AppState>,
    provider: String,
    text: String,
    proxy: String,
    engine: String,
) -> Result<Vec<LoginImportItem>, IpcError> {
    let engine = BrowserEngine::parse(&engine)
        .map_err(|e| IpcError::new("INVALID_ENGINE", e.to_string()))?;
    login_cookie_text_bulk_impl(&state, &provider, &text, &proxy, engine).await
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
        let view = login_cookie_text_impl(
            &ctx,
            "google",
            "Gmail acc1",
            COOKIE_TEXT,
            "",
            BrowserEngine::Chromium,
        )
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
    async fn login_cookie_text_impl_firefox_ghi_cookies_sqlite_va_meta_engine() {
        // Engine Firefox KHONG dung driver (ghi thang cookies.sqlite) — khong can FakeStep::Launch.
        let (dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let view = login_cookie_text_impl(
            &ctx,
            "museai",
            "ff acc",
            COOKIE_TEXT,
            "",
            BrowserEngine::FirefoxPortable,
        )
        .await
        .unwrap();
        assert_eq!(view.meta["engine"], "firefox_portable");
        let profile_dir = dir
            .path()
            .join("accounts")
            .join(".logins")
            .join("firefox")
            .join("museai")
            .join("ff_acc");
        assert_eq!(
            view.meta["profile_dir"],
            profile_dir.to_string_lossy().to_string()
        );
        assert!(
            profile_dir.join("cookies.sqlite").is_file(),
            "phai ghi cookies.sqlite cho profile Firefox"
        );
    }

    #[tokio::test]
    async fn login_cookie_text_impl_cookie_rong_khong_luu_credential() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = login_cookie_text_impl(
            &ctx,
            "google",
            "acc1",
            "khong-phai-cookie",
            "",
            BrowserEngine::Chromium,
        )
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
        let view = login_cookie_text_impl(
            &ctx,
            "google",
            "acc1",
            COOKIE_TEXT,
            "",
            BrowserEngine::Chromium,
        )
        .await
        .unwrap();
        assert_eq!(view.meta.get("profile_dir"), None);
    }

    #[tokio::test]
    async fn museai_cookie_add_tu_gan_node_types_video_image() {
        // Feature "Dùng cho node": thêm cookie MuseAI phải tự gán node_types mặc định để cột không
        // trống. (COOKIE_TEXT là netscape .google.com — không ảnh hưởng node_types theo provider.)
        let (_dir, ctx, _driver) = ctx_with(vec![FakeStep::Launch(Ok(()))], &[]).await;
        let view = login_cookie_text_impl(
            &ctx,
            "museai",
            "acc1",
            COOKIE_TEXT,
            "",
            BrowserEngine::Chromium,
        )
        .await
        .unwrap();
        assert_eq!(
            view.meta["node_types"],
            serde_json::json!(["video", "image"]),
            "MuseAI cookie phai tu gan node_types video+image"
        );
    }

    #[tokio::test]
    async fn relogin_tiem_cookie_moi_cap_nhat_secret_va_giu_node_types() {
        // Feature "Đăng nhập lại": tiêm cookie mới vào ĐÚNG credential cũ (giữ node_types). 2 lần
        // provision (add + relogin) nên cần 2 FakeStep::Launch.
        let (_dir, ctx, _driver) = ctx_with(
            vec![FakeStep::Launch(Ok(())), FakeStep::Launch(Ok(()))],
            &[],
        )
        .await;
        let created = login_cookie_text_impl(
            &ctx,
            "museai",
            "acc1",
            COOKIE_TEXT,
            "",
            BrowserEngine::Chromium,
        )
        .await
        .unwrap();
        let id = created.id.clone();
        // Cookie Netscape khác (hạn mới) để phân biệt với COOKIE_TEXT ban đầu.
        let new_cookie = ".google.com\tTRUE\t/\tTRUE\t1900000000\tSID\txyz999\n";
        let after = login_cookie_relogin_impl(&ctx, &id, new_cookie)
            .await
            .unwrap();
        assert_eq!(after.id, id, "cung credential, khong tao moi");
        assert_eq!(
            after.meta["node_types"],
            serde_json::json!(["video", "image"]),
            "node_types phai duoc giu qua dang nhap lai"
        );
        assert!(
            after.meta.get("profile_dir").is_some(),
            "dang nhap lai phai tiem lai profile"
        );
        // Chỉ còn ĐÚNG 1 credential (relogin cập nhật tại chỗ).
        assert_eq!(ctx.vault.list(Some("museai")).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn relogin_id_khong_ton_tai_bao_not_found() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = login_cookie_relogin_impl(&ctx, "khong-co", COOKIE_TEXT)
            .await
            .unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");
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
