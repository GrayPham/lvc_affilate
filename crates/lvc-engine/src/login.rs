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
use lvc_core::cookie::Cookie;
use lvc_core::{BrowserKind, effective_proxy};
use tracing::{info, warn};

use crate::accounts::AccountError;
use crate::generate::Engine;

/// Một thành phần đường dẫn AN TOÀN từ chuỗi tự do (nhãn/nền tảng) — chỉ giữ chữ/số/`_`/`-`, còn
/// lại thành `_`; rỗng → `"acc"`. Dùng cho tên thư mục `profile_dir` dưới `.logins/<provider>/<label>`
/// (tiền tố `.` để `AccountPool` bỏ qua, không nhầm thành tài khoản Dola) — KHÁC nhãn hiển thị
/// (nhãn giữ nguyên chuỗi người dùng nhập để lưu vào credential). Dùng chung bởi
/// `lvc-app::commands::login::interactive` (B — đăng nhập thủ công) và `crate::profiles` (A —
/// tiêm cookie → profile cho mọi nền tảng khác Dola).
#[must_use]
pub fn safe_component(s: &str) -> String {
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

/// Gói URL + callback cho Phần C (trích xuất cookie định kỳ trong lúc cửa sổ đăng nhập thủ công còn
/// mở, thay vì chỉ giữ `profile_dir`) — `url` để [`BrowserDriver::cookies`] biết đọc cookie của
/// trang nào (giống `goto`), `on_tick` được gọi mỗi khi đọc thành công (lỗi đọc giữa chừng — trang
/// có thể đang chuyển hướng — bị BỎ QUA, không coi là fatal, xem [`tick_extract`]).
pub(crate) struct CookieExtract {
    pub url: String,
    pub on_tick: Arc<dyn Fn(Vec<Cookie>) + Send + Sync>,
}

/// Đọc cookie hiện tại (nếu `extract` có mặt) rồi gọi `on_tick` — tách riêng vì
/// [`watch_until_closed`] gọi CHỖ NÀY hai lần (mỗi tick trong vòng lặp + một lần cuối ngay trước
/// `close`, không bỏ lỡ cookie vừa có lúc người dùng sắp đóng cửa sổ).
async fn tick_extract<D: BrowserDriver>(
    driver: &D,
    session: &D::Session,
    extract: Option<&CookieExtract>,
) {
    let Some(ex) = extract else {
        return;
    };
    if let Ok(cookies) = driver.cookies(session, &ex.url).await {
        (ex.on_tick)(cookies);
    }
}

/// Tác vụ nền: giữ `session` sống (cửa sổ ở lại cho người dùng đăng nhập/xem) cho tới khi người dùng
/// đóng cửa sổ ([`BrowserDriver::is_connected`] = `false`) HOẶC quá [`WATCH_MAX`], rồi đóng phiên để
/// dọn tiến trình. KHÔNG bao giờ chạm tới dữ liệu đăng nhập ngoài [`CookieExtract`] (Phần C, tuỳ
/// chọn, `None` ở MỌI nơi không cần trích xuất — hành vi cũ giữ nguyên).
///
/// `pub(crate)`: `accounts.rs::open_account_viewer` (Dola) tái dùng để giữ `Lease` SỐNG đến hết tác
/// vụ nền này (bọc trong closure `async move` riêng ở đó) — xem doc ở đó vì sao cần giữ lease; Dola
/// LUÔN truyền `extract: None` (pipeline Dola nằm riêng ở `accounts.rs`, không qua đây).
pub(crate) async fn watch_until_closed<D: BrowserDriver + 'static>(
    driver: Arc<D>,
    session: D::Session,
    extract: Option<CookieExtract>,
) {
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
        tick_extract(driver.as_ref(), &session, extract.as_ref()).await;
    }
    // Lan doc CUOI ngay truoc khi dong — xem doc `tick_extract`.
    tick_extract(driver.as_ref(), &session, extract.as_ref()).await;
    if let Err(e) = driver.close(session).await {
        warn!(error = %e, "open_login: không đóng được phiên theo dõi (bỏ qua)");
    }
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Thân dùng chung cho [`open_login`](Self::open_login) (đăng nhập thủ công, luôn có `url`) và
    /// [`open_profile_viewer`](Self::open_profile_viewer) (mở lại profile đã có để XEM, `url` tuỳ
    /// chọn — không ép điều hướng nếu không biết URL nào phù hợp). Luồng: kiểm proxy (THUẦN, trước
    /// khi chạm đĩa/trình duyệt) → tạo `profile_dir` → `launch` (`headless=false`, cửa sổ HIỆN) →
    /// `goto(url)` nếu có → giao cửa sổ cho một tác vụ nền [`watch_until_closed`]. Trả về NGAY sau
    /// khi cửa sổ mở (& điều hướng xong nếu có `url`).
    ///
    /// # Errors
    /// - [`AccountError::InvalidProxy`]: `proxy` sai định dạng ([`effective_proxy`]) hoặc (khi `proxy`
    ///   rỗng) proxy chung cấu hình cũng sai.
    /// - [`AccountError::Io`]: không tạo được `profile_dir`.
    /// - [`AccountError::Browser`]: `launch`/`goto` lỗi (thiếu Chrome, profile đang bị khoá, CDP lỗi).
    async fn open_headed(
        &self,
        profile_dir: &Path,
        url: Option<&str>,
        proxy: &str,
        extract: Option<CookieExtract>,
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
            // THẤY được, đây là điểm mấu chốt của đăng nhập thủ công/xem profile.
            headless: false,
            proxy: proxy_spec,
            logs_dir: self.cfg.logs_dir.clone(),
            ..LaunchOpts::default()
        };
        let session = self.driver.launch(&opts).await?;

        // goto lỗi -> đóng phiên vừa mở rồi báo lỗi (không để rơi lại tiến trình Chrome mồ côi).
        if let Some(url) = url {
            if let Err(e) = self.driver.goto(&session, url).await {
                if let Err(close_err) = self.driver.close(session).await {
                    warn!(error = %close_err, "open_headed: không đóng được phiên sau khi goto lỗi (bỏ qua)");
                }
                return Err(AccountError::Browser(e));
            }
        }

        // Giao cửa sổ cho tác vụ nền — trả về NGAY để caller đăng ký credential; người dùng thao tác
        // trong cửa sổ, tác vụ nền dọn khi cửa sổ đóng.
        let driver = Arc::clone(&self.driver);
        tokio::spawn(watch_until_closed(driver, session, extract));
        Ok(())
    }

    /// Mở cửa sổ trình duyệt THẬT tới `url` với `profile_dir` bền vững để NGƯỜI DÙNG TỰ đăng nhập
    /// (đường HỢP LỆ — xem doc đầu module). Phiên được ghi vào `profile_dir`; caller (`lvc-app`)
    /// đăng ký một credential kind `session` tham chiếu `profile_dir` để tái dùng. KHÔNG trích xuất
    /// cookie — dùng [`Self::open_login_with_extract`] nếu cần (Phần C).
    ///
    /// `profile_dir` phải là đường dẫn TUYỆT ĐỐI đã được caller tính (lvc-app dùng
    /// `accounts_dir/.logins/<platform>/<label>` — tiền tố `.` để `AccountPool` bỏ qua, không nhầm
    /// thành tài khoản Dola). Engine chỉ tạo & dùng đúng thư mục đó.
    ///
    /// # Errors
    /// Xem [`open_headed`](Self::open_headed).
    pub async fn open_login(
        &self,
        profile_dir: &Path,
        url: &str,
        proxy: &str,
    ) -> Result<(), AccountError>
    where
        D::Session: 'static,
    {
        self.open_headed(profile_dir, Some(url), proxy, None).await
    }

    /// Như [`Self::open_login`] nhưng THÊM `on_cookies` — gọi mỗi khi đọc được cookie của `url`
    /// trong lúc cửa sổ còn mở (định kỳ, cộng một lần cuối ngay trước khi đóng), xem
    /// [`CookieExtract`]/[`watch_until_closed`]. Phần C: `lvc-app::commands::login::interactive`
    /// dùng hàm này để lưu cookie THẬT vào credential thay vì chỉ giữ `profile_dir`.
    ///
    /// # Errors
    /// Xem [`open_headed`](Self::open_headed).
    pub async fn open_login_with_extract(
        &self,
        profile_dir: &Path,
        url: &str,
        proxy: &str,
        on_cookies: Arc<dyn Fn(Vec<Cookie>) + Send + Sync>,
    ) -> Result<(), AccountError>
    where
        D::Session: 'static,
    {
        self.open_headed(
            profile_dir,
            Some(url),
            proxy,
            Some(CookieExtract {
                url: url.to_string(),
                on_tick: on_cookies,
            }),
        )
        .await
    }

    /// Mở lại một `profile_dir` ĐÃ CÓ (Dola hoặc credential đã tiêm cookie — xem `profiles.rs`) ở
    /// chế độ HIỆN cửa sổ để người dùng tự xem/thao tác — "Mở trình duyệt" trong UI. KHÔNG ép điều
    /// hướng: `url` là gợi ý tuỳ chọn (vd `login::platform::login_url(provider)` nếu biết), `None`
    /// thì chỉ mở trình duyệt tới trang mặc định của chính profile đó.
    ///
    /// # Errors
    /// Xem [`open_headed`](Self::open_headed).
    pub async fn open_profile_viewer(
        &self,
        profile_dir: &Path,
        url: Option<&str>,
        proxy: &str,
    ) -> Result<(), AccountError>
    where
        D::Session: 'static,
    {
        self.open_headed(profile_dir, url, proxy, None).await
    }
}

#[cfg(test)]
mod tests {
    use lvc_browser::{BrowserError, FakeCall, FakeStep};

    use super::*;
    use crate::generate::tests::fx;

    const URL: &str = "https://accounts.google.com/";

    #[test]
    fn safe_component_loc_ky_tu_la_va_rong() {
        assert_eq!(safe_component("Gmail acc1"), "Gmail_acc1");
        assert_eq!(safe_component("../../etc"), "etc");
        assert_eq!(safe_component(""), "acc");
        assert_eq!(safe_component("   "), "acc");
        assert_eq!(safe_component("a-b_C9"), "a-b_C9");
    }

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

    /// Phan C: `on_cookies` phai duoc goi (voi noi dung THUC SU, khong rong — xem doc
    /// `FakeDriver::cookies`) trong luc tac vu nen theo doi cua so con chay. `WATCH_MAX`/`WATCH_TICK`
    /// o che do test rat ngan (50ms/10ms) nen cho mot khoang du de tac vu nen tu dong qua gio va
    /// dong (giong quy uoc cho reaper trong `session_cache.rs`).
    #[tokio::test]
    async fn open_login_with_extract_goi_on_cookies_truoc_khi_dong() {
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;
        let profile_dir = f.accounts_dir().join(".logins").join("google").join("acc1");
        let seen: Arc<std::sync::Mutex<Vec<Vec<Cookie>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen2 = Arc::clone(&seen);
        f.engine
            .open_login_with_extract(
                &profile_dir,
                URL,
                "",
                Arc::new(move |cookies| seen2.lock().unwrap().push(cookies)),
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        let calls = seen.lock().unwrap();
        assert!(
            !calls.is_empty(),
            "on_cookies phai duoc goi it nhat mot lan"
        );
        assert!(
            calls.iter().all(|c| !c.is_empty()),
            "FakeDriver::cookies tra ve cookie gia khong rong — on_cookies phai nhan duoc"
        );
    }

    #[tokio::test]
    async fn open_profile_viewer_khong_url_van_mo_duoc_khong_goto() {
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;
        let profile_dir = f.accounts_dir().join("acc1");
        f.engine
            .open_profile_viewer(&profile_dir, None, "")
            .await
            .unwrap();
        assert!(profile_dir.is_dir(), "profile_dir phải được tạo");
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
    async fn open_profile_viewer_co_url_thi_goto() {
        let f = fx(&[], vec![FakeStep::Launch(Ok(()))]).await;
        let profile_dir = f.accounts_dir().join("acc1");
        f.engine
            .open_profile_viewer(&profile_dir, Some(URL), "")
            .await
            .unwrap();
        assert!(profile_dir.is_dir(), "profile_dir phải được tạo");
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
