//! Lệnh Tauri cổng bản quyền LVC Shop (bọc `lvc-license`): đăng nhập tài khoản → kích hoạt license →
//! auto-login offline từ session đã cache. Bí mật (password/license/JWT/session) CHỈ đi VÀO; ra IPC
//! chỉ trạng thái đã che. Xem quy ước `xxx_impl`/`xxx` ở `commands/mod.rs`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use lvc_license::client::{LicenseClient, LicenseError};
use lvc_license::session::{Session, load_session, save_session};
use serde::Serialize;

use super::IpcError;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Trạng thái cổng bản quyền (giữ qua `app.manage`). `auth` = `(JWT token, username)` sau khi đăng
/// nhập — CHỈ trong bộ nhớ (không ghi đĩa; session kích hoạt mới ghi `session_path`).
pub struct LicenseState {
    pub client: LicenseClient,
    pub session_path: PathBuf,
    pub salt: String,
    pub auth: Mutex<Option<(String, String)>>,
}

impl LicenseState {
    #[must_use]
    pub fn new(client: LicenseClient, session_path: PathBuf, salt: impl Into<String>) -> Self {
        Self {
            client,
            session_path,
            salt: salt.into(),
            auth: Mutex::new(None),
        }
    }
}

/// Trạng thái bản quyền trả ra UI (KHÔNG chứa token/license key).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatusView {
    pub activated: bool,
    pub username: Option<String>,
    pub expires_at: Option<String>,
}

fn to_ipc(e: &LicenseError) -> IpcError {
    let code = match e {
        LicenseError::Network => "NETWORK",
        LicenseError::BadCredentials => "BAD_CREDENTIALS",
        LicenseError::VerifyFailed => "VERIFY_FAILED",
    };
    IpcError::new(code, e.to_string())
}

/// Trạng thái hiện tại: auto-login OFFLINE từ session đã cache (còn hạn). Không gọi server.
#[must_use]
pub fn license_status_impl(st: &LicenseState) -> LicenseStatusView {
    match load_session(&st.session_path, &st.salt, now_unix()) {
        Some(s) => LicenseStatusView {
            activated: true,
            username: Some(s.username),
            expires_at: Some(s.license.expires_at),
        },
        None => LicenseStatusView {
            activated: false,
            username: None,
            expires_at: None,
        },
    }
}

/// Đăng nhập tài khoản (bước 1) — lưu token vào bộ nhớ để bước kích hoạt dùng.
///
/// # Errors
/// [`IpcError`] `NETWORK`/`BAD_CREDENTIALS` (xem [`LicenseError`]).
pub async fn license_login_impl(
    st: &LicenseState,
    username: &str,
    password: &str,
) -> Result<(), IpcError> {
    let token = st
        .client
        .authenticate(username, password)
        .await
        .map_err(|e| to_ipc(&e))?;
    *st.auth.lock().expect("license auth mutex") = Some((token, username.to_string()));
    Ok(())
}

/// Kích hoạt license (bước 2) — cần đã đăng nhập. Verify phản hồi (4 lớp) → lưu session cache.
///
/// # Errors
/// [`IpcError`] `NOT_LOGGED_IN` nếu chưa đăng nhập; `NETWORK`/`VERIFY_FAILED`; `SESSION_SAVE`.
pub async fn license_activate_impl(
    st: &LicenseState,
    license_key: &str,
) -> Result<LicenseStatusView, IpcError> {
    let creds = st.auth.lock().expect("license auth mutex").clone();
    let (token, username) =
        creds.ok_or_else(|| IpcError::new("NOT_LOGGED_IN", "Chưa đăng nhập tài khoản"))?;
    let info = st
        .client
        .verify_license(license_key, &token, &username)
        .await
        .map_err(|e| to_ipc(&e))?;
    let session = Session {
        username: username.clone(),
        license: info.clone(),
        device_id: st.client.device_id().to_string(),
        saved_at: now_unix(),
    };
    save_session(&st.session_path, &session, &st.salt)
        .map_err(|e| IpcError::new("SESSION_SAVE", e.to_string()))?;
    Ok(LicenseStatusView {
        activated: true,
        username: Some(username),
        expires_at: Some(info.expires_at),
    })
}

/// Đăng xuất — xoá session cache + token bộ nhớ.
///
/// # Errors
/// Không lỗi (xoá file best-effort).
pub fn license_logout_impl(st: &LicenseState) -> Result<(), IpcError> {
    let _ = std::fs::remove_file(&st.session_path);
    *st.auth.lock().expect("license auth mutex") = None;
    Ok(())
}

#[tauri::command]
pub fn license_status(state: tauri::State<'_, LicenseState>) -> LicenseStatusView {
    license_status_impl(&state)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn license_login(
    state: tauri::State<'_, LicenseState>,
    username: String,
    password: String,
) -> Result<(), IpcError> {
    license_login_impl(&state, &username, &password).await
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu - bien gioi IPC"
)]
#[tauri::command]
pub async fn license_activate(
    state: tauri::State<'_, LicenseState>,
    license_key: String,
) -> Result<LicenseStatusView, IpcError> {
    license_activate_impl(&state, &license_key).await
}

#[tauri::command]
pub fn license_logout(state: tauri::State<'_, LicenseState>) -> Result<(), IpcError> {
    license_logout_impl(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_license::verify::LicenseInfo;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn license_status_chua_kich_hoat_false() {
        let dir = tempfile::tempdir().unwrap();
        let st = LicenseState::new(
            lvc_license::default_client(),
            dir.path().join("s.enc"),
            "salt",
        );
        assert!(!license_status_impl(&st).activated);
    }

    #[tokio::test]
    async fn license_login_sai_tra_bad_credentials() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/auth/login"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let st = LicenseState::new(
            LicenseClient::new(server.uri(), "dummy", "lvc-shop"),
            dir.path().join("s.enc"),
            "salt",
        );
        let err = license_login_impl(&st, "u", "p").await.unwrap_err();
        assert_eq!(err.code, "BAD_CREDENTIALS");
    }

    #[test]
    fn license_status_sau_save_session_true() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.enc");
        let session = Session {
            username: "alice".into(),
            license: LicenseInfo {
                key: "K".into(),
                expires_at: "2999-01-01T00:00:00Z".into(),
                tool_id: None,
                max_devices: Some(1),
                device_id: "D".into(),
            },
            device_id: "D".into(),
            saved_at: 1,
        };
        save_session(&p, &session, "salt").unwrap();
        let st = LicenseState::new(lvc_license::default_client(), p, "salt");
        let v = license_status_impl(&st);
        assert!(v.activated);
        assert_eq!(v.username.as_deref(), Some("alice"));
    }
}
