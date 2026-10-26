//! Lệnh Tauri cổng bản quyền LVC Shop (bọc `lvc-license`): đăng nhập tài khoản → kích hoạt license →
//! auto-login offline từ session đã cache. Bí mật (password/license/JWT/session) CHỈ đi VÀO; ra IPC
//! chỉ trạng thái đã che. Xem quy ước `xxx_impl`/`xxx` ở `commands/mod.rs`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use lvc_license::client::{LicenseClient, LicenseError};
use lvc_license::integrity::embedded_key_intact;
use lvc_license::session::{Session, load_session, save_session, touch_session};
use lvc_license::verify::parse_iso_to_unix;
use serde::Serialize;

use super::IpcError;

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Trạng thái cổng bản quyền (giữ qua `app.manage`). `auth` = `(JWT token, username)` sau khi đăng
/// nhập — CHỈ trong bộ nhớ (không ghi đĩa; session kích hoạt mới ghi `session_path`).
pub struct LicenseState {
    pub client: LicenseClient,
    pub session_path: PathBuf,
    pub salt: String,
    pub auth: Mutex<Option<(String, String)>>,
    /// Quyền dùng tool đã được BACKEND xác nhận (chỉ trong bộ nhớ). Đây mới là nguồn sự thật cho
    /// cổng [`LicenseState::require_licensed`] — màn `LicenseGate` ở giao diện chỉ là lớp hiển thị,
    /// vá giao diện không cấp được quyền này.
    grant: Mutex<Option<Grant>>,
}

/// Quyền đã xác nhận. Dùng đồng hồ ĐƠN ĐIỆU (`Instant`) tính từ lúc cấp, nên sau khi cấp, chỉnh
/// đồng hồ hệ thống (lùi giờ để kéo dài hạn) không làm quyền sống lâu hơn.
struct Grant {
    username: String,
    /// Chuỗi ISO nguyên văn của server, trả lại cho UI.
    expires_at: String,
    expires_unix: i64,
    granted_at_unix: i64,
    granted_at: Instant,
}

impl Grant {
    fn new(username: &str, expires_at: &str, expires_unix: i64) -> Self {
        Self {
            username: username.to_string(),
            expires_at: expires_at.to_string(),
            expires_unix,
            granted_at_unix: now_unix(),
            granted_at: Instant::now(),
        }
    }

    /// Hiện tại theo đồng hồ đơn điệu: mốc cấp + thời gian đã trôi (không phụ thuộc đồng hồ hệ thống).
    fn monotonic_now(&self) -> i64 {
        let elapsed = i64::try_from(self.granted_at.elapsed().as_secs()).unwrap_or(i64::MAX);
        self.granted_at_unix.saturating_add(elapsed)
    }

    fn still_valid(&self) -> bool {
        self.monotonic_now() < self.expires_unix
    }
}

impl LicenseState {
    #[must_use]
    pub fn new(client: LicenseClient, session_path: PathBuf, salt: impl Into<String>) -> Self {
        Self {
            client,
            session_path,
            salt: salt.into(),
            auth: Mutex::new(None),
            grant: Mutex::new(None),
        }
    }

    fn set_grant(&self, username: &str, expires_at: &str) {
        let grant = parse_iso_to_unix(expires_at).map(|exp| Grant::new(username, expires_at, exp));
        *self.grant.lock().expect("license grant mutex") = grant;
    }

    /// Quyền đang còn hiệu lực trong bộ nhớ (kèm tên + hạn để trả UI), nếu có.
    fn live_grant(&self) -> Option<(String, String)> {
        self.grant
            .lock()
            .expect("license grant mutex")
            .as_ref()
            .filter(|g| g.still_valid())
            .map(|g| (g.username.clone(), g.expires_at.clone()))
    }

    fn clear_grant(&self) {
        *self.grant.lock().expect("license grant mutex") = None;
    }

    /// Cổng backend: lệnh làm việc thật (chạy workflow, kịch bản, dựng video…) gọi hàm này TRƯỚC khi
    /// làm gì. Từ chối khi chưa kích hoạt, license đã hết hạn giữa chừng, hoặc public key nhúng bị
    /// đổi. Thông báo cố ý chung chung, không mách nước lớp nào chặn.
    ///
    /// # Errors
    /// [`IpcError`] mã `LICENSE_REQUIRED`.
    pub fn require_licensed(&self) -> Result<(), IpcError> {
        let ok = embedded_key_intact()
            && self
                .grant
                .lock()
                .expect("license grant mutex")
                .as_ref()
                .is_some_and(Grant::still_valid);
        if ok {
            Ok(())
        } else {
            Err(IpcError::new(
                "LICENSE_REQUIRED",
                "Cần kích hoạt bản quyền để dùng chức năng này",
            ))
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

/// Trạng thái hiện tại. Thứ tự tin cậy:
///  1. quyền ĐANG CÓ trong bộ nhớ backend (vừa kích hoạt, kể cả khi không "ghi nhớ" nên không có
///     file session) — dùng đồng hồ đơn điệu nên chỉnh giờ hệ thống không kéo dài được;
///  2. session đã cache trên đĩa (auto-login OFFLINE lúc mở app), phải qua kiểm tra thời gian
///     (hạn license, offline tối đa, lùi đồng hồ) → cấp quyền cho backend;
///  3. không có gì → chưa kích hoạt, thu hồi quyền.
///
/// Không gọi server. Public key nhúng bị đổi → luôn chưa kích hoạt.
#[must_use]
pub fn license_status_impl(st: &LicenseState) -> LicenseStatusView {
    let off = LicenseStatusView {
        activated: false,
        username: None,
        expires_at: None,
    };
    if !embedded_key_intact() {
        st.clear_grant();
        return off;
    }
    let now = now_unix();
    // Ghi mốc đồng hồ đã thấy (chỉ tiến) để lần mở sau phát hiện lùi giờ. No-op khi không có file.
    touch_session(&st.session_path, &st.salt, now);
    if let Some((username, expires_at)) = st.live_grant() {
        return LicenseStatusView {
            activated: true,
            username: Some(username),
            expires_at: Some(expires_at),
        };
    }
    if let Some(s) = load_session(&st.session_path, &st.salt, now) {
        st.set_grant(&s.username, &s.license.expires_at);
        return LicenseStatusView {
            activated: true,
            username: Some(s.username),
            expires_at: Some(s.license.expires_at),
        };
    }
    st.clear_grant();
    off
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

/// Ghi session cache theo ô "Ghi nhớ tài khoản".
///
/// `remember == false` → KHÔNG ghi gì VÀ xoá bản cũ, nên lần mở app sau [`license_status_impl`] trả
/// `activated: false` và người dùng phải đăng nhập lại. Hàm thuần theo tham số (không đụng
/// [`LicenseState`]) để test độc lập, không cần server.
///
/// # Errors
/// [`IpcError`] `SESSION_SAVE` khi ghi file thất bại. Xoá file hỏng là best-effort, không báo lỗi.
fn persist_session(
    path: &Path,
    session: &Session,
    salt: &str,
    remember: bool,
) -> Result<(), IpcError> {
    if !remember {
        let _ = std::fs::remove_file(path);
        return Ok(());
    }
    save_session(path, session, salt).map_err(|e| IpcError::new("SESSION_SAVE", e.to_string()))
}

/// Kích hoạt license (bước 2) — cần đã đăng nhập. Verify phản hồi (4 lớp) → lưu session cache nếu
/// `remember`.
///
/// # Errors
/// [`IpcError`] `NOT_LOGGED_IN` nếu chưa đăng nhập; `NETWORK`/`VERIFY_FAILED`; `SESSION_SAVE`.
pub async fn license_activate_impl(
    st: &LicenseState,
    license_key: &str,
    remember: bool,
) -> Result<LicenseStatusView, IpcError> {
    // Public key nhúng bị đổi = file chạy đã bị vá: dừng như một phản hồi không xác minh được, không
    // nói rõ lý do.
    if !embedded_key_intact() {
        return Err(to_ipc(&LicenseError::VerifyFailed));
    }
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
        last_seen: now_unix(),
    };
    persist_session(&st.session_path, &session, &st.salt, remember)?;
    // Cấp quyền cho backend NGAY cả khi không "ghi nhớ" (không có file session): phiên hiện tại vẫn
    // dùng được tới hết hạn license, chỉ lần mở app sau mới phải đăng nhập lại.
    st.set_grant(&username, &info.expires_at);
    Ok(LicenseStatusView {
        activated: true,
        username: Some(username),
        expires_at: Some(info.expires_at),
    })
}

/// Đăng xuất — xoá session cache + token bộ nhớ (best-effort, không lỗi).
pub fn license_logout_impl(st: &LicenseState) {
    let _ = std::fs::remove_file(&st.session_path);
    *st.auth.lock().expect("license auth mutex") = None;
    st.clear_grant();
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan State so huu - bien gioi IPC"
)]
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
    remember: bool,
) -> Result<LicenseStatusView, IpcError> {
    license_activate_impl(&state, &license_key, remember).await
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan State so huu - bien gioi IPC"
)]
#[tauri::command]
pub fn license_logout(state: tauri::State<'_, LicenseState>) {
    license_logout_impl(&state);
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

    fn mau_session(username: &str) -> Session {
        Session {
            username: username.into(),
            license: LicenseInfo {
                key: "K".into(),
                expires_at: "2999-01-01T00:00:00Z".into(),
                tool_id: None,
                max_devices: Some(1),
                device_id: "D".into(),
            },
            device_id: "D".into(),
            saved_at: now_unix(),
            last_seen: 0,
        }
    }

    #[test]
    fn khong_ghi_nho_thi_khong_tao_file_session() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.enc");
        persist_session(&p, &mau_session("bob"), "salt", false).unwrap();
        assert!(!p.exists(), "khong tich ghi nho ma van ghi session");
    }

    #[test]
    fn khong_ghi_nho_thi_xoa_session_cu() {
        // Lan truoc co tich "ghi nho", lan nay bo tich -> phai quen han, khong auto-login nua.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.enc");
        save_session(&p, &mau_session("bob"), "salt").unwrap();
        persist_session(&p, &mau_session("bob"), "salt", false).unwrap();
        let st = LicenseState::new(lvc_license::default_client(), p, "salt");
        assert!(!license_status_impl(&st).activated);
    }

    #[test]
    fn co_ghi_nho_thi_lan_sau_auto_login() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.enc");
        persist_session(&p, &mau_session("bob"), "salt", true).unwrap();
        let st = LicenseState::new(lvc_license::default_client(), p, "salt");
        let v = license_status_impl(&st);
        assert!(v.activated);
        assert_eq!(v.username.as_deref(), Some("bob"));
    }

    fn state_tam(dir: &std::path::Path) -> LicenseState {
        LicenseState::new(lvc_license::default_client(), dir.join("s.enc"), "salt")
    }

    #[test]
    fn chua_kich_hoat_thi_cong_backend_chan() {
        let dir = tempfile::tempdir().unwrap();
        let st = state_tam(dir.path());
        let err = st.require_licensed().unwrap_err();
        assert_eq!(err.code, "LICENSE_REQUIRED");
    }

    #[test]
    fn co_grant_con_han_thi_cong_backend_mo() {
        let dir = tempfile::tempdir().unwrap();
        let st = state_tam(dir.path());
        st.set_grant("t", "2999-01-01T00:00:00Z");
        assert!(st.require_licensed().is_ok());
    }

    #[test]
    fn grant_het_han_giua_chung_thi_chan() {
        // License hết hạn SAU khi đã mở app: lệnh làm việc kế tiếp phải bị chặn, không đợi mở lại app.
        let dir = tempfile::tempdir().unwrap();
        let st = state_tam(dir.path());
        st.set_grant("t", "2000-01-01T00:00:00Z");
        assert_eq!(st.require_licensed().unwrap_err().code, "LICENSE_REQUIRED");
    }

    #[test]
    fn moc_het_han_khong_doc_duoc_thi_khong_cap_quyen() {
        let dir = tempfile::tempdir().unwrap();
        let st = state_tam(dir.path());
        st.set_grant("t", "khong-phai-ngay");
        assert!(st.require_licensed().is_err());
    }

    #[test]
    fn dang_xuat_thu_hoi_quyen_backend() {
        let dir = tempfile::tempdir().unwrap();
        let st = state_tam(dir.path());
        st.set_grant("t", "2999-01-01T00:00:00Z");
        license_logout_impl(&st);
        assert!(st.require_licensed().is_err());
    }

    #[test]
    fn status_voi_session_hop_le_cap_quyen_cho_backend() {
        // Vá giao diện không cấp được quyền: quyền chỉ đến từ session đã giải mã được ở backend.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.enc");
        save_session(&p, &mau_session("carol"), "salt").unwrap();
        let st = LicenseState::new(lvc_license::default_client(), p, "salt");
        assert!(
            st.require_licensed().is_err(),
            "chưa gọi status thì chưa có quyền"
        );
        assert!(license_status_impl(&st).activated);
        assert!(st.require_licensed().is_ok());
    }

    #[test]
    fn mo_app_khi_dong_ho_bi_lui_thi_khong_auto_login() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.enc");
        let mut s = mau_session("dave");
        s.last_seen = now_unix() + 86_400; // đã từng thấy đồng hồ ở "ngày mai" → giờ là lùi 1 ngày
        save_session(&p, &s, "salt").unwrap();
        let st = LicenseState::new(lvc_license::default_client(), p, "salt");
        // Mở app lần sau (chưa có quyền trong bộ nhớ) mà đồng hồ đã lùi → không auto-login được.
        assert!(!license_status_impl(&st).activated);
        assert!(st.require_licensed().is_err());
    }

    #[test]
    fn khong_ghi_nho_van_giu_quyen_trong_phien_hien_tai() {
        // Không tích "ghi nhớ" → không có file session. Kiểm tra trạng thái định kỳ của giao diện
        // KHÔNG được đá người dùng ra giữa chừng: quyền sống trong bộ nhớ backend.
        let dir = tempfile::tempdir().unwrap();
        let st = state_tam(dir.path());
        st.set_grant("erin", "2999-01-01T00:00:00Z");
        assert!(!st.session_path.exists());
        let v = license_status_impl(&st);
        assert!(v.activated);
        assert_eq!(v.username.as_deref(), Some("erin"));
        assert!(st.require_licensed().is_ok());
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
            saved_at: now_unix(),
            last_seen: 0,
        };
        save_session(&p, &session, "salt").unwrap();
        let st = LicenseState::new(lvc_license::default_client(), p, "salt");
        let v = license_status_impl(&st);
        assert!(v.activated);
        assert_eq!(v.username.as_deref(), Some("alice"));
    }
}
