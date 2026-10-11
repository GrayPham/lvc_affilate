//! Mint token reCAPTCHA bang 1 trinh duyet KHACH (an danh), dung lai TUAN TU qua CDP. Port tu
//! `Lvc_veo3_automation/src/core/flow_cdp_captcha/{minter,browser,config}.py`.
//!
//! KHONG cookie tai khoan nao duoc nap vao trinh duyet nay — profile tam, xoa sau khi dong. Day
//! la diem MAU CHOT khien token duoc Google chap nhan (mint trong phien tai khoan THAT LUON bi
//! `PUBLIC_ERROR_UNUSUAL_ACTIVITY` — xem MEMORY.md).
//!
//! **Khong goi day la "pool"**: chi co DUY NHAT 1 trinh duyet/profile song tai mot thoi diem,
//! dung lai cho nhieu lan mint (khong phai nhieu instance chay song song, khong phai dich vu/
//! extension nao). `GuestCaptchaBrowser` la 1 phien don; goi tuan tu tu nhieu task dong thoi thi
//! boc no trong `tokio::sync::Mutex` o tang tren (`lvc-engine`) — khong can tu dung hang doi/luong
//! thu rieng nhu ban Python (Selenium khong thread-safe nen Python buoc phai lam vay; chromiumoxide
//! la async thuan, `Mutex` la du).

use std::path::{Path, PathBuf};
use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::page::SetBypassCspParams;
use futures::StreamExt;
use lvc_core::BrowserKind;
use tokio::task::JoinHandle;

use crate::{BrowserError, executable};

/// Trang mint: trang tron, KHONG co app Flow nen khong co widget reCAPTCHA nao cua Flow de token
/// bi rang vao — day la diem khien token mint o day duoc Google nhan (khac trang du an da login).
pub const MINT_PAGE: &str = "https://flow.google.com/about";
/// Site key reCAPTCHA Enterprise cua Flow — hang so, dung chung anh+video.
pub const SITE_KEY: &str = "6LdsFiUsAAAAAIjVDZcuLhaHiDn5nnHVXVRQGeMV";
const ENTERPRISE_JS_URL: &str = "https://www.google.com/recaptcha/enterprise.js";
/// Token thuc cua Flow dai ~2200-2700 ky tu; ngan hon coi nhu rac (reCAPTCHA da nghi ngo phien).
pub const MIN_TOKEN_LEN: usize = 500;
/// Xoay profile sau khi mint qua nhieu token, hoac ra token ngan lien tiep — dung doi den luc hong
/// han moi thay (xem [`GuestCaptchaBrowser::should_rotate`]).
pub const MAX_TOKENS_PER_PROFILE: u32 = 25;
pub const MAX_CONSECUTIVE_SHORT_TOKENS: u32 = 3;

const GUEST_ARGS: &[&str] = &[
    "no-first-run",
    "no-default-browser-check",
    "disable-blink-features=AutomationControlled",
    "disable-background-networking",
    "disable-sync",
    "disable-features=Translate,OptimizationHints",
    // Day cua so ra ngoai man hinh de an ma van "visible" voi trang — headless/minimize deu la
    // tin hieu xau cho reCAPTCHA (doi document.visibilityState). Xem MEMORY.md.
    "window-position=-32000,-32000",
];

/// Tuy chon mo trinh duyet khach. `executable_override`: dung Chrome THAT (khong phai Chromium
/// pinned cua patchright dung cho Dola) — [`BrowserKind::Chrome`] luon duoc dung, khong cho doi.
#[derive(Debug, Clone)]
pub struct GuestCaptchaOpts {
    pub executable_override: Option<PathBuf>,
    /// Thoi gian toi da cho Chrome khoi dong + nap xong trang mint + `enterprise.js` san sang.
    pub open_timeout: Duration,
    /// Thoi gian toi da cho MOT lan mint (`grecaptcha.enterprise.execute`).
    pub mint_timeout: Duration,
}

impl Default for GuestCaptchaOpts {
    fn default() -> Self {
        Self {
            executable_override: None,
            open_timeout: Duration::from_secs(90),
            mint_timeout: Duration::from_secs(60),
        }
    }
}

/// Mot phien trinh duyet khach DANG SONG. Khong `Clone`/`Sync` co chu y — chi 1 chu so huu tai
/// mot thoi diem (xem doc module ve ly do khong can "pool").
pub struct GuestCaptchaBrowser {
    browser: Browser,
    handler: JoinHandle<()>,
    page: Page,
    profile_dir: PathBuf,
    token_count: u32,
    short_token_streak: u32,
}

impl GuestCaptchaBrowser {
    /// Mo Chrome that voi 1 profile KHACH moi (`<root_dir>/guest_<uuid>`), tat CSP cua trang
    /// (`Page.setBypassCSP` — PHAI goi truoc khi dieu huong), vao [`MINT_PAGE`], cheo script
    /// `enterprise.js`, cho `grecaptcha.enterprise` san sang.
    ///
    /// # Errors
    /// [`BrowserError::ExecutableNotFound`] (khong co Chrome), [`BrowserError::Launch`]/`Cdp`
    /// (khoi dong/dieu huong loi), [`BrowserError::Timeout`] (qua [`GuestCaptchaOpts::open_timeout`]
    /// ma `enterprise.js` chua san sang).
    pub async fn open(root_dir: &Path, opts: &GuestCaptchaOpts) -> Result<Self, BrowserError> {
        let profile_dir = root_dir.join(format!("guest_{}", uuid::Uuid::new_v4().simple()));
        tokio::fs::create_dir_all(&profile_dir).await?;
        let exe = executable::resolve_executable(
            BrowserKind::Chrome,
            opts.executable_override.as_deref(),
        )?;
        let mut builder = BrowserConfig::builder()
            .user_data_dir(&profile_dir)
            .chrome_executable(exe)
            .launch_timeout(opts.open_timeout)
            .viewport(None)
            .window_size(1280, 860)
            .disable_default_args()
            .with_head(); // KHONG headless — xem MEMORY.md (headless la dau hieu xau cho reCAPTCHA)
        for arg in GUEST_ARGS {
            builder = builder.arg(*arg);
        }
        let config = builder.build().map_err(BrowserError::Launch)?;
        let (mut browser, mut handler) = Browser::launch(config).await?;
        let handler_task = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
        match init_page(&browser, opts).await {
            Ok(page) => Ok(Self {
                browser,
                handler: handler_task,
                page,
                profile_dir,
                token_count: 0,
                short_token_streak: 0,
            }),
            Err(e) => {
                let _ = tokio::time::timeout(Duration::from_secs(10), browser.close()).await;
                handler_task.abort();
                let _ = tokio::fs::remove_dir_all(&profile_dir).await;
                Err(e)
            }
        }
    }

    /// Mot token tuoi. Dung NGAY, dung de danh (song ~90-120s, dung 1 lan).
    ///
    /// # Errors
    /// [`BrowserError::Timeout`] qua [`GuestCaptchaOpts::mint_timeout`]; [`BrowserError::Cdp`]/
    /// `Json` khi lenh CDP hoac giai ma ket qua loi.
    pub async fn mint(&mut self, action: &str, timeout: Duration) -> Result<String, BrowserError> {
        let js = mint_script(action);
        let token: String = tokio::time::timeout(timeout, self.page.evaluate_function(js))
            .await
            .map_err(|_elapsed| BrowserError::Timeout(format!("mint reCAPTCHA qua {timeout:?}")))??
            .into_value()
            .map_err(BrowserError::from)?;
        self.token_count += 1;
        if token.len() < MIN_TOKEN_LEN {
            self.short_token_streak += 1;
        } else {
            self.short_token_streak = 0;
        }
        Ok(token)
    }

    /// Da toi luc bo profile nay dung cai moi chua (qua nhieu token, hoac lien tiep ra token
    /// ngan — dau hieu Google da giam diem tin cay profile nay).
    #[must_use]
    pub fn should_rotate(&self) -> bool {
        self.token_count >= MAX_TOKENS_PER_PROFILE
            || self.short_token_streak >= MAX_CONSECUTIVE_SHORT_TOKENS
    }

    /// Dong Chrome (best-effort) va xoa thu muc profile khach. Khong bao gio panic/`Err` — day
    /// la don dep, khong phai tao tac vu quan trong nhu phien tai khoan.
    pub async fn close(mut self) {
        let _ = tokio::time::timeout(Duration::from_secs(10), self.browser.close()).await;
        self.handler.abort();
        if let Err(e) = tokio::fs::remove_dir_all(&self.profile_dir).await {
            tracing::debug!(error = %e, dir = %self.profile_dir.display(), "khong xoa duoc profile khach — bo qua");
        }
    }
}

/// Mo tab, tat CSP, vao [`MINT_PAGE`], cheo `enterprise.js`, cho san sang. Tach khoi
/// [`GuestCaptchaBrowser::open`] vi luc nay chua co `Self` day du (chi co `browser`) — loi giua
/// duong phai don duoc `browser`/`profile_dir` o `open()`, khong the goi `self.close()`.
async fn init_page(browser: &Browser, opts: &GuestCaptchaOpts) -> Result<Page, BrowserError> {
    let page = browser.new_page("about:blank").await?;
    page.execute(SetBypassCspParams::new(true)).await?;
    page.goto(MINT_PAGE).await?;
    page.wait_for_navigation().await?;
    let inject = format!(
        "(function(){{var s=document.createElement('script');s.src='{ENTERPRISE_JS_URL}?render={SITE_KEY}';s.async=true;document.head.appendChild(s);}})();"
    );
    page.evaluate_expression(inject).await?;
    wait_grecaptcha_ready(&page, opts.open_timeout).await?;
    Ok(page)
}

/// JS mint 1 token: cho `grecaptcha.enterprise.ready` roi `execute(SITE_KEY, {{action}})`. `action`
/// chi nhan hang so noi bo (`"IMAGE_GENERATION"`/`"IMAGE_UPLOAD"`/`"VIDEO_GENERATION"`) — khong
/// bao gio nhan du lieu nguoi dung, an toan noi truc tiep vao chuoi JS.
fn mint_script(action: &str) -> String {
    format!(
        "async function() {{ return await new Promise(function(resolve) {{ \
         grecaptcha.enterprise.ready(function() {{ \
         grecaptcha.enterprise.execute('{SITE_KEY}', {{action: '{action}'}}) \
         .then(function(t) {{ resolve(t || ''); }}).catch(function() {{ resolve(''); }}); \
         }}); }}); }}"
    )
}

const GRECAPTCHA_READY_EXPR: &str = "typeof grecaptcha !== 'undefined' && grecaptcha.enterprise && typeof grecaptcha.enterprise.execute === 'function'";

async fn wait_grecaptcha_ready(page: &Page, timeout: Duration) -> Result<(), BrowserError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let ready = page
            .evaluate(GRECAPTCHA_READY_EXPR)
            .await
            .ok()
            .and_then(|v| v.into_value::<bool>().ok())
            .unwrap_or(false);
        if ready {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(BrowserError::Timeout(format!(
                "enterprise.js khong san sang sau {timeout:?}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_script_noi_dung_action_dung_cho() {
        let js = mint_script("VIDEO_GENERATION");
        assert!(js.contains("grecaptcha.enterprise.execute('6LdsFiUsAAAAAIjVDZcuLhaHiDn5nnHVXVRQGeMV', {action: 'VIDEO_GENERATION'})"));
    }

    #[test]
    fn should_rotate_theo_so_token() {
        let mut b = fake_state();
        assert!(!b.0);
        b.1 = MAX_TOKENS_PER_PROFILE;
        assert!(should_rotate_from(b.1, 0));
    }

    #[test]
    fn should_rotate_theo_token_ngan_lien_tiep() {
        assert!(should_rotate_from(0, MAX_CONSECUTIVE_SHORT_TOKENS));
        assert!(!should_rotate_from(0, MAX_CONSECUTIVE_SHORT_TOKENS - 1));
    }

    // Gia lap dung tri so `should_rotate` ma khong can mo Chrome thuc (khong the dung
    // `GuestCaptchaBrowser` truc tiep vi truong `browser`/`page` doi Chrome that).
    fn should_rotate_from(token_count: u32, short_token_streak: u32) -> bool {
        token_count >= MAX_TOKENS_PER_PROFILE || short_token_streak >= MAX_CONSECUTIVE_SHORT_TOKENS
    }

    fn fake_state() -> (bool, u32) {
        (false, 0)
    }
}
