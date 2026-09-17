//! Impl `BrowserDriver` tren chromiumoxide (CDP thuan Rust).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::{
    SetLocaleOverrideParams, SetTimezoneOverrideParams, SetUserAgentOverrideParams,
};
use futures::StreamExt;
use lvc_core::Cookie;
use tokio::task::JoinHandle;

use crate::{BrowserDriver, BrowserError, LaunchOpts, SessionProbe, executable, profile};

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
/// M-2: ba co SAU day KHONG co trong dong lenh patchright da bat, la lech CO CHU Y — ghi
/// nhan o day thay vi xoa vi da do An toan tren dola.com (ADR-R012):
/// - `--lang=ja-JP`: TA them (qua [`LOCALE`], xem ghi chu I-1 trong `launch()`) de
///   `navigator.language`/Accept-Language ra "ja-JP" dung yeu cau, du patchright khong truyen.
/// - `--window-size=1280,900` va `--remote-debugging-port=0`: CHROMIUMOXIDE tu them (qua
///   `.window_size()` va cong CDP mac dinh khi khong tu dat port — xem `browser/config.rs`
///   trong crate).
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
/// 3. Dung `--remote-debugging-port` (mo mot cong TCP tren 127.0.0.1) de noi CDP; patchright
///    dung pipe (stdio), khong mo cong mang nao — de bi phat hien hon tu tien trinh khac tren
///    cung may (khong phai "tu ben ngoai may": cong chi bind loopback).
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
    /// Chep tu `LaunchOpts::navigation_timeout` luc launch de `goto()` dung.
    navigation_timeout: std::time::Duration,
}

impl ChromiumSession {
    /// Trang chinh cua phien - dau vao cho moi ham trong [`crate::ops`] (Task 6-12).
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }
}

/// Windows: mo file dang duoc tien trinh khac giu doc quyen (vd Chrome dang chay tren cung
/// profile) loi voi ma he dieu hanh `ERROR_SHARING_VIOLATION`.
#[cfg(windows)]
const ERROR_SHARING_VIOLATION: i32 = 32;

/// I-1: Chrome giu `<profile>/lockfile` mo suot vong doi (`process_singleton_win.cc`) va KHONG
/// in gi ra stderr khi tu choi mo lai — thong bao "Opening in existing browser session." la
/// `printf` ra STDOUT (bi chromiumoxide null hoa qua `Stdio::null()`) va da qua `--lang=ja-JP`
/// dich sang tieng Nhat, nen khong the doc duoc tu `CdpError` bang chuoi. Probe truc tiep
/// TRUOC `Browser::launch` bang cach tu mo cung file: neu co tien trinh khac dang giu, mo se
/// loi `ERROR_SHARING_VIOLATION` (32) tren Windows. Chi probe — KHONG xoa file, KHONG cat noi
/// dung (`truncate(false)`); dong ngay (drop) sau khi biet ket qua.
fn probe_lockfile_busy(profile_dir: &Path) -> Result<(), BrowserError> {
    let path = profile_dir.join("lockfile");
    match std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    {
        Ok(_file) => Ok(()), // drop ngay o cuoi match arm — chi probe
        #[cfg(windows)]
        Err(e) if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION) => Err(
            BrowserError::ProfileBusy(format!("lockfile dang bi giu: {}", path.display())),
        ),
        Err(e) => Err(e.into()),
    }
}

/// I-1: `Browser::launch` tra `CdpError` chu khong phai `BrowserError` — quy ve day mot cho.
/// `LaunchExit` la truong hop duy nhat co ma thoat co cau truc; Chrome dung ma
/// `CHROME_RESULT_CODE_PROFILE_IN_USE` (21) hoac, sau khi chuyen dong lenh cho phien dang
/// chay, `CHROME_RESULT_CODE_NORMAL_EXIT_PROCESS_NOTIFIED` (24) khi profile dang bi tien
/// trinh Chrome khac dung (`chrome_result_codes.h`) — hai ma nay la dau hieu DUY NHAT con lai
/// sau khi mat stdout (xem `probe_lockfile_busy`), nhung chi bat duoc SAU khi da spawn tien
/// trinh; `probe_lockfile_busy` bat truoc nen thuong se chan som hon. Cac bien the `CdpError`
/// khac (`LaunchTimeout`, `LaunchIo`, loi ket noi...) khong co ma thoat co cau truc — giu
/// match chuoi tren thong diep `Display` lam luoi an toan cuoi (vd ban Chrome/Chromium khac
/// van in "already in use" ra stderr thay vi stdout).
fn map_launch_error(e: &chromiumoxide::error::CdpError) -> BrowserError {
    use chromiumoxide::error::CdpError;
    if let CdpError::LaunchExit(status, stderr) = e {
        let code = status.code();
        if matches!(code, Some(21 | 24)) {
            return BrowserError::ProfileBusy(format!(
                "Chrome thoat voi ma {code:?} — profile dang bi tien trinh Chrome khac dung"
            ));
        }
        return BrowserError::LaunchExit {
            code,
            stderr: String::from_utf8_lossy(stderr.as_slice()).into_owned(),
        };
    }
    let s = e.to_string();
    if s.contains("already in use") || s.contains("existing browser session") {
        BrowserError::ProfileBusy(s)
    } else {
        BrowserError::Launch(s)
    }
}

/// I-3: danh sach co dua vao builder qua `.arg()`, tach thanh ham THUAN de gate test duoc
/// (khong can Chrome) cac rang buoc "dung 1 lan", "co lang=ja-JP", "khong co
/// enable-automation" ma truoc day chi biet duoc qua doc source chromiumoxide. KHONG co "--"
/// (dung nguyen quy uoc cua `Arg`/`.arg()` — xem vi tri goi). Dung THU TU se dua vao builder.
#[must_use]
pub(crate) fn chrome_args(opts: &LaunchOpts) -> Vec<String> {
    let mut args: Vec<String> = PATCHRIGHT_DEFAULT_ARGS
        .iter()
        .map(|a| (*a).to_string())
        .collect();
    args.push(format!("lang={LOCALE}"));
    for a in LAUNCH_ARGS {
        // Arg::from(&str) coi CA CHUOI (ke ca "--") la khoa, roi ArgsBuilder tu them "--" khi
        // render -> giu nguyen chuoi co san se ra "----foo". Bo "--" dau de chi con dung 1
        // cap "--" sau khi render.
        args.push(a.trim_start_matches("--").to_string());
    }
    if opts.headless {
        // Review Task 3-6 (minor): Playwright them co nay o che do headless de gia lap thiet
        // bi khong co cam ung (chuot/ban phim), khop may desktop that hon mac dinh cua
        // headless Chrome.
        args.push(
            "blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4"
                .to_string(),
        );
    }
    if opts.window_offscreen && !opts.headless {
        args.push("window-position=-32000,-32000".to_string());
    }
    if let Some(p) = &opts.proxy {
        // Chi `scheme://host:port`; user/pass (neu co) di qua Fetch.* o Task 13.
        args.push(format!("proxy-server={}", p.server()));
        // M-11 (Playwright parity): Playwright luon them bypass `<-loopback>` khi co proxy de
        // 127.0.0.1/localhost KHONG di qua proxy (cong CDP, server test cuc bo...).
        args.push("proxy-bypass-list=<-loopback>".to_string());
    }
    args
}

/// Ket qua buoc chuan bi THUAN cua `launch()` (khong ghi dia): config da build + duong dan
/// profile TUYET DOI (dung cho `create_dir_all`/Preferences/lockfile, cung mot gia tri voi
/// `--user-data-dir`).
struct Prepared {
    config: BrowserConfig,
    profile_dir: PathBuf,
}

impl ChromiumDriver {
    /// Review Task 5 I-2: MOI kiem tra co the that bai ma khong can Chrome (profile rong, ep
    /// tuyet doi, resolve binary, build config) nam o day va `launch()` goi no TRUOC bat ky I/O
    /// nao — truoc day `create_dir_all` + ghi `Default/Preferences` + tao `lockfile` chay truoc,
    /// nen may thieu binary van de lai rac trong `accounts/<ten>/`, va `LaunchOpts::default()`
    /// (profile rong) ghi `./Default/Preferences` + `./lockfile` vao CWD roi moi loi.
    fn prepare(opts: &LaunchOpts) -> Result<Prepared, BrowserError> {
        if opts.profile_dir.as_os_str().is_empty() {
            return Err(BrowserError::Launch(
                "profile_dir rong — moi tai khoan phai co thu muc accounts/<ten>/".into(),
            ));
        }
        // ADR-R012: Chromium nhan `--user-data-dir` TUONG DOI thi chi nap 7-8/79 cookie cua
        // profile (bat dinh giua cac lan chay) → mat sessionid → "logged_out" gia. Duong dan
        // TUYET DOI nap du 79/79. Playwright luon truyen tuyet doi; ta ep o day de khong
        // phu thuoc vao cwd cua tien trinh goi.
        let profile_dir = std::path::absolute(&opts.profile_dir)?;
        // Review P1 I-6 / ADR-R012: resolve TUONG MINH theo BrowserKind; loi ro neu khong thay.
        // KHONG bao gio de BrowserConfigBuilder::build() roi vao detection::default_executable()
        // (no se vo tinh mo Google Chrome tren profile Chromium 1234 cua patchright).
        let exe =
            executable::resolve_executable(opts.browser, opts.executable_override.as_deref())?;
        let mut b = BrowserConfig::builder()
            .user_data_dir(&profile_dir)
            .chrome_executable(exe)
            .launch_timeout(opts.launch_timeout)
            // M-1: bo .hide() — no chi lam hai viec: (1) dat --disable-blink-features=
            // AutomationControlled, da co san trong LAUNCH_ARGS (qua chrome_args()) nen goi
            // .hide() tao ra co nay HAI LAN (ArgsBuilder gop theo khoa nen khong sao tren dong
            // lenh that, nhung chrome_args() se khong con phan anh dung "list se dua vao
            // builder" cho unit test I-3); (2) null hoa viewport NEU no dang la
            // Some(Viewport::default()) — ta da ep ro bang .viewport(None) ngay duoi day nen
            // khong phu thuoc vao hieu ung phu do cua .hide().
            .viewport(None)
            .window_size(1280, 900)
            // Review Task 3-6 (minor): giu Security.setIgnoreCertificateErrors nhu mac dinh
            // (khong goi), giong Python — tranh che loi proxy/MITM that trong Task 7.
            .respect_https_errors()
            .disable_default_args(); // xem ghi chu PATCHRIGHT_DEFAULT_ARGS o tren
        for a in chrome_args(opts) {
            b = b.arg(a);
        }
        if !opts.headless {
            b = b.with_head();
        }
        let config = b.build().map_err(BrowserError::Launch)?;
        Ok(Prepared {
            config,
            profile_dir,
        })
    }
}

#[async_trait]
impl BrowserDriver for ChromiumDriver {
    type Session = ChromiumSession;

    async fn launch(&self, opts: &LaunchOpts) -> Result<ChromiumSession, BrowserError> {
        // I-2: chuan bi thuan TRUOC — loi profile/binary thi tra ve ngay, chua dung dia.
        let Prepared {
            config,
            profile_dir,
        } = Self::prepare(opts)?;
        tokio::fs::create_dir_all(&profile_dir).await?;
        profile::disable_session_restore(&profile_dir).await?;
        probe_lockfile_busy(&profile_dir)?;
        let (mut browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| map_launch_error(&e))?;
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
        // M-12: neu buoc nao duoi day loi, TRUOC day ham tra Err ngay va lam `browser` bi drop
        // — Drop cua Browser chi canh bao (khong doi CDP), va Child ben duoi kill_on_drop
        // (TerminateProcess) giet Chrome GIUA chung khi no co the dang ghi profile (giong hai
        // vi tri da fix o `close()`, xem review Task 3-6 I-3). Gom lai trong mot khoi de khi
        // loi thi dong nhe nhang (`Browser::close`) truoc khi tra Err, thay vi de drop giet.
        let init_result: Result<Page, BrowserError> = async {
            let page = browser.new_page("about:blank").await?;
            page.execute(SetLocaleOverrideParams::builder().locale(LOCALE).build())
                .await?;
            // Review Task 3-6 (I-1): navigator.language(s) VA header Accept-Language deu bat
            // nguon tu danh sach accept-languages cua renderer, chu KHONG phai tu
            // SetLocaleOverrideParams (cai do chi doi ICU cho Intl/Date). --lang=ja-JP chon UI
            // locale "ja" co danh sach accept-languages mac dinh la "ja,en-US,en" ->
            // navigator.language tra ve "ja". Cach Playwright dat duoc "ja-JP" chinh xac la
            // goi Emulation.setUserAgentOverride voi userAgent="" (chuoi rong = KHONG ghi de
            // UA, Chromium bo qua phan do) va acceptLanguage=locale — khong dung script tiem
            // nao ca.
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
            // Review Task 3-6 (C-1): KHONG goi page.enable_stealth_mode() — than ham cua no
            // (xem doc ChromiumDriver o tren) tu tao nhieu dau hieu bi phat hien hon la an
            // toan. Co san --disable-blink-features=AutomationControlled (qua LAUNCH_ARGS) da
            // du de navigator.webdriver tra ve false/undefined mot cach NATIVE.
            Ok(page)
        }
        .await;
        let page = match init_result {
            Ok(page) => page,
            Err(e) => {
                let _ = browser.close().await;
                handler_task.abort();
                return Err(e);
            }
        };
        Ok(ChromiumSession {
            browser,
            page,
            handler: handler_task,
            navigation_timeout: opts.navigation_timeout,
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
        // M-6: wait_for_navigation() khong co deadline rieng — no "park" toi lan `Page` load
        // KE TIEP (handler/target.rs), nen mot SPA khong bao gio phat su kien load lai (hoac
        // mot redirect vo han) se treo vinh vien. Boc timeout `navigation_timeout` (mac dinh 60s,
        // khop Python: /chat cho toi 60s) ngoai ca goto+wait.
        let limit = s.navigation_timeout;
        tokio::time::timeout(limit, async {
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
            Ok::<(), BrowserError>(())
        })
        .await
        .map_err(|_elapsed| BrowserError::Timeout(format!("dieu huong toi {url} qua {limit:?}")))?
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
        // I-2: gioi han urls=[DOLA_URL] thay vi Page::get_cookies() (Network.getCookies KHONG
        // urls -> cookie cua TRANG/TAB hien tai, co the la trang OAuth/loi sau redirect) —
        // khop Python context.cookies("https://www.dola.com"). Luat "co sessionid hop le" la
        // ham thuan lvc_core::has_dola_sessionid, test duoc khong can Chrome.
        let cookies = s
            .page
            .execute(
                chromiumoxide::cdp::browser_protocol::network::GetCookiesParams::builder()
                    .urls(vec![lvc_core::DOLA_URL.to_string()])
                    .build(),
            )
            .await?
            .result
            .cookies;
        let has_sessionid = lvc_core::has_dola_sessionid(
            cookies
                .iter()
                .map(|c| (c.domain.as_str(), c.name.as_str(), c.value.as_str())),
        );
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

    /// Opts cho test unit: KHONG can Chrome that. `executable_override` tro toi mot file tam
    /// (resolver chi stat, `BrowserConfigBuilder::build()` khong stat lai) de `config()` khong
    /// roi vao do binary that (I-4: gate khong phu thuoc Chrome).
    fn opts_voi_exe_gia(dir: &tempfile::TempDir, headless: bool) -> LaunchOpts {
        let exe = dir.path().join("chrome_gia.exe");
        std::fs::write(&exe, b"").unwrap();
        LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            headless,
            executable_override: Some(exe),
            ..LaunchOpts::default()
        }
    }

    /// I-1: profile dang bi mot tien trinh Chrome khac giu (Windows: `ERROR_SHARING_VIOLATION`
    /// khi mo `lockfile`) phai bao `ProfileBusy` TRUOC khi goi `Browser::launch` — khong can
    /// Chrome that de tai hien, chi can giu handle len cung file.
    #[cfg(windows)]
    #[test]
    fn probe_lockfile_busy_khi_tien_trinh_khac_dang_giu() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempfile::tempdir().unwrap();
        let lock_path = dir.path().join("lockfile");
        let held = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(&lock_path)
            .expect("mo lockfile de giu doc quyen (share_mode 0)");

        let err = probe_lockfile_busy(dir.path()).expect_err("phai bao ProfileBusy");
        assert!(
            matches!(err, BrowserError::ProfileBusy(_)),
            "loi sai kieu: {err:?}"
        );

        drop(held);
        probe_lockfile_busy(dir.path()).expect("phai Ok sau khi tha handle");
    }

    /// ADR-R012: duong dan profile tuong doi PHAI duoc ep tuyet doi truoc khi thanh
    /// `--user-data-dir` (tuong doi → Chromium nap thieu cookie, mat sessionid).
    #[test]
    fn config_ep_user_data_dir_tuyet_doi() {
        let tmp = tempfile::tempdir().unwrap();
        let opts = LaunchOpts {
            profile_dir: PathBuf::from("accounts/tuong_doi"),
            ..opts_voi_exe_gia(&tmp, true)
        };
        let prepared = ChromiumDriver::prepare(&opts).expect("prepare");
        let dir = prepared
            .config
            .user_data_dir
            .expect("user_data_dir phai duoc dat");
        assert_eq!(
            prepared.profile_dir, dir,
            "duong dan dung cho I/O (Preferences/lockfile) phai la chinh --user-data-dir"
        );
        assert!(
            dir.is_absolute(),
            "user_data_dir van tuong doi: {}",
            dir.display()
        );
        assert!(dir.ends_with("accounts/tuong_doi"), "{}", dir.display());
        // I-4: cung ghim "khong co tien to \\?\" — std::path::absolute (GetFullPathNameW)
        // khong them tien to nay, nhung canonicalize() (neu ai do doi sang) thi co, va van
        // "tuyet doi" + "ends_with" van khop nen khong bi bat neu chi test 2 dieu tren.
        assert!(
            !dir.to_string_lossy().starts_with(r"\\?\"),
            "user_data_dir co tien to verbatim \\\\?\\: {}",
            dir.display()
        );
    }

    /// Review P1 I-6: `config()` PHAI di qua resolver — override khong ton tai thi loi
    /// `ExecutableNotFound` ngay, KHONG roi ve `detection::default_executable()` cua chromiumoxide.
    #[test]
    fn config_override_khong_ton_tai_thi_executable_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("khong_co.exe");
        let opts = LaunchOpts {
            profile_dir: tmp.path().join("p"),
            executable_override: Some(missing.clone()),
            ..LaunchOpts::default()
        };
        let err = ChromiumDriver::prepare(&opts)
            .map(|_| ())
            .expect_err("phai loi");
        match err {
            BrowserError::ExecutableNotFound { kind, tried } => {
                assert_eq!(kind, lvc_core::BrowserKind::Chromium);
                assert_eq!(tried, vec![missing]);
            }
            other => panic!("sai bien the: {other:?}"),
        }
    }

    /// Review Task 5 I-2: `launch()` PHAI that bai o buoc chuan bi thuan (profile rong / binary
    /// khong resolve duoc) TRUOC khi dung dia — truoc day `create_dir_all` + ghi
    /// `Default/Preferences` + tao `lockfile` chay truoc `config()`, nen `LaunchOpts::default()`
    /// (profile rong) ghi `./Default/Preferences` + `./lockfile` vao CWD roi moi loi. Khong can
    /// Chrome: ca hai truong hop deu loi truoc `Browser::launch`.
    #[tokio::test]
    async fn launch_khong_dung_dia_khi_chuan_bi_loi() {
        let d = ChromiumDriver;

        // (a) profile rong -> loi ro, KHONG ghi gi vao cwd (cwd cua test = crates/lvc-browser)
        // ChromiumSession khong co Debug -> khong dung expect_err
        let Err(err) = d.launch(&LaunchOpts::default()).await else {
            panic!("profile rong phai loi");
        };
        assert!(
            matches!(&err, BrowserError::Launch(m) if m.contains("profile_dir")),
            "loi sai kieu/thong diep: {err:?}"
        );
        assert!(
            !Path::new("Default").join("Preferences").exists(),
            "da ghi ./Default/Preferences vao cwd truoc khi loi"
        );
        assert!(
            !Path::new("lockfile").exists(),
            "da tao ./lockfile trong cwd truoc khi loi"
        );

        // (b) binary khong resolve duoc -> ExecutableNotFound, thu muc profile KHONG duoc tao
        let tmp = tempfile::tempdir().unwrap();
        let profile = tmp.path().join("chua_tao");
        let Err(err) = d
            .launch(&LaunchOpts {
                profile_dir: profile.clone(),
                executable_override: Some(tmp.path().join("khong_co.exe")),
                ..LaunchOpts::default()
            })
            .await
        else {
            panic!("override khong ton tai phai loi");
        };
        assert!(
            matches!(err, BrowserError::ExecutableNotFound { .. }),
            "loi sai kieu: {err:?}"
        );
        assert!(
            !profile.exists(),
            "khong duoc tao thu muc profile truoc khi resolve binary xong"
        );
    }

    /// I-3: ghim cac rang buoc dong lenh launch bang mot ham THUAN (khong can Chrome) — gate
    /// truoc day khong test duoc 5 co trong `LAUNCH_ARGS`, --lang=ja-JP, hay "khong co
    /// --enable-automation" (chi biet duoc qua doc chromiumoxide, xem final-review.md I-3).
    #[test]
    fn chrome_args_chua_du_5_launch_args_dung_1_lan() {
        let opts = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            headless: true,
            ..LaunchOpts::default()
        };
        let args = chrome_args(&opts);
        for flag in LAUNCH_ARGS {
            let want = flag.trim_start_matches("--");
            assert_eq!(
                args.iter().filter(|a| a.as_str() == want).count(),
                1,
                "co {want:?} phai xuat hien dung 1 lan trong {args:?}"
            );
        }
    }

    #[test]
    fn chrome_args_chua_lang_ja_jp() {
        let opts = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            headless: true,
            ..LaunchOpts::default()
        };
        assert!(chrome_args(&opts).iter().any(|a| a == "lang=ja-JP"));
    }

    #[test]
    fn chrome_args_khong_enable_automation_va_khong_co_bo_qua_chu_dinh() {
        let opts = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            headless: false,
            ..LaunchOpts::default()
        };
        let args = chrome_args(&opts);
        assert!(
            !args.iter().any(|a| a.contains("enable-automation")),
            "khong duoc co enable-automation: {args:?}"
        );
        assert!(!args.iter().any(|a| a.contains("disable-popup-blocking")));
        assert!(!args.iter().any(|a| a.contains("disable-default-apps")));
    }

    #[test]
    fn chrome_args_blink_settings_chi_khi_headless() {
        let base = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            headless: true,
            ..LaunchOpts::default()
        };
        assert!(
            chrome_args(&base)
                .iter()
                .any(|a| a.starts_with("blink-settings="))
        );

        let headed = LaunchOpts {
            headless: false,
            ..base
        };
        assert!(
            !chrome_args(&headed)
                .iter()
                .any(|a| a.starts_with("blink-settings="))
        );
    }

    /// M-11: co proxy → `proxy-server=<server()>` (KHONG kem user:pass) va ngay sau la
    /// `proxy-bypass-list=<-loopback>` (Playwright parity); khong proxy → khong co ca hai.
    #[test]
    fn chrome_args_proxy_server_va_bypass_loopback() {
        let no_proxy = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            ..LaunchOpts::default()
        };
        let args = chrome_args(&no_proxy);
        assert!(!args.iter().any(|a| a.starts_with("proxy-")), "{args:?}");

        let with_proxy = LaunchOpts {
            proxy: lvc_core::parse_proxy("socks5://tenND:matkhauBM@10.0.0.9:1080").unwrap(),
            ..no_proxy
        };
        let args = chrome_args(&with_proxy);
        let i = args
            .iter()
            .position(|a| a == "proxy-server=socks5://10.0.0.9:1080")
            .unwrap_or_else(|| panic!("thieu proxy-server: {args:?}"));
        assert_eq!(
            args.get(i + 1).map(String::as_str),
            Some("proxy-bypass-list=<-loopback>"),
            "{args:?}"
        );
        assert!(
            !args
                .iter()
                .any(|a| a.contains("tenND") || a.contains("matkhauBM")),
            "khong duoc lo user/pass tren dong lenh: {args:?}"
        );
    }
}
