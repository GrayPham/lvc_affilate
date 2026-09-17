//! Impl `BrowserDriver` tren chromiumoxide (CDP thuan Rust).

use std::path::Path;

use async_trait::async_trait;
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::{
    SetLocaleOverrideParams, SetTimezoneOverrideParams, SetUserAgentOverrideParams,
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

/// Bo co MAC DINH cua patchright 1.62.3 (Chromium 1234), chep NGUYEN VAN tu dong lenh that
/// ma patchright khoi chay (bat `DEBUG=pw:browser` khi Python goi
/// `launch_persistent_context(..., ignore_default_args=["--enable-automation"])`), TRU:
/// - `--disable-blink-features=AutomationControlled`, `--disable-infobars`, `--no-first-run`,
///   `--no-default-browser-check`: da co trong [`LAUNCH_ARGS`] (patchright cung truyen trung).
/// - `--user-data-dir`, `--remote-debugging-pipe`, `about:blank`: chromiumoxide tu lo
///   (dung `--remote-debugging-port=0`, khong co pipe).
///
/// Ly do dung list nay thay vi bo Puppeteer-style co san cua chromiumoxide: chromiumoxide
/// 0.9 tu dat `--enable-automation` + `--lang=en_US` (khong xoa le duoc — `ArgsBuilder` la
/// `HashMap` gop theo khoa, module `argument` private) nen ta goi `disable_default_args()`
/// roi tu dat co; va cong cu Python da chay on dinh voi Dola bang dung bo co nay, nen giu
/// parity thay vi tu che. `--no-sandbox` giu vi Playwright tren Windows luon truyen
/// (`chromiumSandbox: false`). `--disable-extensions` chromiumoxide luon them khi khong cau
/// hinh extension (khong tat duoc) — Python khong co co nay; ghi nhan, chua thay anh huong.
/// NHAC: kiem tra lai list nay moi khi nang cap patchright/chromiumoxide (khong co canh bao
/// bien dich nao bao lech).
const PATCHRIGHT_DEFAULT_ARGS: [&str; 22] = [
    "disable-field-trial-config",
    "disable-background-networking",
    "disable-background-timer-throttling",
    "disable-backgrounding-occluded-windows",
    "disable-breakpad",
    "disable-dev-shm-usage",
    "disable-edgeupdater",
    "disable-features=AvoidUnnecessaryBeforeUnloadCheckSync,BoundaryEventDispatchTracksNodeRemoval,DestroyProfileOnBrowserClose,DialMediaRouteProvider,GlobalMediaControls,HttpsUpgrades,LensOverlay,MediaRouter,PaintHolding,ThirdPartyStoragePartitioning,BlockOriginHeaderModificationOnRedirect,Translate,AutoDeElevate,OptimizationHints,msForceBrowserSignIn,msEdgeUpdateLaunchServicesPreferredVersion",
    "enable-features=CDPScreenshotNewSurface",
    "disable-hang-monitor",
    "disable-prompt-on-repost",
    "disable-renderer-backgrounding",
    "disable-updater-scheduler",
    "force-color-profile=srgb",
    "password-store=basic",
    "use-mock-keychain",
    "no-service-autorun",
    "export-tagged-pdf",
    "disable-search-engine-choice-screen",
    "edge-skip-compat-layer-relaunch",
    "disable-sync",
    "no-sandbox",
];

/// Driver chromiumoxide (CDP thuan) cho `BrowserDriver`. Review Task 3-6 (I-5): ghi lai o day
/// nhung khac biet CO SAN so voi patchright/Playwright ma driver nay KHONG the tat qua API
/// cong khai cua chromiumoxide 0.9 — de neu Task 7 that bai (bi Dola chan) thi biet quy ve dau
/// truoc:
/// 1. `Runtime.enable` + `Log.enable` + `Performance.enable` duoc chromiumoxide gui cung luc
///    voi `Target.setAutoAttach` cho MOI target/frame moi (`handler/target.rs::page_init_commands`,
///    `handler/frame.rs`), khong co co nao de tat qua `BrowserConfig`. Day chinh la dieu
///    patchright tranh duoc bang mot co che CDP session khac han — day la GIA THIET CHINH cua
///    thi nghiem P1 (chromiumoxide "lo" nhieu hon patchright ngay tu tang giao thuc).
/// 2. `--disable-extensions` luon duoc chromiumoxide them khi khong cau hinh extension nao
///    (`browser/config.rs::launch`), khong tat duoc qua `BrowserConfig`.
/// 3. Dung `--remote-debugging-port` (mo mot cong TCP) de noi CDP; patchright dung pipe
///    (stdio), khong mo cong mang nao — de bi phat hien hon tu ben ngoai may.
/// 4. `Page::enable_stealth_mode()` co san cua chromiumoxide KHONG duoc goi (xem C-1, review
///    Task 3-6): than ham no tu lam nhieu dieu con te hon — UA gia nam 2022 khong khop build
///    Chrome that dang chay, giao dien JS thay `navigator.webdriver`/`plugins` khong native,
///    va lam vo `WebGLRenderingContext.getParameter`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChromiumDriver;

pub struct ChromiumSession {
    pub browser: Browser,
    pub page: Page,
    handler: JoinHandle<()>,
}

impl ChromiumDriver {
    fn config(opts: &LaunchOpts) -> Result<BrowserConfig, BrowserError> {
        // ADR-R012: Chromium nhan `--user-data-dir` TUONG DOI thi chi nap 7-8/79 cookie cua
        // profile (bat dinh giua cac lan chay) → mat sessionid → "logged_out" gia. Duong dan
        // TUYET DOI nap du 79/79. Playwright luon truyen tuyet doi; ta ep o day de khong
        // phu thuoc vao cwd cua tien trinh goi.
        let profile_dir = std::path::absolute(&opts.profile_dir)?;
        let mut b = BrowserConfig::builder()
            .user_data_dir(&profile_dir)
            .hide() // bo navigator.webdriver that (--disable-blink-features=AutomationControlled)
            // Review Task 3-6 (minor): hide() chi tu null viewport NEU no dang la
            // Some(Viewport::default()) — ep ro bang .viewport(None) o day de khong phu thuoc
            // vao trung hop do. Viewport khac None se khien chromiumoxide tu goi
            // Emulation.setDeviceMetricsOverride VA setTouchEmulationEnabled(true) — dau hieu
            // "desktop co cam ung" rat de bi phat hien, khac may that.
            .viewport(None)
            .window_size(1280, 900)
            // Review Task 3-6 (minor): giu Security.setIgnoreCertificateErrors nhu mac dinh
            // (khong goi), giong Python — tranh che loi proxy/MITM that trong Task 7.
            .respect_https_errors()
            .disable_default_args(); // xem ghi chu PATCHRIGHT_DEFAULT_ARGS o tren
        for a in PATCHRIGHT_DEFAULT_ARGS {
            b = b.arg(a);
        }
        b = b.arg(format!("lang={LOCALE}"));
        for a in LAUNCH_ARGS {
            // Arg::from(&str) coi CA CHUOI (ke ca "--") la khoa, roi ArgsBuilder tu them
            // "--" khi render -> giu nguyen chuoi co san se ra "----foo". Bo "--" dau de
            // chi con dung 1 cap "--" sau khi render.
            b = b.arg(a.trim_start_matches("--"));
        }
        if opts.headless {
            // Review Task 3-6 (minor): Playwright them co nay o che do headless de gia lap
            // thiet bi khong co cam ung (chuot/ban phim), khop may desktop that hon mac dinh
            // cua headless Chrome.
            b = b.arg(
                "blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4",
            );
        } else {
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
        // Review Task 3-6 (I-1): navigator.language(s) VA header Accept-Language deu bat nguon
        // tu danh sach accept-languages cua renderer, chu KHONG phai tu SetLocaleOverrideParams
        // (cai do chi doi ICU cho Intl/Date). --lang=ja-JP chon UI locale "ja" co danh sach
        // accept-languages mac dinh la "ja,en-US,en" -> navigator.language tra ve "ja". Cach
        // Playwright dat duoc "ja-JP" chinh xac la goi Emulation.setUserAgentOverride voi
        // userAgent="" (chuoi rong = KHONG ghi de UA, Chromium bo qua phan do) va
        // acceptLanguage=locale — khong dung script tiem nao ca.
        page.execute(
            SetUserAgentOverrideParams::builder()
                .user_agent("")
                .accept_language(LOCALE)
                .build()
                .map_err(BrowserError::Launch)?,
        )
        .await?;
        page.execute(
            SetTimezoneOverrideParams::builder()
                .timezone_id(TIMEZONE)
                .build()
                .map_err(BrowserError::Launch)?,
        )
        .await?;
        // Review Task 3-6 (C-1): KHONG goi page.enable_stealth_mode() — than ham cua no (xem
        // doc ChromiumDriver o tren) tu tao nhieu dau hieu bi phat hien hon la an toan. Co san
        // --disable-blink-features=AutomationControlled (qua .hide() o config()) da du de
        // navigator.webdriver tra ve false/undefined mot cach NATIVE.
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
            // Review Task 3-6 (I-2): Page::set_cookie CHI tu suy ra url tu trang hien tai neu
            // CookieParam.url dang None, va tu choi khi trang la about:blank/data: — day la
            // luat cua wrapper chromiumoxide, khong phai CDP. Dat url tuong minh o day de
            // add_cookies() dung duoc ngay sau launch(), khong can ghe qua trang khach truoc
            // (Python khong bao gio tai dola.com khi chua co cookie).
            let url = format!("https://{}/", c.domain.trim_start_matches('.'));
            let mut b = CookieParam::builder()
                .name(&c.name)
                .value(&c.value)
                .url(url)
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
        // Review Task 3-6 (I-3): Browser::close chi doi phan hoi CDP Browser.close; drop
        // ChromiumSession ngay sau do co the kich hoat kill_on_drop trong luc Chrome van dang
        // ghi profile (cookie SQLite commit theo lo, exit_type chi ghi khi thoat sach). Doi
        // them (co timeout de khong treo mai neu tien trinh da bi giet) truoc khi huy handler.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), s.browser.wait()).await;
        s.handler.abort();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// ADR-R012: duong dan profile tuong doi PHAI duoc ep tuyet doi truoc khi thanh
    /// `--user-data-dir` (tuong doi → Chromium nap thieu cookie, mat sessionid).
    #[test]
    fn config_ep_user_data_dir_tuyet_doi() {
        let opts = LaunchOpts {
            profile_dir: PathBuf::from("accounts/tuong_doi"),
            headless: true,
            proxy_server: None,
            window_offscreen: false,
            chrome_executable: None,
        };
        let cfg = ChromiumDriver::config(&opts).expect("config");
        let dir = cfg.user_data_dir.expect("user_data_dir phai duoc dat");
        assert!(
            dir.is_absolute(),
            "user_data_dir van tuong doi: {}",
            dir.display()
        );
        assert!(dir.ends_with("accounts/tuong_doi"), "{}", dir.display());
    }
}
