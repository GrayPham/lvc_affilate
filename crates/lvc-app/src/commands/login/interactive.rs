//! (B) Đăng nhập THỦ CÔNG có hỗ trợ — ĐƯỜNG HỢP LỆ. Mở MỘT cửa sổ trình duyệt THẬT tới trang đăng
//! nhập của nền tảng để NGƯỜI DÙNG TỰ đăng nhập, rồi lưu một credential kind `session` tham chiếu
//! `profile_dir` bền vững (tái dùng bằng cách mở lại đúng thư mục đó).
//!
//! RANH GIỚI CỨNG (xem `login/mod.rs`, MEMORY `p4b-no-antibot-signing`): TUYỆT ĐỐI KHÔNG tự gõ
//! email/mật khẩu, KHÔNG tính TOTP/2FA, KHÔNG giải captcha, KHÔNG né phát hiện bot, KHÔNG port các
//! flow tự động của bản Python (`add_account.py`/`facebook_login.py`/`apple_login.py`). File này chỉ
//! mở-trình-duyệt-cho-người-dùng (`Engine::open_login_with_extract`) + đăng ký credential.
//!
//! Phần C: credential được tạo TRƯỚC khi mở cửa sổ (khác thứ tự CŨ — tạo SAU khi mở) để có sẵn
//! `id` truyền vào `on_cookies` — mỗi lần [`lvc_engine::login::CookieExtract`] đọc được cookie của
//! `url` trong lúc cửa sổ còn mở, secret của credential đó được GHI ĐÈ bằng cookie THẬT (JSON,
//! `vault.update_secret`) thay vì chỉ giữ marker `"manual_login"` như trước — `profile_dir` trong
//! `meta` vẫn là nguồn sự thật để mở lại phiên/chạy automation, cookie trong `secret` chỉ là bản ghi
//! thêm. Mở cửa sổ lỗi → xoá NGAY credential vừa tạo (giữ hành vi CŨ: chỉ lưu khi thành công thật
//! sự, không để lại một tài khoản "ma" không profile nào dùng được).

use std::sync::Arc;

use lvc_browser::BrowserDriver;
use lvc_core::cookie::Cookie;
use lvc_engine::login::safe_component;
use lvc_vault::AddCredential;
use tracing::warn;

use super::platform::login_url;
use crate::commands::{Ctx, IpcError};
use crate::state::AppState;

/// Kết quả mở đăng nhập thủ công — trả cho UI để hiện "đã lưu" + biết `profile_dir` đã dùng.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractiveLoginResult {
    /// Nhãn người dùng đặt (hiển thị).
    pub label: String,
    /// Nền tảng (= `provider` của credential).
    pub provider: String,
    /// Thư mục profile bền vững đã mở (để tái dùng — mở lại đúng thư mục này).
    pub profile_dir: String,
    /// Id credential vừa lưu vào kho.
    pub credential_id: String,
}

/// Thân `login_open_interactive` — kiểm nền tảng (suy URL) → tính `profile_dir` bền vững dưới
/// `accounts_dir/.logins/<platform>/<label an toàn>` (tiền tố `.` để `AccountPool` bỏ qua, không
/// nhầm thành tài khoản Dola) → đăng ký credential kind `session` (marker, có `id`) →
/// `Engine::open_login_with_extract` (mở cửa sổ THẬT cho người dùng tự đăng nhập, `on_cookies` ghi
/// đè secret bằng cookie thật mỗi lần đọc được — xem doc đầu module).
///
/// # Errors
/// - `LOGIN_BAD_PLATFORM`: `platform` không có trong bảng nền tảng đăng nhập ([`login_url`]).
/// - `CRED_*`/`STORE`: từ `ctx.vault.add` (nhãn/kind/provider không hợp lệ, lỗi mã hoá/lưu).
/// - `INVALID_PROXY`/`IO`/`BROWSER`: từ [`lvc_engine::Engine::open_login_with_extract`] (xem
///   `AccountError`) — credential vừa tạo bị XOÁ lại trước khi trả lỗi (xem doc đầu module).
pub async fn login_open_interactive_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    platform: &str,
    label: &str,
    proxy: &str,
) -> Result<InteractiveLoginResult, IpcError> {
    let url = login_url(platform).ok_or_else(|| {
        IpcError::new(
            "LOGIN_BAD_PLATFORM",
            format!("nền tảng đăng nhập không hỗ trợ: {platform}"),
        )
    })?;

    let profile_dir = ctx
        .engine
        .config()
        .accounts_dir
        .join(".logins")
        .join(safe_component(platform))
        .join(safe_component(label));
    let profile_dir_str = profile_dir.to_string_lossy().to_string();

    // Tao credential TRUOC (secret = marker, giong cu) de co `id` truyen vao `on_cookies` ngay khi
    // mo cua so — Engine can `id` nay TU LUC dau, khong the biet SAU khi mo (khac thu tu CU, xem
    // doc dau module).
    let marker_secret = serde_json::json!({
        "mode": "manual_login",
        "profile_dir": profile_dir_str,
    })
    .to_string();
    let req = AddCredential {
        provider: platform.to_string(),
        kind: "session".to_string(),
        label: label.to_string(),
        secret: marker_secret,
        meta: serde_json::json!({
            "mode": "cookie",
            "login": "manual",
            "profile_dir": profile_dir_str,
        }),
        proxy: proxy.trim().to_string(),
    };
    let view = ctx.vault.add(&req).await?;

    let vault = Arc::clone(&ctx.vault);
    let cred_id = view.id.clone();
    let on_cookies: Arc<dyn Fn(Vec<Cookie>) + Send + Sync> =
        Arc::new(move |cookies: Vec<Cookie>| {
            let vault = Arc::clone(&vault);
            let cred_id = cred_id.clone();
            // `on_tick` la closure DONG BO (goi truc tiep trong vong lap theo doi, khong `.await`) nen
            // ghi DB phai lui ra mot tac vu nen rieng — best-effort, loi chi canh bao (xem doc dau module).
            tokio::spawn(async move {
                let Ok(json) = serde_json::to_string(&cookies) else {
                    return;
                };
                if let Err(e) = vault.update_secret(&cred_id, &json).await {
                    warn!(
                        credential_id = %cred_id,
                        error = %e,
                        "khong luu duoc cookie trich xuat tu dang nhap thu cong (bo qua)"
                    );
                }
            });
        });

    // Mo cua so trinh duyet THAT — nguoi dung tu dang nhap; profile duoc ghi vao `profile_dir`
    // (khong doi), cookie THAT duoc trich xuat dinh ky va ghi de vao secret cua credential vua tao
    // (qua `on_cookies`).
    if let Err(e) = ctx
        .engine
        .open_login_with_extract(&profile_dir, url, proxy, on_cookies)
        .await
    {
        // Mo khong thanh cong -> don credential vua tao (giu hanh vi CU: chi luu khi THUC SU thanh
        // cong, tranh de lai mot tai khoan "ma" khong profile nao dung duoc).
        if let Err(del_err) = ctx.vault.delete(&view.id).await {
            warn!(
                credential_id = %view.id,
                error = %del_err,
                "khong don duoc credential sau khi mo dang nhap loi (bo qua)"
            );
        }
        return Err(e.into());
    }

    Ok(InteractiveLoginResult {
        label: view.label,
        provider: view.provider,
        profile_dir: profile_dir_str,
        credential_id: view.id,
    })
}

#[tauri::command]
pub async fn login_open_interactive(
    state: tauri::State<'_, AppState>,
    platform: String,
    label: String,
    proxy: String,
) -> Result<InteractiveLoginResult, IpcError> {
    login_open_interactive_impl(&state, &platform, &label, &proxy).await
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeStep;

    use super::*;
    use crate::commands::test_support::ctx_with;

    #[tokio::test]
    async fn mo_dang_nhap_luu_credential_session_tham_chieu_profile() {
        let (dir, ctx, _driver) = ctx_with(vec![FakeStep::Launch(Ok(()))], &[]).await;
        let res = login_open_interactive_impl(&ctx, "google", "Gmail acc1", "")
            .await
            .unwrap();
        assert_eq!(res.provider, "google");
        assert_eq!(res.label, "Gmail acc1");
        // profile_dir nằm dưới accounts_dir/.logins/google/<nhãn an toàn>.
        let expected = dir
            .path()
            .join("accounts")
            .join(".logins")
            .join("google")
            .join("Gmail_acc1");
        assert_eq!(res.profile_dir, expected.to_string_lossy());
        assert!(expected.is_dir(), "profile_dir phải được tạo");

        // credential đã lưu, KHÔNG lộ secret (chỉ hint đã che).
        let list = ctx.vault.list(Some("google")).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "session");
        assert_eq!(list[0].id, res.credential_id);
        let json = serde_json::to_string(&list).unwrap();
        assert!(
            !json.contains("manual_login"),
            "IPC không được lộ secret: {json}"
        );
    }

    /// Phan C (thu tu MOI — credential tao TRUOC khi mo cua so): mo cua so loi (vd `PROFILE_BUSY`)
    /// phai DON credential vua tao, giu nguyen hanh vi CU cua ca ham (chi luu khi mo THUC SU thanh
    /// cong, khong de lai tai khoan "ma"). `proxy_sai_bao_ma_dung_khong_luu_credential` duoi day da
    /// phu nhanh `INVALID_PROXY`; test nay phu nhanh LOI TU DRIVER (khac diem that bai trong luong).
    #[tokio::test]
    async fn mo_cua_so_loi_don_credential_vua_tao() {
        let (_dir, ctx, _driver) = ctx_with(
            vec![FakeStep::Launch(Err(
                lvc_browser::BrowserError::ProfileBusy("dang ban".into()),
            ))],
            &[],
        )
        .await;
        let err = login_open_interactive_impl(&ctx, "google", "Gmail acc1", "")
            .await
            .unwrap_err();
        assert_eq!(err.code, "PROFILE_BUSY");
        assert!(
            ctx.vault.list(None).await.unwrap().is_empty(),
            "credential vua tao phai duoc don khi mo cua so loi"
        );
    }

    #[tokio::test]
    async fn nen_tang_khong_ho_tro_bao_ma_dung() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = login_open_interactive_impl(&ctx, "khong-co", "x", "")
            .await
            .unwrap_err();
        assert_eq!(err.code, "LOGIN_BAD_PLATFORM");
    }

    #[tokio::test]
    async fn proxy_sai_bao_ma_dung_khong_luu_credential() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = login_open_interactive_impl(&ctx, "google", "acc", "khong-hop-le")
            .await
            .unwrap_err();
        assert_eq!(err.code, "INVALID_PROXY");
        assert!(ctx.vault.list(None).await.unwrap().is_empty());
    }
}
