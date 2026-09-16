//! Trait `BrowserDriver` (ADR-R002): moi thao tac trinh duyet di qua day de doi duoc
//! sang driver khac neu chromiumoxide bi Dola chan.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use lvc_core::{Cookie, SessionState};

use crate::BrowserError;

#[derive(Debug, Clone)]
pub struct LaunchOpts {
    /// `accounts/<name>/` — cùng bố cục bản Python để tái dùng profile.
    pub profile_dir: PathBuf,
    pub headless: bool,
    /// `scheme://host:port`, KHÔNG auth (P2).
    pub proxy_server: Option<String>,
    /// Đưa cửa sổ ra ngoài màn hình (-32000,-32000) như `SILENT_MODE` của Python.
    pub window_offscreen: bool,
    /// `None` → chromiumoxide tự tìm Chrome/Chromium trên máy.
    pub chrome_executable: Option<PathBuf>,
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
    type Session: Send;

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
