//! Impl `BrowserDriver` tren chromiumoxide (CDP thuan Rust).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::emulation::{
    SetLocaleOverrideParams, SetTimezoneOverrideParams, SetUserAgentOverrideParams,
};
use chromiumoxide::cdp::browser_protocol::fetch::{
    AuthChallengeResponse, AuthChallengeResponseResponse, AuthChallengeSource,
    ContinueRequestParams, ContinueWithAuthParams, EnableParams as FetchEnableParams,
    EventAuthRequired, EventRequestPaused, RequestPattern,
};
use chromiumoxide::listeners::EventStream;
use futures::StreamExt;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use lvc_core::{Cookie, Limits, ProxySpec, SessionState};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::task::JoinHandle;

use crate::dola::{
    self, ConversationId, DolaError, DolaHttp, PollHooks, PollOpts, SubmitRequest, VideoArtifact,
};
use crate::{BrowserDriver, BrowserError, LaunchOpts, SessionProbe, executable, ops, profile};

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

/// Task 13: `close()` doi Chrome thoat toi da chung nay sau `Browser.close` (Python/Playwright
/// `context.close()` cung doi tien trinh; Chrome ghi cookie `SQLite`/`exit_type` luc thoat sach).
pub const CLOSE_WAIT: Duration = Duration::from_secs(30);
/// Task 13: `probe_session` — Python `browser.py:352` `asyncio.wait_for(page.evaluate(...), 15)`.
pub const PROBE_EVAL_TIMEOUT: Duration = Duration::from_secs(15);
/// Task 13: `launch()` doi handler bao ve trang dau (`Target.targetCreated` + attach) toi da chung nay
/// truoc khi roi ve `new_page` (binh thuong < 100 ms).
const FIRST_PAGE_WAIT: Duration = Duration::from_secs(3);
/// Python `browser.py` `CLOSE_TAB_TIMEOUT = 5`: dong mot tab thua toi da 5 s; qua → bo, khong dong tiep.
const CLOSE_TAB_TIMEOUT: Duration = Duration::from_secs(5);
/// Python `browser.py` `STARTUP_JUNK_HOSTS`: tab Opera GX mo san — KHONG dung lam tab chinh (goto
/// bi `ERR_ABORTED`) va KHONG dong (Opera GX treo).
const STARTUP_JUNK_HOSTS: [&str; 1] = ["gxcorner.games"];
/// Gioi han bo nho cua tap "request id da thu xac thuc proxy" (xem `proxy_auth_loop`).
const AUTH_ATTEMPTED_CAP: usize = 1024;
/// Review T13 M-2: URL gia cho tab ma `Page::url()` khong tra loi trong [`FIRST_PAGE_WAIT`] (target
/// init loi/treo) — KHONG chon lam tab chinh, KHONG dong (nhu tab rac), khong lam `launch()` loi.
const UNREADABLE_URL: &str = "<khong-doc-duoc>";

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
/// 5. (Task 13) Proxy co user/pass → `Fetch.enable{patterns:[*], handleAuthRequests}` tren PHIEN
///    BROWSER: MOI request cua MOI target bi tam dung roi `continueRequest` qua CDP (Playwright
///    lam y het cho `proxy.username`, nhung tren phien trang + `Network.setCacheDisabled(true)`;
///    ta KHONG tat cache). Chi bat khi co user/pass — proxy khong auth/khong proxy: khong Fetch.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChromiumDriver;

/// Phien Chromium mot tai khoan. Task 13: cac phan can dong deu nam trong `Option`/`Arc` de
/// `close()` lay ra (`take`) va `Drop` biet "da dong hay chua":
/// - `browser`: `Arc` vi task xac thuc proxy ([`proxy_auth_loop`]) can `&Browser` dong thoi (gui
///   `Fetch.continue*` tren PHIEN BROWSER); `close()` dung task do roi `Arc::try_unwrap` de goi
///   `Browser::close`/`wait` (`&mut self`).
/// - `handler`: task bom su kien CDP; `is_connected()` = chua ket thuc.
/// - `proxy_auth`: task Fetch.* (chi khi proxy co user/pass).
/// - `job`: Job Object Windows (`KILL_ON_JOB_CLOSE`), tha SAU khi Chrome da thoat.
pub struct ChromiumSession {
    browser: Option<Arc<Browser>>,
    pub page: Page,
    handler: Option<JoinHandle<()>>,
    proxy_auth: Option<JoinHandle<()>>,
    /// Dinh so lenh `Fetch.continue*` dang bay cung luc (review T13 I-1: bang chung tra loi song
    /// song); `None` khi khong bat proxy auth.
    proxy_auth_peak: Option<Arc<AtomicUsize>>,
    #[cfg(windows)]
    job: Option<crate::job::JobHandle>,
    /// PID tien trinh Chrome (browser process) — chan doan / smoke test dong lenh.
    pid: Option<u32>,
    /// Chep tu `LaunchOpts::navigation_timeout` luc launch de `goto()` dung.
    navigation_timeout: Duration,
    /// Task 14: chep tu `LaunchOpts::proxy` luc launch — `poll_and_download()` dung lai de dung
    /// `DolaHttp` cung proxy voi Chrome (khong doc lai `LaunchOpts` cua caller moi lan poll).
    proxy: Option<ProxySpec>,
    /// Task 14: chep TUYET DOI tu `LaunchOpts::logs_dir` luc launch (giong `profile_dir`,
    /// ADR-R012) — `submit()` dung cho `SubmitCtx::logs_dir`.
    logs_dir: PathBuf,
}

impl ChromiumSession {
    /// Trang chinh cua phien - dau vao cho moi ham trong [`crate::ops`] (Task 6-12).
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// Task 13: handler CDP con chay (websocket con mo, Chrome con song). `false` sau `close()`
    /// hoac khi Chrome chet/bi kill — engine (T15 `SessionCache::spawn_reaper`) bo phien nay.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.handler.as_ref().is_some_and(|h| !h.is_finished())
    }

    /// PID tien trinh Chrome (browser process); `None` neu chromiumoxide khong tra `Child`.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// `Browser` chromiumoxide (lenh muc browser: `pages()`, `version()`…); `None` sau `close()`.
    /// Thay cho truong `pub browser` cu (Task 13 boc `Option<Arc<_>>`).
    #[must_use]
    pub fn browser(&self) -> Option<&Browser> {
        self.browser.as_deref()
    }

    /// Chan doan (review T13 I-1): so lenh `Fetch.continueRequest`/`continueWithAuth` NHIEU NHAT
    /// tung bay cung luc trong [`proxy_auth_loop`]; `None` khi proxy khong co user/pass (khong bat
    /// Fetch). ≥ 2 = tra loi duoc pipeline, khong tuan tu.
    #[must_use]
    pub fn proxy_auth_peak_inflight(&self) -> Option<usize> {
        self.proxy_auth_peak
            .as_ref()
            .map(|p| p.load(Ordering::Relaxed))
    }

    /// Lay het phan can dong ra khoi `self` (mot lan): `None` neu da `close()`.
    fn take_parts(&mut self) -> Option<SessionParts> {
        let browser = self.browser.take()?;
        Some(SessionParts {
            browser,
            handler: self.handler.take(),
            proxy_auth: self.proxy_auth.take(),
            #[cfg(windows)]
            job: self.job.take(),
        })
    }
}

/// Task 13: `Drop` khong bo roi Chrome. Chua `close()` ma con runtime tokio → spawn dong NHE NHANG
/// ([`shutdown`]: `Browser.close` + doi thoat ≤ [`CLOSE_WAIT`]). Khong co runtime (drop ngoai
/// async, hoac runtime dang tat nen task spawn bi huy ngay) → roi ve `kill_on_drop` cua
/// chromiumoxide (`TerminateProcess` khi `Browser`/`Child` drop) + Job Object dong → Chrome chet
/// ngay, KHONG thoat sach (co the mat cookie ghi do; profile tu phuc hoi lan sau — `profile.rs`).
impl Drop for ChromiumSession {
    fn drop(&mut self) {
        let Some(parts) = self.take_parts() else {
            return; // da close()
        };
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            tracing::warn!(
                pid = ?self.pid,
                "ChromiumSession bi drop khi chua close() — dong Chrome nhe nhang trong nen"
            );
            rt.spawn(shutdown(parts, CLOSE_WAIT));
        } else {
            tracing::warn!(
                pid = ?self.pid,
                "ChromiumSession bi drop ngoai runtime tokio — Chrome bi kill (kill_on_drop + Job Object)"
            );
            drop(parts);
        }
    }
}

/// Nhung gi [`shutdown`] can — tach khoi `ChromiumSession` de `close()`, `Drop` va duong loi cua
/// `launch()` dung chung mot trinh tu.
struct SessionParts {
    browser: Arc<Browser>,
    handler: Option<JoinHandle<()>>,
    proxy_auth: Option<JoinHandle<()>>,
    #[cfg(windows)]
    job: Option<crate::job::JobHandle>,
}

/// Trinh tu dong duy nhat (Task 13): (1) dung task proxy auth (tha `Arc<Browser>` cua no);
/// (2) `Browser.close` (CDP) roi doi tien trinh thoat toi da `wait` — qua han → drop `Browser`
/// (chromiumoxide `kill_on_drop` = `TerminateProcess`); (3) huy handler; (4) tha Job Object cuoi
/// cung (Chrome da thoat → Job rong; con sot → Job giet ca cay). Khong bao gio tra loi: dong la
/// best-effort, moi loi chi log.
async fn shutdown(parts: SessionParts, wait: Duration) {
    let SessionParts {
        browser,
        handler,
        proxy_auth,
        #[cfg(windows)]
        job,
    } = parts;
    if let Some(t) = proxy_auth {
        t.abort();
        let _ = t.await; // dam bao future (va Arc<Browser> trong no) da bi drop
    }
    match Arc::try_unwrap(browser) {
        Ok(mut b) => {
            if let Err(e) = b.close().await {
                tracing::debug!(error = %e, "Browser.close loi (Chrome co the da thoat)");
            }
            // Review Task 3-6 (I-3): Browser::close chi doi phan hoi CDP; Chrome van dang ghi profile
            // (cookie SQLite, exit_type) — doi tien trinh thoat that su.
            match tokio::time::timeout(wait, b.wait()).await {
                Ok(Ok(status)) => tracing::debug!(?status, "Chrome da thoat"),
                Ok(Err(e)) => tracing::warn!(error = %e, "doi Chrome thoat loi"),
                Err(_elapsed) => tracing::warn!(
                    ?wait,
                    "Chrome khong thoat kip — kill (kill_on_drop + Job Object)"
                ),
            }
            drop(b);
        }
        Err(shared) => {
            // KHONG THE XAY RA theo cau truc (review T13 M-6): `browser()` chi cho `&Browser`, task
            // auth muon `&Browser` va da bi abort+await o tren, `launch()` drop clone cua no truoc
            // khi goi shutdown. Nhanh nay chi la luoi an toan: gui Browser.close qua `execute` (khong
            // dat `closing` cua handler, khong doi tien trinh) — dung "sua" thanh cho doi.
            debug_assert!(false, "Arc<Browser> con tham chieu khac luc shutdown");
            tracing::warn!(
                "Arc<Browser> con tham chieu khac — chi gui Browser.close, khong doi duoc"
            );
            let _ = shared
                .execute(chromiumoxide::cdp::browser_protocol::browser::CloseParams::default())
                .await;
        }
    }
    if let Some(h) = handler {
        h.abort();
    }
    #[cfg(windows)]
    drop(job);
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
        // Chi `scheme://host:port`; user/pass (neu co) di qua Fetch.* sau khi mo (`enable_proxy_auth`).
        args.push(format!("proxy-server={}", p.server()));
        // M-11 (Playwright parity): Playwright luon them bypass `<-loopback>` khi co proxy de
        // 127.0.0.1/localhost KHONG di qua proxy (cong CDP, server test cuc bo...).
        args.push("proxy-bypass-list=<-loopback>".to_string());
    }
    args
}

/// Task 13: user/pass proxy de day qua `Fetch.*` — `Some` CHI khi `ProxySpec::has_auth()`
/// (co username; password thieu → chuoi rong). Thuan de test "khong auth → khong bat Fetch".
#[must_use]
pub(crate) fn proxy_credentials(opts: &LaunchOpts) -> Option<(String, String)> {
    let p = opts.proxy.as_ref()?;
    let username = p.username.clone()?;
    Some((username, p.password.clone().unwrap_or_default()))
}

/// Python `browser.py` `is_startup_junk`: URL thuoc host rac luc khoi dong (Opera GX). Tab
/// [`UNREADABLE_URL`] (M-2) doi xu nhu tab rac: khong chon, khong dong.
fn is_startup_junk(url: &str) -> bool {
    if url == UNREADABLE_URL {
        return true;
    }
    let u = url.to_ascii_lowercase();
    STARTUP_JUNK_HOSTS.iter().any(|h| u.contains(h))
}

/// Python `browser.py` `fresh_page`/`close_extra_pages` (M-3): chon chi so trang CHINH trong danh
/// sach URL cua cac tab dang mo — uu tien `about:blank` (URL rong coi nhu `about:blank`), khong thi
/// tab dau tien khong rac; toan rac → `None` (caller mo tab moi). Thuan de test khong can Chrome.
#[must_use]
pub(crate) fn choose_main_page(urls: &[Option<String>]) -> Option<usize> {
    let is_blank = |u: &Option<String>| {
        u.as_deref()
            .is_none_or(|u| u.is_empty() || u == "about:blank")
    };
    let not_junk = |u: &Option<String>| !u.as_deref().is_some_and(is_startup_junk);
    urls.iter()
        .position(|u| not_junk(u) && is_blank(u))
        .or_else(|| urls.iter().position(not_junk))
}

/// M-3 (Python `fresh_page` + `close_extra_pages`): dung TRANG DAU cua Chrome (tab `about:blank`/NTP
/// mo san) thay vi `Target.createTarget` — patchright cung dung `context.pages[0]`; mot tab moi
/// bang CDP la dau hieu tu dong hoa khong can thiet. Doi handler bao ve tab (toi da
/// [`FIRST_PAGE_WAIT`]); khong co tab nao → `new_page` (Chrome mo khong tab, hiem). Tab thua (khoi
/// phuc phien, tab rac) dong voi timeout [`CLOSE_TAB_TIMEOUT`] moi tab; tab rac (Opera GX) giu
/// nguyen nhu Python. Sau do `Page.navigate about:blank` de tab chinh o trang thai sach (chromiumoxide
/// khong truyen `about:blank` vi tri cuoi dong lenh nhu Playwright → headed mo NTP).
async fn first_page(browser: &Browser) -> Result<Page, BrowserError> {
    let deadline = tokio::time::Instant::now() + FIRST_PAGE_WAIT;
    let pages = loop {
        let pages = browser.pages().await?;
        if !pages.is_empty() || tokio::time::Instant::now() >= deadline {
            break pages;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let mut urls = Vec::with_capacity(pages.len());
    for p in &pages {
        // Review T13 M-2: `Page::url()` chi duoc tra loi sau khi target init xong; tab init loi/treo
        // → khong lam launch() loi, danh dau UNREADABLE_URL (khong chon, khong dong).
        match tokio::time::timeout(FIRST_PAGE_WAIT, p.url()).await {
            Ok(Ok(u)) => urls.push(u),
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "khong doc duoc URL tab — bo qua tab nay");
                urls.push(Some(UNREADABLE_URL.to_string()));
            }
            Err(_elapsed) => {
                tracing::warn!(?FIRST_PAGE_WAIT, "tab khong tra loi URL — bo qua tab nay");
                urls.push(Some(UNREADABLE_URL.to_string()));
            }
        }
    }
    let Some(keep) = choose_main_page(&urls) else {
        tracing::debug!(tabs = pages.len(), "khong co tab dung duoc — mo tab moi");
        return Ok(browser.new_page("about:blank").await?);
    };
    let mut closed = 0_usize;
    for (i, p) in pages.iter().enumerate() {
        if i == keep || urls[i].as_deref().is_some_and(is_startup_junk) {
            continue; // Python: khong dong tab rac (Opera GX treo)
        }
        match tokio::time::timeout(CLOSE_TAB_TIMEOUT, p.clone().close()).await {
            Ok(Ok(())) => closed += 1,
            Ok(Err(e)) => tracing::debug!(error = %e, "dong tab thua loi — bo qua"),
            Err(_elapsed) => {
                tracing::warn!(
                    ?CLOSE_TAB_TIMEOUT,
                    "dong tab thua treo — de nguyen cac tab con lai"
                );
                break;
            }
        }
    }
    if closed > 0 {
        tracing::info!(closed, "da dong tab thua/khoi phuc");
    }
    let page = pages
        .into_iter()
        .nth(keep)
        .ok_or(BrowserError::Disconnected)?;
    ops::navigate(&page, "about:blank").await?;
    Ok(page)
}

/// Task 13: bat xac thuc proxy `user:pass` bang `Fetch.*` THO tren PHIEN BROWSER (khong phai phien
/// trang). CHI goi khi [`proxy_credentials`] la `Some`.
///
/// Vi sao phien BROWSER (lech so voi brief `page.event_listener`): `NetworkManager` cua chromiumoxide
/// tra loi MOI `Fetch.authRequired` cua phien trang bang `continueWithAuth(Default)` ngay trong
/// `Target::on_event` (`handler/network.rs::on_fetch_auth_required`, khong co dieu kien) — TRUOC khi
/// listener cua ta nhan su kien, nen `ProvideCredentials` cua ta se toi sau va bi Chrome tu choi
/// ("khong con thu thach dang cho"). Su kien cua phien browser (khong `sessionId`) KHONG di qua
/// `Target` (`handler/mod.rs::on_event`) ma chi toi `Browser::event_listener`, va `Fetch` tren
/// browser target chan request cua MOI target (ke ca tab/worker mo sau). Van la Fetch tho: KHONG
/// `Page::authenticate`/`enable_request_intercept` (chung gui kem `Network.setCacheDisabled`).
///
/// Dang ky listener TRUOC `Fetch.enable` de khong lot su kien dau tien.
///
/// Review T13 M-5 (log): `Fetch.continueWithAuth` mang MAT KHAU trong payload CDP, va chromiumoxide
/// in MOI lenh gui di o muc `trace` (`conn.rs`). Chinh sach log o `lib.rs` (`chromiumoxide=warn`,
/// khong bao gio `debug`/`trace`) la DIEU KIEN de tinh nang nay khong lo mat khau ra log.
async fn enable_proxy_auth(
    browser: &Arc<Browser>,
    username: String,
    password: String,
) -> Result<(JoinHandle<()>, Arc<AtomicUsize>), BrowserError> {
    let paused = browser.event_listener::<EventRequestPaused>().await?;
    let auth = browser.event_listener::<EventAuthRequired>().await?;
    browser
        .execute(
            FetchEnableParams::builder()
                .handle_auth_requests(true)
                .pattern(RequestPattern::builder().url_pattern("*").build())
                .build(),
        )
        .await?;
    tracing::debug!("Fetch.enable(handleAuthRequests) tren phien browser — proxy co user/pass");
    let peak = Arc::new(AtomicUsize::new(0));
    let task = tokio::spawn(proxy_auth_loop(
        Arc::clone(browser),
        paused,
        auth,
        username,
        password,
        Arc::clone(&peak),
    ));
    Ok((task, peak))
}

/// Vong nghe su kien Fetch (task rieng, huy trong [`shutdown`]):
/// - `Fetch.requestPaused` → `Fetch.continueRequest` NGAY, khong sua gi (moi request deu bi tam dung
///   vi `Fetch.enable` can pattern `*` moi nhan duoc `authRequired` — Playwright cung vay).
/// - `Fetch.authRequired` nguon `Proxy` → `ProvideCredentials` lan dau cho moi request id; lan hai
///   cung id (sai mat khau) → `CancelAuth` de khong lap vo han (Playwright `_attemptedAuthentications`);
///   nguon `Server` (401 cua trang) → `Default` (khong phai viec cua ta).
///
/// Tap id da thu bi xoa khi qua [`AUTH_ATTEMPTED_CAP`] (id la duy nhat theo request, chi can nho
/// ngan han). KHONG log `ev.request` (URL co chu ky) va KHONG BAO GIO log `password`.
///
/// Review T13 I-1 — tra loi PIPELINE, khong tuan tu: hai nhanh su kien chi TAO future
/// `browser.execute(..)` va day vao `FuturesUnordered` (muon `&Browser`, khong clone `Arc` → `shutdown`
/// van `Arc::try_unwrap` duoc); nhanh thu ba gom ket qua. Mot round-trip CDP cham (Chrome ban, toi
/// `request_timeout` 30 s) khong con chan moi request khac cua ca trinh duyet — Playwright cung
/// `_sendMayFail` khong await trong handler. `peak` ghi dinh so lenh dang bay (bang chung smoke).
async fn proxy_auth_loop(
    browser: Arc<Browser>,
    mut paused: EventStream<EventRequestPaused>,
    mut auth: EventStream<EventAuthRequired>,
    username: String,
    password: String,
    peak: Arc<AtomicUsize>,
) {
    let browser: &Browser = &browser;
    let mut attempted: HashSet<String> = HashSet::new();
    let mut inflight: FuturesUnordered<
        BoxFuture<'_, Result<&'static str, chromiumoxide::error::CdpError>>,
    > = FuturesUnordered::new();
    loop {
        tokio::select! {
            ev = paused.next() => {
                let Some(ev) = ev else { break };
                let params = ContinueRequestParams::new(ev.request_id.clone());
                inflight.push(Box::pin(async move {
                    browser.execute(params).await.map(|_| "Fetch.continueRequest")
                }));
            }
            ev = auth.next() => {
                let Some(ev) = ev else { break };
                let id: &str = ev.request_id.as_ref();
                let is_proxy = matches!(ev.auth_challenge.source, Some(AuthChallengeSource::Proxy));
                let response = if !is_proxy {
                    AuthChallengeResponseResponse::Default
                } else if attempted.contains(id) {
                    AuthChallengeResponseResponse::CancelAuth
                } else {
                    if attempted.len() >= AUTH_ATTEMPTED_CAP {
                        attempted.clear();
                    }
                    attempted.insert(id.to_string());
                    AuthChallengeResponseResponse::ProvideCredentials
                };
                tracing::debug!(
                    source = ?ev.auth_challenge.source,
                    origin = %ev.auth_challenge.origin,
                    response = response.as_ref(),
                    "Fetch.authRequired"
                );
                let mut reply = AuthChallengeResponse::new(response.clone());
                if response == AuthChallengeResponseResponse::ProvideCredentials {
                    reply.username = Some(username.clone());
                    reply.password = Some(password.clone());
                }
                let params = ContinueWithAuthParams::new(ev.request_id.clone(), reply);
                inflight.push(Box::pin(async move {
                    browser.execute(params).await.map(|_| "Fetch.continueWithAuth")
                }));
            }
            Some(res) = inflight.next(), if !inflight.is_empty() => {
                if let Err(e) = res {
                    tracing::debug!(error = %e, "Fetch.continue* loi");
                }
            }
        }
        peak.fetch_max(inflight.len(), Ordering::Relaxed);
    }
    tracing::debug!("proxy_auth_loop ket thuc (luong su kien dong)");
}

/// Ket qua buoc chuan bi THUAN cua `launch()` (khong ghi dia): config da build + duong dan
/// profile TUYET DOI (dung cho `create_dir_all`/Preferences/lockfile, cung mot gia tri voi
/// `--user-data-dir`) + duong dan logs TUYET DOI (Task 14: `SubmitCtx::logs_dir`).
struct Prepared {
    config: BrowserConfig,
    profile_dir: PathBuf,
    logs_dir: PathBuf,
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
        // Task 14: cung mot ly do voi profile_dir — ep TUYET DOI ngay tu buoc chuan bi thuan de
        // ChromiumSession::submit() ghi dung `logs/no_conv_*.png` bat ke CWD tai thoi diem goi.
        let logs_dir = std::path::absolute(&opts.logs_dir)?;
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
            logs_dir,
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
            logs_dir,
        } = Self::prepare(opts)?;
        tokio::fs::create_dir_all(&profile_dir).await?;
        profile::disable_session_restore(&profile_dir).await?;
        probe_lockfile_busy(&profile_dir)?;
        let (mut browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| map_launch_error(&e))?;
        // Task 13 (I-7): Job Object KILL_ON_JOB_CLOSE ngay khi co Child — loi chi warn (vd tien
        // trinh nay nam trong Job cam long nhau), khong fail launch.
        let pid = browser.get_mut_child().and_then(|c| c.as_mut_inner().id());
        #[cfg(windows)]
        let job = match pid.map(crate::job::assign_kill_on_close) {
            Some(Ok(j)) => Some(j),
            Some(Err(e)) => {
                tracing::warn!(?pid, error = %e, "khong gan duoc Chrome vao Job Object — bo qua");
                None
            }
            None => {
                tracing::warn!("chromiumoxide khong tra Child — khong co Job Object");
                None
            }
        };
        let browser = Arc::new(browser);
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
        let mut parts = SessionParts {
            browser: Arc::clone(&browser),
            handler: Some(handler_task),
            proxy_auth: None,
            #[cfg(windows)]
            job,
        };
        let mut proxy_auth_peak: Option<Arc<AtomicUsize>> = None;
        // M-12: neu buoc nao duoi day loi, TRUOC day ham tra Err ngay va lam `browser` bi drop
        // — Drop cua Browser chi canh bao (khong doi CDP), va Child ben duoi kill_on_drop
        // (TerminateProcess) giet Chrome GIUA chung khi no co the dang ghi profile (giong hai
        // vi tri da fix o `close()`, xem review Task 3-6 I-3). Gom lai trong mot khoi de khi
        // loi thi dong nhe nhang (`shutdown`: Browser.close + doi thoat) truoc khi tra Err.
        let init_result: Result<Page, BrowserError> = async {
            // Task 13: proxy co user/pass → Fetch.* tren phien browser TRUOC khi tab dau dieu huong.
            if let Some((username, password)) = proxy_credentials(opts) {
                let (task, peak) = enable_proxy_auth(&browser, username, password).await?;
                parts.proxy_auth = Some(task);
                proxy_auth_peak = Some(peak);
            }
            let page = first_page(&browser).await?;
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
                drop(browser); // tha Arc cua ham nay de shutdown() try_unwrap duoc
                shutdown(parts, CLOSE_WAIT).await;
                return Err(e);
            }
        };
        drop(browser);
        let SessionParts {
            browser,
            handler,
            proxy_auth,
            #[cfg(windows)]
            job,
        } = parts;
        Ok(ChromiumSession {
            browser: Some(browser),
            page,
            handler,
            proxy_auth,
            proxy_auth_peak,
            #[cfg(windows)]
            job,
            pid,
            navigation_timeout: opts.navigation_timeout,
            proxy: opts.proxy.clone(),
            logs_dir,
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
        // Task 13: `ops::navigate` = Page.navigate tho → `errorText` thanh `Navigation{net_error}`
        // (`is_proxy_down()` cho engine); loi kenh → `Disconnected`. Thong diep timeout chi co
        // host/path (khong query — URL Dola co chu ky).
        let limit = s.navigation_timeout;
        tokio::time::timeout(limit, async {
            ops::navigate(&s.page, url).await?;
            s.page.wait_for_navigation().await?;
            Ok::<(), BrowserError>(())
        })
        .await
        .map_err(|_elapsed| {
            BrowserError::Timeout(format!(
                "dieu huong toi {} qua {limit:?}",
                crate::url_host_path(url)
            ))
        })?
    }

    async fn probe_session(&self, s: &ChromiumSession) -> Result<SessionProbe, BrowserError> {
        #[derive(serde::Deserialize)]
        struct Raw {
            texts: Vec<String>,
            composer: bool,
        }
        // I-2: gioi han urls=[DOLA_URL] thay vi Page::get_cookies() (Network.getCookies KHONG
        // urls -> cookie cua TRANG/TAB hien tai, co the la trang OAuth/loi sau redirect) —
        // khop Python context.cookies("https://www.dola.com"). Luat "co sessionid hop le" la
        // ham thuan lvc_core::has_dola_sessionid, test duoc khong can Chrome. Task 6: doc cookie
        // qua `ops::cookies` (dung chung voi `dola::session::assert_logged_in`).
        let cookies = crate::ops::cookies(&s.page, lvc_core::DOLA_URL).await?;
        let has_sessionid = crate::dola::session::has_sessionid(&cookies);
        let url = s.page.url().await?.unwrap_or_default();
        // Task 13 — Python `dola_session_state` (`browser.py:349-356`): chi doc TRANG khi co
        // sessionid; `wait_for(evaluate, 15)` het gio hoac nem (trang chua san sang / dang dieu
        // huong) → tra `Unknown` (caller khong ket luan), KHONG Err. Khong sessionid → `LoggedOut`
        // ngay (classify_session), khong evaluate.
        if !has_sessionid {
            return Ok(SessionProbe {
                has_sessionid,
                texts: Vec::new(),
                composer: false,
                url,
                // Review T13 M-3: mot nguon su that cho luat trang thai (= LoggedOut).
                state: lvc_core::classify_session(false, &[], false),
            });
        }
        let evaluated = tokio::time::timeout(PROBE_EVAL_TIMEOUT, async {
            s.page
                .evaluate(lvc_core::SESSION_PROBE_JS)
                .await?
                .into_value::<Raw>()
                .map_err(BrowserError::from)
        })
        .await;
        let raw = match evaluated {
            Ok(Ok(raw)) => raw,
            Ok(Err(e)) => {
                tracing::debug!(error = %e, "SESSION_PROBE_JS loi — state Unknown");
                return Ok(unknown_probe(has_sessionid, url));
            }
            Err(_elapsed) => {
                tracing::debug!(
                    ?PROBE_EVAL_TIMEOUT,
                    "SESSION_PROBE_JS qua gio — state Unknown"
                );
                return Ok(unknown_probe(has_sessionid, url));
            }
        };
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

    /// Task 13: [`shutdown`] voi [`CLOSE_WAIT`] (30 s). Sau khi lay het phan can dong ra, `Drop`
    /// cua `s` khong lam gi nua. Khong bao gio `Err` (dong la best-effort).
    async fn close(&self, mut s: ChromiumSession) -> Result<(), BrowserError> {
        if let Some(parts) = s.take_parts() {
            shutdown(parts, CLOSE_WAIT).await;
        }
        Ok(())
    }

    /// Task 14: uy quyen thang cho [`dola::submit`] qua `SubmitCtx { page, limits, logs_dir }`
    /// (`logs_dir` = duong dan TUYET DOI da chuan bi luc `launch()` — xem [`ChromiumSession`]).
    async fn submit(
        &self,
        s: &ChromiumSession,
        req: &SubmitRequest,
        limits: &Limits,
    ) -> Result<ConversationId, DolaError> {
        let ctx = dola::SubmitCtx {
            page: s.page(),
            limits,
            logs_dir: s.logs_dir.clone(),
        };
        dola::submit(&ctx, req).await
    }

    /// Task 14: dung `DolaHttp` MOI moi lan goi (re — xem [`DolaHttp::new`]) voi UA THAT cua
    /// phien va proxy da luu luc `launch()`, roi uy quyen cho [`dola::poll_and_download`].
    #[allow(clippy::too_many_arguments)] // khop chu ky ham thuan da chot o Task 11 (poll.rs)
    async fn poll_and_download(
        &self,
        s: &ChromiumSession,
        account: &str,
        conv: &ConversationId,
        out_dir: &Path,
        opts: &PollOpts,
        limits: &Limits,
        hooks: &dyn PollHooks,
    ) -> Result<VideoArtifact, DolaError> {
        let ua = ops::user_agent(s.page()).await?;
        let http = DolaHttp::new(ua, s.proxy.clone())?;
        dola::poll_and_download(s.page(), &http, account, conv, out_dir, opts, limits, hooks).await
    }

    async fn user_agent(&self, s: &ChromiumSession) -> Result<String, BrowserError> {
        ops::user_agent(s.page()).await
    }

    /// Task 17: [`ChromiumSession::is_connected`] — handler CDP con chay (Chrome con song).
    fn is_connected(&self, s: &ChromiumSession) -> bool {
        s.is_connected()
    }
}

/// `SessionProbe` khi khong doc duoc trang (timeout/loi evaluate): giu `has_sessionid` + URL,
/// state `Unknown`.
fn unknown_probe(has_sessionid: bool, url: String) -> SessionProbe {
    SessionProbe {
        has_sessionid,
        texts: Vec::new(),
        composer: false,
        url,
        state: SessionState::Unknown,
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

    /// Task 13: user/pass proxy KHONG lam dong lenh doi (chi `proxy-server=` + bypass) — xac thuc
    /// di qua Fetch.* sau khi Chrome mo, khong bao gio nam tren dong lenh (ai cung doc duoc qua
    /// `Win32_Process.CommandLine`).
    #[test]
    fn chrome_args_khong_doi_khi_proxy_co_auth() {
        let base = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            ..LaunchOpts::default()
        };
        let no_auth = LaunchOpts {
            proxy: lvc_core::parse_proxy("http://10.0.0.9:3128").unwrap(),
            ..base.clone()
        };
        let with_auth = LaunchOpts {
            proxy: lvc_core::parse_proxy("http://tenND:matkhauBM@10.0.0.9:3128").unwrap(),
            ..base
        };
        assert!(with_auth.proxy.as_ref().unwrap().has_auth());
        assert_eq!(chrome_args(&no_auth), chrome_args(&with_auth));
    }

    /// Task 13: `proxy_credentials` chi `Some` khi co username (password thieu → "").
    #[test]
    fn proxy_credentials_chi_khi_co_username() {
        let base = LaunchOpts {
            profile_dir: PathBuf::from("accounts/x"),
            ..LaunchOpts::default()
        };
        assert_eq!(proxy_credentials(&base), None);

        let no_auth = LaunchOpts {
            proxy: lvc_core::parse_proxy("1.2.3.4:8080").unwrap(),
            ..base.clone()
        };
        assert_eq!(proxy_credentials(&no_auth), None);

        let with_auth = LaunchOpts {
            proxy: lvc_core::parse_proxy("1.2.3.4:8080:user1:s3cr3tPW").unwrap(),
            ..base.clone()
        };
        assert_eq!(
            proxy_credentials(&with_auth),
            Some(("user1".to_string(), "s3cr3tPW".to_string()))
        );

        let mut spec = with_auth.proxy.clone().unwrap();
        spec.password = None;
        let no_password = LaunchOpts {
            proxy: Some(spec),
            ..base
        };
        assert_eq!(
            proxy_credentials(&no_password),
            Some(("user1".to_string(), String::new()))
        );
    }

    /// M-3 / Python `fresh_page`: uu tien `about:blank` (URL `None`/rong cung coi la blank), khong
    /// thi tab dau khong rac; tab rac (Opera GX) khong bao gio duoc chon; toan rac → `None`.
    #[test]
    fn choose_main_page_uu_tien_about_blank_va_bo_rac() {
        let u = |s: &str| Some(s.to_string());
        assert_eq!(choose_main_page(&[]), None);
        assert_eq!(choose_main_page(&[None]), Some(0));
        assert_eq!(choose_main_page(&[u("about:blank")]), Some(0));
        assert_eq!(
            choose_main_page(&[u("chrome://new-tab-page/"), u("about:blank")]),
            Some(1)
        );
        assert_eq!(
            choose_main_page(&[u("https://www.dola.com/chat"), u("chrome://newtab/")]),
            Some(0),
            "khong co about:blank → tab dau"
        );
        assert_eq!(
            choose_main_page(&[u("https://gxcorner.games/"), u("https://a.b/")]),
            Some(1),
            "tab rac khong duoc chon du dung dau"
        );
        assert_eq!(
            choose_main_page(&[u("https://GXCORNER.games/x"), u("")]),
            Some(1),
            "URL rong = about:blank; so sanh host rac khong phan biet hoa thuong"
        );
        assert_eq!(choose_main_page(&[u("https://gxcorner.games/")]), None);
        // M-2: tab khong doc duoc URL doi xu nhu rac
        assert_eq!(choose_main_page(&[u(UNREADABLE_URL)]), None);
        assert_eq!(
            choose_main_page(&[u(UNREADABLE_URL), u("chrome://newtab/")]),
            Some(1)
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
