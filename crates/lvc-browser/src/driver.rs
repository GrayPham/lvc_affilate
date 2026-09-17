//! Trait `BrowserDriver` (ADR-R002): moi thao tac trinh duyet di qua day de doi duoc
//! sang driver khac neu chromiumoxide bi Dola chan.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use lvc_core::{BrowserKind, Cookie, ProxySpec, SessionState};

use crate::BrowserError;

/// Mac dinh `launch_timeout` (chromiumoxide cho websocket URL cua tien trinh Chrome).
pub const DEFAULT_LAUNCH_TIMEOUT: Duration = Duration::from_secs(20);
/// Mac dinh `navigation_timeout` (Python `page.goto(..., timeout=60000)`).
pub const DEFAULT_NAVIGATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Tuy chon mo trinh duyet cho mot tai khoan. `Debug` viet tay theo ruling P2 (kieu chua bi mat
/// phai che mat khau MOT CACH TUONG MINH tai bien giao dien): ban than `derive(Debug)` cung KHONG
/// lo gi vi `ProxySpec::fmt` da che `password` — impl tay chi de moi truong `LaunchOpts` doc
/// duoc dieu do va test `debug_khong_lo_mat_khau_proxy` ghim (review Task 5 M-8).
#[derive(Clone)]
pub struct LaunchOpts {
    /// `accounts/<name>/` - cung bo cuc ban Python de tai dung profile. Duoc ep TUYET DOI
    /// trong `ChromiumDriver::config()` (ADR-R012).
    pub profile_dir: PathBuf,
    /// Tu cot `accounts_meta.browser` (Task 4): quyet dinh binary nao duoc resolve
    /// (`executable::resolve_executable`), KHONG de chromiumoxide tu do (review P1 I-6).
    pub browser: BrowserKind,
    /// Ghi de duong dan file chay (vd env `LVC_CHROME`); phai ton tai, neu khong thi
    /// `BrowserError::ExecutableNotFound` - khong roi ve tu do.
    pub executable_override: Option<PathBuf>,
    pub headless: bool,
    /// Proxy rieng tai khoan. Task 5 chi dung `spec.server()` cho `--proxy-server`;
    /// user/pass (neu co) duoc Task 13 xu ly qua `Fetch.*`.
    pub proxy: Option<ProxySpec>,
    /// Dua cua so ra ngoai man hinh (-32000,-32000) nhu `SILENT_MODE` cua Python.
    pub window_offscreen: bool,
    /// Thoi gian toi da cho Chrome in websocket URL -> `BrowserConfigBuilder::launch_timeout`.
    pub launch_timeout: Duration,
    /// Thoi gian toi da cho `goto()` (Page.navigate + cho load).
    pub navigation_timeout: Duration,
}

impl Default for LaunchOpts {
    fn default() -> Self {
        Self {
            profile_dir: PathBuf::new(),
            browser: BrowserKind::Chromium,
            executable_override: None,
            headless: false,
            proxy: None,
            window_offscreen: false,
            launch_timeout: DEFAULT_LAUNCH_TIMEOUT,
            navigation_timeout: DEFAULT_NAVIGATION_TIMEOUT,
        }
    }
}

impl fmt::Debug for LaunchOpts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchOpts")
            .field("profile_dir", &self.profile_dir)
            .field("browser", &self.browser)
            .field("executable_override", &self.executable_override)
            .field("headless", &self.headless)
            // ProxySpec::fmt tu che password bang "***"; KHONG in thang tung truong cua no.
            .field("proxy", &self.proxy)
            .field("window_offscreen", &self.window_offscreen)
            .field("launch_timeout", &self.launch_timeout)
            .field("navigation_timeout", &self.navigation_timeout)
            .finish()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionProbe {
    pub has_sessionid: bool,
    pub texts: Vec<String>,
    pub composer: bool,
    pub url: String,
    pub state: SessionState,
}

/// Mọi thao tác trình duyệt đi qua trait này (ADR-R002) để đổi driver được nếu
/// chromiumoxide bị Dola chặn. P1 chỉ có bề mặt cần cho thí nghiệm đăng nhập;
/// `submit`/`poll_and_download` thêm ở P2.
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    // Review Task 3-6 (minor): moi ham tru launch/close deu giu &Session qua diem await, nen
    // Session can Sync (khong chi Send) de future cua chung Send duoc khi goi tu ngu canh da
    // luong (vd tokio::spawn boc ngoai).
    type Session: Send + Sync;

    /// # Errors
    /// Khong khoi dong duoc trinh duyet (thieu Chrome, profile dang bi khoa, CDP loi).
    async fn launch(&self, opts: &LaunchOpts) -> Result<Self::Session, BrowserError>;

    /// # Errors
    /// Loi CDP khi dat cookie.
    async fn add_cookies(&self, s: &Self::Session, cookies: &[Cookie]) -> Result<(), BrowserError>;

    /// # Errors
    /// Dieu huong that bai (`BrowserError::Navigation`) hoac loi CDP.
    async fn goto(&self, s: &Self::Session, url: &str) -> Result<(), BrowserError>;

    /// # Errors
    /// Loi CDP khi chay JS hoac doc cookie/URL.
    async fn probe_session(&self, s: &Self::Session) -> Result<SessionProbe, BrowserError>;

    /// # Errors
    /// Loi CDP hoac I/O khi ghi file anh chup man hinh.
    async fn screenshot(&self, s: &Self::Session, path: &Path) -> Result<(), BrowserError>;

    /// # Errors
    /// Loi CDP khi dong trinh duyet.
    async fn close(&self, s: Self::Session) -> Result<(), BrowserError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dung_gia_tri_ruling() {
        let d = LaunchOpts::default();
        assert_eq!(d.profile_dir, PathBuf::new());
        assert_eq!(d.browser, BrowserKind::Chromium);
        assert!(d.executable_override.is_none());
        assert!(!d.headless);
        assert!(d.proxy.is_none());
        assert!(!d.window_offscreen);
        assert_eq!(d.launch_timeout, Duration::from_secs(20));
        assert_eq!(d.navigation_timeout, Duration::from_secs(60));
    }

    /// Ruling P2: `Debug` cua kieu chua bi mat phai che mat khau proxy.
    #[test]
    fn debug_khong_lo_mat_khau_proxy() {
        let opts = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            proxy: lvc_core::parse_proxy("1.2.3.4:8080:user1:s3cr3tPW").unwrap(),
            ..LaunchOpts::default()
        };
        let dbg = format!("{opts:?}");
        assert!(!dbg.contains("s3cr3tPW"), "lo mat khau: {dbg}");
        assert!(dbg.contains("***"), "phai co the cho mat khau: {dbg}");
        assert!(dbg.contains("user1"), "{dbg}");
        assert!(dbg.contains("1.2.3.4"), "{dbg}");
        assert!(dbg.contains("Chromium"), "{dbg}");
    }
}
