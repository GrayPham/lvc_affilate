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

    async fn add_cookies(
        &self,
        s: &ChromiumSession,
        cookies: &[Cookie],
    ) -> Result<(), BrowserError> {
        use chromiumoxide::cdp::browser_protocol::network::CookieParam;
        for c in cookies {
            let mut b = CookieParam::builder()
                .name(&c.name)
                .value(&c.value)
                .domain(&c.domain)
                .path(&c.path)
                .secure(c.secure)
                .http_only(c.http_only);
            if let Some(exp) = c.expires {
                #[allow(clippy::cast_precision_loss)] // epoch giay thuc te < 2^53, chinh xac
                let exp = exp as f64;
                b = b.expires(
                    chromiumoxide::cdp::browser_protocol::network::TimeSinceEpoch::new(exp),
                );
            }
            s.page
                .set_cookie(b.build().map_err(BrowserError::Launch)?)
                .await?;
        }
        Ok(())
    }

    async fn goto(&self, s: &ChromiumSession, url: &str) -> Result<(), BrowserError> {
        s.page
            .goto(url)
            .await
            .map_err(|e| BrowserError::Navigation {
                url: url.into(),
                reason: e.to_string(),
            })?;
        s.page
            .wait_for_navigation()
            .await
            .map_err(|e| BrowserError::Navigation {
                url: url.into(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    async fn probe_session(&self, s: &ChromiumSession) -> Result<SessionProbe, BrowserError> {
        #[derive(serde::Deserialize)]
        struct Raw {
            texts: Vec<String>,
            composer: bool,
        }
        let raw: Raw = s
            .page
            .evaluate(lvc_core::SESSION_PROBE_JS)
            .await?
            .into_value()?;
        let cookies = s.page.get_cookies().await?;
        let has_sessionid = cookies
            .iter()
            .any(|c| c.name == lvc_core::cookie::REQUIRED_COOKIE && !c.value.is_empty());
        let url = s.page.url().await?.unwrap_or_default();
        let state = lvc_core::classify_session(has_sessionid, &raw.texts, raw.composer);
        Ok(SessionProbe {
            has_sessionid,
            texts: raw.texts,
            composer: raw.composer,
            url,
            state,
        })
    }

    async fn screenshot(&self, s: &ChromiumSession, path: &Path) -> Result<(), BrowserError> {
        use chromiumoxide::page::ScreenshotParams;
        let bytes = s
            .page
            .screenshot(ScreenshotParams::builder().full_page(false).build())
            .await?;
        tokio::fs::write(path, bytes).await?;
        Ok(())
    }

    async fn close(&self, mut s: ChromiumSession) -> Result<(), BrowserError> {
        let _ = s.browser.close().await;
        s.handler.abort();
        Ok(())
    }
}
