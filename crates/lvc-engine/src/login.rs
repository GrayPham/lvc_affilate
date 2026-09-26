//! `Engine::open_login` (B — đăng nhập THỦ CÔNG có hỗ trợ) — mở MỘT cửa sổ trình duyệt THẬT
//! (`headless=false`) tới trang đăng nhập để NGƯỜI DÙNG TỰ đăng nhập, rồi giữ profile bền vững trên
//! đĩa (`profile_dir`) để tái dùng bằng cách mở lại đúng thư mục đó.
//!
//! RANH GIỚI CỨNG (xem MEMORY `p4b-no-antibot-signing` + `login/CLAUDE.md` của lvc-app): hàm này CHỈ
//! `launch` + `goto` tới URL đăng nhập. TUYỆT ĐỐI KHÔNG tự gõ email/mật khẩu, KHÔNG tính TOTP/2FA,
//! KHÔNG giải captcha, KHÔNG né phát hiện bot — mọi thao tác đăng nhập do CHÍNH người dùng làm trong
//! cửa sổ hiện ra. Vì vậy hàm nhận sẵn `url` (do lvc-app suy từ nền tảng, xem
//! `commands::login::platform::login_url`) và một `profile_dir` tuyệt đối — engine không biết khái
//! niệm "nền tảng".
//!
//! Tách file riêng theo LUẬT chia module (CLAUDE.md): `generate.rs` đã lớn; đây là một thao tác ADMIN
//! độc lập (bấm nút "Đăng nhập thủ công"), KHÔNG phải bước trong pipeline tạo video nên KHÔNG dùng
//! `SessionCache`/`Lease`/`CancellationToken` (giống lý do `accounts.rs` nêu ở đầu module đó).

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lvc_browser::{BrowserDriver, LaunchOpts};
use lvc_core::{BrowserKind, effective_proxy};
use tracing::{info, warn};

use crate::accounts::AccountError;
use crate::generate::Engine;

/// Nhịp hỏi "cửa sổ còn mở không" ([`BrowserDriver::is_connected`]) của tác vụ nền theo dõi phiên —
/// `#[cfg(test)]` nhỏ để test không phải chờ thật.
#[cfg(not(test))]
const WATCH_TICK: Duration = Duration::from_secs(2);
#[cfg(test)]
const WATCH_TICK: Duration = Duration::from_millis(10);

/// Giới hạn tối đa tác vụ nền giữ phiên (lưới an toàn — người dùng quên đóng cửa sổ thì không giữ
/// tiến trình Chrome mãi mãi). Phiên/profile ĐÃ được lưu lên đĩa ngay khi người dùng đăng nhập nên
/// đóng cửa sổ sau giới hạn này KHÔNG mất dữ liệu đăng nhập.
#[cfg(not(test))]
const WATCH_MAX: Duration = Duration::from_secs(60 * 60 * 3);
#[cfg(test)]
const WATCH_MAX: Duration = Duration::from_millis(50);

/// Tác vụ nền: giữ `session` sống (cửa sổ ở lại cho người dùng đăng nhập) cho tới khi người dùng
/// đóng cửa sổ ([`BrowserDriver::is_connected`] = `false`) HOẶC quá [`WATCH_MAX`], rồi đóng phiên để
/// dọn tiến trình. KHÔNG bao giờ chạm tới dữ liệu đăng nhập — chỉ theo dõi vòng đời cửa sổ.
async fn watch_until_closed<D: BrowserDriver + 'static>(driver: Arc<D>, session: D::Session) {
    let start = Instant::now();
    loop {
        tokio::time::sleep(WATCH_TICK).await;
        if !driver.is_connected(&session) {
            info!("open_login: người dùng đã đóng cửa sổ đăng nhập — dọn phiên");
            break;
        }
        if start.elapsed() >= WATCH_MAX {
            warn!(
                "open_login: quá thời gian giữ cửa sổ đăng nhập — tự đóng (dữ liệu đã lưu trên đĩa)"
            );
            break;
        }
    }
    if let Err(e) = driver.close(session).await {
        warn!(error = %e, "open_login: không đóng được phiên theo dõi (bỏ qua)");
    }
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Mở cửa sổ trình duyệt THẬT tới `url` với `profile_dir` bền vững để NGƯỜI DÙNG TỰ đăng nhập
    /// (đường HỢP LỆ — xem doc đầu module). Luồng: kiểm proxy (THUẦN, trước khi chạm đĩa/trình
    /// duyệt) → tạo `profile_dir` → `launch` (`headless=false`, cửa sổ HIỆN) → `goto(url)` → giao
    /// cửa sổ cho một tác vụ nền [`watch_until_closed`] (người dùng đăng nhập trong cửa sổ đó; phiên
    /// được ghi vào `profile_dir`). Trả về NGAY sau khi cửa sổ mở & điều hướng xong — caller
    /// (`lvc-app`) đăng ký một credential kind `session` tham chiếu `profile_dir` để tái dùng.
    ///
    /// `profile_dir` phải là đường dẫn TUYỆT ĐỐI đã được caller tính (lvc-app dùng
    /// `accounts_dir/.logins/<platform>/<label>` — tiền tố `.` để `AccountPool` bỏ qua, không nhầm
    /// thành tài khoản Dola). Engine chỉ tạo & dùng đúng thư mục đó.
    ///
    /// # Errors
    /// - [`AccountError::InvalidProxy`]: `proxy` sai định dạng ([`effective_proxy`]) hoặc (khi `proxy`
    ///   rỗng) proxy chung cấu hình cũng sai.
    /// - [`AccountError::Io`]: không tạo được `profile_dir`.
    /// - [`AccountError::Browser`]: `launch`/`goto` lỗi (thiếu Chrome, profile đang bị khoá, CDP lỗi).
    pub async fn open_login(
        &self,
        profile_dir: &Path,
        url: &str,
        proxy: &str,
    ) -> Result<(), AccountError>
    where
        // Tác vụ nền `watch_until_closed` (qua `tokio::spawn`) sở hữu `D::Session` nên nó phải
        // `'static` — `ChromiumSession`/`FakeSession` (driver thật/giả) đều thoả.
        D::Session: 'static,
    {
        let proxy = proxy.trim();
        // Kiểm THUẦN trước — sai proxy thì không tạo rác thư mục / không mở trình duyệt.
        let proxy_spec = effective_proxy(proxy, &self.cfg.global_proxy)
            .map_err(|e| AccountError::InvalidProxy(e.to_string()))?;

        tokio::fs::create_dir_all(profile_dir).await?;

        let opts = LaunchOpts {
            profile_dir: profile_dir.to_path_buf(),
            browser: BrowserKind::Chromium,
            executable_override: self.cfg.executable_override.clone(),
            // headless=false = cửa sổ HIỆN cho người dùng tự thao tác (KHÁC import_cookies/verify —
            // hai hàm đó headless để chỉ đọc). window_offscreen mặc định false: cửa sổ phải NHÌN
            // THẤY được, đây là điểm mấu chốt của đăng nhập thủ công.
            headless: false,
            proxy: proxy_spec,
            logs_dir: self.cfg.logs_dir.clone(),
            ..LaunchOpts::default()
        };
        let session = self.driver.launch(&opts).await?;

        // goto lỗi -> đóng phiên vừa mở rồi báo lỗi (không để rơi lại tiến trình Chrome mồ côi).
        if let Err(e) = self.driver.goto(&session, url).await {
            if let Err(close_err) = self.driver.close(session).await {
                warn!(error = %close_err, "open_login: không đóng được phiên sau khi goto lỗi (bỏ qua)");
            }
            return Err(AccountError::Browser(e));
        }

        // Giao cửa sổ cho tác vụ nền — trả về NGAY để caller đăng ký credential; người dùng đăng
        // nhập trong cửa sổ, tác vụ nền dọn khi cửa sổ đóng.
        let driver = Arc::clone(&self.driver);
        tokio::spawn(watch_until_closed(driver, session));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use lvc_browser::{BrowserError, FakeCall, FakeStep};

    use super::*;
    use crate::generate::tests::fx;

    const URL: &str = "https://accounts.google.com/";

    #[tokio::test]
    async fn open_login_tao_thu_muc_va_launch_hien_cua_so() {
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;
        let profile_dir = f.accounts_dir().join(".logins").join("google").join("acc1");
        f.engine
            .open_login(&profile_dir, URL, "1.2.3.4:8080")
            .await
            .unwrap();
        assert!(profile_dir.is_dir(), "profile_dir phải được tạo");
        // launch được ghi ngay trong open_login (đồng bộ, trước khi spawn tác vụ nền).
        let calls = f.calls();
        assert_eq!(
            calls.first(),
            Some(&FakeCall::Launch {
                profile_dir,
                browser: BrowserKind::Chromium,
            })
        );
    }

    #[tokio::test]
    async fn open_login_proxy_sai_bao_loi_khong_tao_thu_muc() {
        let f = fx(&[], vec![]).await;
        let profile_dir = f.accounts_dir().join(".logins").join("google").join("acc1");
        let err = f
            .engine
            .open_login(&profile_dir, URL, "khong-hop-le")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "INVALID_PROXY");
        assert!(!profile_dir.exists(), "proxy sai thì không tạo rác thư mục");
    }

    #[tokio::test]
    async fn open_login_launch_loi_bao_ma_browser() {
        // FakeDriver::goto luôn Ok nên không ép được goto lỗi qua kịch bản; test đường launch lỗi
        // để phủ nhánh lỗi trình duyệt (goto lỗi đi CÙNG đường `?`/`From<BrowserError>`).
        let f = fx(
            &[],
            vec![FakeStep::Launch(Err(BrowserError::ProfileBusy(
                "đang bị Chrome khác dùng".into(),
            )))],
        )
        .await;
        let profile_dir = f.accounts_dir().join(".logins").join("apple").join("x");
        let err = f
            .engine
            .open_login(&profile_dir, URL, "")
            .await
            .unwrap_err();
        assert_eq!(err.code(), "PROFILE_BUSY");
    }
}
