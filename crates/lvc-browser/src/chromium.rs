//! Impl `BrowserDriver` tren chromiumoxide (CDP thuan Rust).

use std::path::Path;

use async_trait::async_trait;
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::{
    SetLocaleOverrideParams, SetTimezoneOverrideParams,
};
use futures::StreamExt;
use lvc_core::Cookie;
use tokio::task::JoinHandle;

use crate::{BrowserDriver, BrowserError, LaunchOpts, SessionProbe, profile};

/// Khop Python `browser.py` `LAUNCH_ARGS` tung chuoi.
pub const LAUNCH_ARGS: [&str; 5] = [
    "--disable-blink-features=AutomationControlled",
    "--disable-infobars",
    "--silent-debugger-extension-api",
    "--no-first-run",
    "--no-default-browser-check",
];

pub const LOCALE: &str = "ja-JP";
pub const TIMEZONE: &str = "Asia/Tokyo";

/// chromiumoxide 0.9 tu dat mot bo co Puppeteer-style theo mac dinh (xem static rieng
/// `DEFAULT_ARGS` trong `browser/config.rs` cua chromiumoxide — module `argument` KHONG
/// `pub` nen khong doc, ghi de tung co, hay xoa mot co don le duoc; `ArgsBuilder` noi bo
/// la `HashMap` GOP gia tri theo khoa chu khong GHI DE). Bo do co san `--enable-automation`
/// va `--lang=en_US`. De giu dung 2 ngu nghia bat buoc cua ke hoach (bo --enable-automation,
/// khoa ja-JP), ta goi `disable_default_args()` roi lap lai 23 co con lai (tham khao source
/// chromiumoxide-0.9.1/src/browser/config.rs, khong co tren docs.rs vi private) TRU
/// "enable-automation", va doi "lang" thanh `LOCALE`. Xem bao cao Task 4 de biet chi tiet.
const DEFAULT_ARGS_SANS_AUTOMATION: [&str; 22] = [
    "disable-background-networking",
    "enable-features=NetworkService,NetworkServiceInProcess",
    "disable-background-timer-throttling",
    "disable-backgrounding-occluded-windows",
    "disable-breakpad",
    "disable-client-side-phishing-detection",
    "disable-component-extensions-with-background-pages",
    "disable-default-apps",
    "disable-dev-shm-usage",
    "disable-features=TranslateUI",
    "disable-hang-monitor",
    "disable-ipc-flooding-protection",
    "disable-popup-blocking",
    "disable-prompt-on-repost",
    "disable-renderer-backgrounding",
    "disable-sync",
    "force-color-profile=srgb",
    "metrics-recording-only",
    "no-first-run",
    "password-store=basic",
    "use-mock-keychain",
    "enable-blink-features=IdleDetection",
];

#[derive(Debug, Default, Clone, Copy)]
pub struct ChromiumDriver;

pub struct ChromiumSession {
    pub browser: Browser,
    pub page: Page,
    // PF-3: chi doc trong close() (Task 5, s.handler.abort()); Task 4 chua co close that
    // nen truong nay tam thoi "khong doc duoc" theo clippy.
    #[allow(dead_code)]
    handler: JoinHandle<()>,
}

impl ChromiumDriver {
    fn config(opts: &LaunchOpts) -> Result<BrowserConfig, BrowserError> {
        let mut b = BrowserConfig::builder()
            .user_data_dir(&opts.profile_dir)
            .hide() // bo navigator.webdriver (--disable-blink-features=AutomationControlled)
            .window_size(1280, 900)
            .disable_default_args(); // xem ghi chu DEFAULT_ARGS_SANS_AUTOMATION o tren
        for a in DEFAULT_ARGS_SANS_AUTOMATION {
            b = b.arg(a);
        }
        b = b.arg(format!("lang={LOCALE}"));
        for a in LAUNCH_ARGS {
            // Arg::from(&str) coi CA CHUOI (ke ca "--") la khoa, roi ArgsBuilder tu them
            // "--" khi render -> giu nguyen chuoi co san se ra "----foo". Bo "--" dau de
            // chi con dung 1 cap "--" sau khi render.
            b = b.arg(a.trim_start_matches("--"));
        }
        if !opts.headless {
            b = b.with_head();
        }
        if opts.window_offscreen && !opts.headless {
            b = b.arg("window-position=-32000,-32000");
        }
        if let Some(p) = &opts.proxy_server {
            b = b.arg(format!("proxy-server={p}"));
        }
        if let Some(exe) = &opts.chrome_executable {
            b = b.chrome_executable(exe);
        }
        b.build().map_err(BrowserError::Launch)
    }
}

#[async_trait]
impl BrowserDriver for ChromiumDriver {
    type Session = ChromiumSession;

    async fn launch(&self, opts: &LaunchOpts) -> Result<ChromiumSession, BrowserError> {
        tokio::fs::create_dir_all(&opts.profile_dir).await?;
        profile::disable_session_restore(&opts.profile_dir).await?;
        let (browser, mut handler) = Browser::launch(Self::config(opts)?).await.map_err(|e| {
            let s = e.to_string();
            if s.contains("already in use") || s.contains("existing browser session") {
                BrowserError::ProfileBusy(s)
            } else {
                BrowserError::Launch(s)
            }
        })?;
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
        let page = browser.new_page("about:blank").await?;
        page.execute(SetLocaleOverrideParams::builder().locale(LOCALE).build())
            .await?;
        page.execute(
            SetTimezoneOverrideParams::builder()
                .timezone_id(TIMEZONE)
                .build()
                .map_err(BrowserError::Launch)?,
        )
        .await?;
        page.enable_stealth_mode().await?;
        Ok(ChromiumSession {
            browser,
            page,
            handler: handler_task,
        })
    }

    // add_cookies / goto / probe_session / screenshot / close: Task 5
    async fn add_cookies(&self, _s: &ChromiumSession, _c: &[Cookie]) -> Result<(), BrowserError> {
        unimplemented!("Task 5")
    }
    async fn goto(&self, _s: &ChromiumSession, _u: &str) -> Result<(), BrowserError> {
        unimplemented!("Task 5")
    }
    async fn probe_session(&self, _s: &ChromiumSession) -> Result<SessionProbe, BrowserError> {
        unimplemented!("Task 5")
    }
    async fn screenshot(&self, _s: &ChromiumSession, _p: &Path) -> Result<(), BrowserError> {
        unimplemented!("Task 5")
    }
    async fn close(&self, _s: ChromiumSession) -> Result<(), BrowserError> {
        unimplemented!("Task 5")
    }
}
