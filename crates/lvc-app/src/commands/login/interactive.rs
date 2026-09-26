//! (B) Đăng nhập THỦ CÔNG có hỗ trợ — ĐƯỜNG HỢP LỆ. Mở MỘT cửa sổ trình duyệt THẬT tới trang đăng
//! nhập của nền tảng để NGƯỜI DÙNG TỰ đăng nhập, rồi lưu một credential kind `session` tham chiếu
//! `profile_dir` bền vững (tái dùng bằng cách mở lại đúng thư mục đó).
//!
//! RANH GIỚI CỨNG (xem `login/mod.rs`, MEMORY `p4b-no-antibot-signing`): TUYỆT ĐỐI KHÔNG tự gõ
//! email/mật khẩu, KHÔNG tính TOTP/2FA, KHÔNG giải captcha, KHÔNG né phát hiện bot, KHÔNG port các
//! flow tự động của bản Python (`add_account.py`/`facebook_login.py`/`apple_login.py`). File này chỉ
//! mở-trình-duyệt-cho-người-dùng (`Engine::open_login`) + đăng ký credential.

use lvc_browser::BrowserDriver;
use lvc_vault::AddCredential;

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

/// Một thành phần đường dẫn AN TOÀN từ chuỗi tự do (nhãn) — chỉ giữ chữ/số/`_`/`-`, còn lại thành
/// `_`; rỗng → `"acc"`. Dùng cho tên thư mục `profile_dir` (KHÁC nhãn hiển thị: nhãn giữ nguyên chuỗi
/// người dùng nhập để lưu vào credential).
fn safe_component(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('_').to_string();
    if cleaned.is_empty() {
        "acc".to_string()
    } else {
        cleaned
    }
}

/// Thân `login_open_interactive` — kiểm nền tảng (suy URL) → tính `profile_dir` bền vững dưới
/// `accounts_dir/.logins/<platform>/<label an toàn>` (tiền tố `.` để `AccountPool` bỏ qua, không
/// nhầm thành tài khoản Dola) → `Engine::open_login` (mở cửa sổ THẬT cho người dùng tự đăng nhập) →
/// đăng ký credential kind `session` tham chiếu `profile_dir`.
///
/// # Errors
/// - `LOGIN_BAD_PLATFORM`: `platform` không có trong bảng nền tảng đăng nhập ([`login_url`]).
/// - `INVALID_PROXY`/`IO`/`BROWSER`: từ [`lvc_engine::Engine::open_login`] (xem `AccountError`).
/// - `CRED_*`/`STORE`: từ `ctx.vault.add` (nhãn/kind/provider không hợp lệ, lỗi mã hoá/lưu).
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

    // Mở cửa sổ trình duyệt THẬT — người dùng tự đăng nhập; profile được ghi vào `profile_dir`.
    ctx.engine.open_login(&profile_dir, url, proxy).await?;

    let profile_dir_str = profile_dir.to_string_lossy().to_string();
    // Bí mật kind `session` KHÔNG rỗng (vault bắt buộc) nhưng phiên THẬT nằm ở `profile_dir` trên
    // đĩa, không phải trong secret — secret chỉ là marker tham chiếu profile.
    let secret = serde_json::json!({
        "mode": "manual_login",
        "profile_dir": profile_dir_str,
    })
    .to_string();
    let req = AddCredential {
        provider: platform.to_string(),
        kind: "session".to_string(),
        label: label.to_string(),
        secret,
        meta: serde_json::json!({
            "mode": "cookie",
            "login": "manual",
            "profile_dir": profile_dir_str,
        }),
        proxy: proxy.trim().to_string(),
    };
    let view = ctx.vault.add(&req).await?;

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
