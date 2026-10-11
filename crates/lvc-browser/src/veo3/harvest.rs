//! "Harvest" phien mot tai khoan Google/Flow — mo profile Chromium THAT (da dang nhap tu truoc),
//! doc `at`/`bl`/`f.sid` (`window.WIZ_global_data`) + cookie SONG cua `flow.google.com`, roi DONG
//! trinh duyet NGAY (khong giu session song — ADR-R018: goi request thuc sau do la HTTP thuan o
//! `lvc-veo3::client`, khong can trinh duyet). Port tu
//! `Lvc_veo3_automation/src/core/flow_account_runner.py`, `FlowAccountRunner.open()` (dong 110-267)
//! + guard cookie cua `get_shared_flow_client` (dong 552-564).
//!
//! Module nay TU DUNG nhu `captcha.rs` — dung `chromiumoxide` TRUC TIEP (khong qua trait
//! `BrowserDriver`), KHONG phu thuoc crate provider `lvc-veo3`. Tra ve 1 struct thuan cuc bo
//! ([`HarvestedSession`]) — viec dich sang `lvc_veo3::FlowSession` la cua `lvc-engine` (noi duy
//! nhat phu thuoc ca hai crate).
//!
//! Khac `captcha.rs` o 2 diem quan trong (xem `MEMORY.md`):
//! - Dung [`lvc_core::BrowserKind::Chromium`] (Chromium ghim cua patchright, giong Dola) —
//!   KHONG dung `BrowserKind::Chrome`: profile o day la profile ACCOUNT da ton tai lau dai, giong
//!   `dola/` chu khong giong trinh duyet KHACH dung-mot-lan cua `captcha.rs`.
//! - KHONG dua cua so ra ngoai man hinh (`window-position=-32000,-32000`) — Chrome LUU toa do do
//!   vao `Preferences` cua profile THAT, lam profile "hong" (mo tay lan sau cung ra ngoai man
//!   hinh) — day la loi Python da gap va tranh (`flow_account_runner.py` dong 142-144, 201-205).

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use lvc_core::BrowserKind;

use crate::{BrowserError, executable, ops};

/// Goc `flow.google.com` — dung chung de dieu huong (khi khong co `project_id`) va de gioi han
/// [`ops::cookies`] (khong keo cookie `accounts.google.com` khac domain vao header gui di).
const FLOW_URL: &str = "https://flow.google.com";

/// Co dong lenh cho profile ACCOUNT (khac han `captcha::GUEST_ARGS` — xem doc module). KHONG co
/// `window-position` (xem doc module). Tu dung, KHONG import `chromium::LAUNCH_ARGS`/
/// `chrome_args()` cua Dola: ham do luon ep them `--lang=ja-JP` + Accept-Language/timezone Nhat
/// (dac thu san pham Dola) — khong hop ly cho mot tai khoan Google Flow bat ky.
const ACCOUNT_ARGS: &[&str] = &[
    "no-first-run",
    "no-default-browser-check",
    "disable-blink-features=AutomationControlled",
    "disable-background-networking",
    "no-sandbox",
    "disable-dev-shm-usage",
];

/// 4 khoa cookie dang nhap THAT cua Google — chi can co IT NHAT 1 (gia tri khong rong) moi coi la
/// "co phien dang nhap". Port nguyen van tu Python (`_AUTH_COOKIE_KEYS`, dong 558).
const AUTH_COOKIE_KEYS: [&str; 4] = [
    "SID",
    "__Secure-1PSID",
    "__Secure-3PSID",
    "__Secure-1PSIDTS",
];

/// Tuy chon harvest. `project_id`: neu caller da biet (vd luu tu lan truoc) thi dieu huong thang
/// toi `/project/<id>`; de trong thi vao trang goc `/` va doc `project_id` tu URL sau khi Flow tu
/// dieu huong (giong Python: `proj = proj or self.project_id`).
#[derive(Debug, Clone)]
pub struct HarvestOpts {
    /// Dung Chrome THAT (khong phai Chromium 1234 cua patchright) — chi doi khi ep bang env/test.
    pub executable_override: Option<PathBuf>,
    pub project_id: Option<String>,
    /// Thoi gian toi da cho tien trinh Chrome khoi dong + CDP san sang (KHONG tinh thoi gian tai
    /// trang — cai do do [`ops::goto`] lo qua `ops::RELOAD_TIMEOUT`, co dinh 60s).
    pub launch_timeout: Duration,
    /// Khoang nghi giua 2 lan poll `window.WIZ_global_data`.
    pub poll_interval: Duration,
    /// So lan poll toi da (Python: 30 lan x 1s ~ 30s).
    pub poll_attempts: u32,
}

impl Default for HarvestOpts {
    fn default() -> Self {
        Self {
            executable_override: None,
            project_id: None,
            launch_timeout: Duration::from_secs(30),
            poll_interval: Duration::from_secs(1),
            poll_attempts: 30,
        }
    }
}

/// Ket qua 1 lan harvest — struct THUAN cuc bo (khong phai `lvc_veo3::FlowSession`, xem doc
/// module). `Debug` che `cookie_header`/`at`/`f_sid` (trang thai phien nhay cam) — chi `bl` (nhan
/// build, khong nhay cam) va `project_id` hien nguyen van; giong quy uoc `ops::CookieKv`.
#[derive(Clone)]
pub struct HarvestedSession {
    /// Header `Cookie: a=b; c=d` cua domain `flow.google.com` — dung cho request HTTP thuan sau
    /// nay (`lvc-veo3::client`). KHONG BAO GIO log gia tri.
    pub cookie_header: String,
    /// XSRF token (`window.WIZ_global_data.SNlM0e`).
    pub at: String,
    /// Nhan build (`window.WIZ_global_data.cfb2h`).
    pub bl: String,
    /// Session id trang (`window.WIZ_global_data.FdrFJe`).
    pub f_sid: String,
    /// Doc tu URL sau dieu huong, hoac giu nguyen `HarvestOpts::project_id` neu URL khong co.
    pub project_id: String,
}

impl fmt::Debug for HarvestedSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HarvestedSession")
            .field("cookie_header", &"***")
            .field("at", &"***")
            .field("bl", &self.bl)
            .field("f_sid", &"***")
            .field("project_id", &self.project_id)
            .finish()
    }
}

/// Snapshot 1 lan doc `window.WIZ_global_data` (+ `project_id` tu URL) — khop dang JSON tra ve tu
/// [`WIZ_READ_JS`]. Tach rieng struct nay (thay vi doc thang vao [`HarvestedSession`]) de deserialize
/// duoc bang `serde` va test THUAN khong can Chrome (xem `tests::parse_wiz_snapshot_*`).
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct WizSnapshot {
    at: Option<String>,
    bl: Option<String>,
    fsid: Option<String>,
    proj: Option<String>,
}

/// JS doc `window.WIZ_global_data` + tu doc `project_id` tu URL hien tai — port nguyen y tuong
/// `_wiz` cua Python (dong 233-235): `SNlM0e`=`at`, `cfb2h`=`bl`, `FdrFJe`=`f.sid`,
/// `location.pathname.match(/project\/([0-9a-fA-F-]{8,})/)`.
const WIZ_READ_JS: &str = "(() => { \
    const w = window.WIZ_global_data || {}; \
    const m = (location.pathname.match(/project\\/([0-9a-fA-F-]{8,})/) || []); \
    return { at: w.SNlM0e || null, bl: w.cfb2h || null, fsid: w.FdrFJe || null, proj: m[1] || null }; \
})()";

/// URL dich: co `project_id` -> `/project/<id>`, khong thi vao goc `/` (Python dong 222). Thuan.
fn target_url(project_id: Option<&str>) -> String {
    match project_id {
        Some(id) if !id.is_empty() => format!("{FLOW_URL}/project/{id}"),
        _ => format!("{FLOW_URL}/"),
    }
}

/// URL sau dieu huong cho thay profile CHUA dang nhap flow (Python dong 229): chua `/about`,
/// `accounts.google.com`, hoac "signin" (khong phan biet hoa/thuong). Thuan.
fn looks_logged_out(url: &str) -> bool {
    url.contains("/about")
        || url.contains("accounts.google.com")
        || url.to_ascii_lowercase().contains("signin")
}

/// Guard bat buoc (Python dong 558-564): PHAI co it nhat 1 [`AUTH_COOKIE_KEYS`] voi gia tri
/// KHONG RONG. Co `at` nhung cookie rong/thieu het cac khoa nay van la LOI — day la nguyen nhan
/// loi thuc da gap (cache "half-baked" gay 401 day chuyen), xem `MissingAuthCookie`. Thuan.
fn has_auth_cookie(cookies: &[ops::CookieKv]) -> bool {
    cookies
        .iter()
        .any(|c| AUTH_COOKIE_KEYS.contains(&c.name.as_str()) && !c.value.is_empty())
}

/// Ghep [`ops::CookieKv`] thanh header `Cookie` (cong thuc giong [`ops::cookie_header`], VIET LAI
/// o day thay vi goi lai): `harvest_session` da doc `cookies` MOT LAN cho guard
/// ([`has_auth_cookie`]), dung lai chinh danh sach do de dung KHONG can doc cookie LAN THU HAI
/// qua CDP. Thuan.
fn join_cookie_header(cookies: &[ops::CookieKv]) -> String {
    cookies
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Danh sach TEN cookie da thay (sap xep, cat 120 ky tu) — dung cho thong diep loi
/// [`BrowserError::MissingAuthCookie`] chan doan (AV/khoa profile chan cookie DB) MA KHONG BAO GIO
/// lo gia tri. Khop Python `",".join(sorted(_ck.keys()))[:120]`. Thuan.
fn cookie_names_joined(cookies: &[ops::CookieKv]) -> String {
    let mut names: Vec<&str> = cookies.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    names.join(",").chars().take(120).collect()
}

/// Poll `window.WIZ_global_data` toi da `opts.poll_attempts` lan (nghi `opts.poll_interval` giua
/// hai lan), dung NGAY khi `at` xuat hien (Python dong 236-244).
///
/// # Errors
/// [`BrowserError::Timeout`] khi het so lan poll ma van chua thay `at`; loi CDP/JS khac tu
/// [`ops::eval_json`] duoc tra thang (khong thu lai — khac `context_gone`, `WIZ_global_data` khong
/// dieu huong lai giua chung nhu Dola).
async fn poll_wiz_snapshot(page: &Page, opts: &HarvestOpts) -> Result<WizSnapshot, BrowserError> {
    let attempts = opts.poll_attempts.max(1);
    for attempt in 0..attempts {
        let snap: WizSnapshot = ops::eval_json(page, WIZ_READ_JS).await?;
        if snap.at.as_deref().is_some_and(|s| !s.is_empty()) {
            return Ok(snap);
        }
        if attempt + 1 < attempts {
            tokio::time::sleep(opts.poll_interval).await;
        }
    }
    Err(BrowserError::Timeout(format!(
        "khong doc duoc at (WIZ_global_data.SNlM0e) sau {attempts} lan poll ({:?} moi lan)",
        opts.poll_interval
    )))
}

/// Mo mot tab, dieu huong toi trang project/goc, kiem tra da dang nhap, poll token, doc cookie va
/// ap guard — toan bo phan "sau khi Chrome da mo" (tach khoi [`harvest_session`] de ham do luon
/// dong duoc Chrome du nhanh nay tra `Ok` hay `Err`, giong cach `captcha::init_page` tach khoi
/// `GuestCaptchaBrowser::open`).
async fn harvest_from_open_browser(
    browser: &Browser,
    opts: &HarvestOpts,
) -> Result<HarvestedSession, BrowserError> {
    let page = browser.new_page("about:blank").await?;
    let url = target_url(opts.project_id.as_deref());
    ops::goto(&page, &url, Duration::from_millis(300)).await?;
    let current_url = ops::url(&page).await?;
    if looks_logged_out(&current_url) {
        return Err(BrowserError::NotLoggedIn(current_url));
    }
    let snap = poll_wiz_snapshot(&page, opts).await?;
    // poll_wiz_snapshot() chi tra Ok khi `at` khong rong — unwrap_or_default an toan.
    let at = snap.at.unwrap_or_default();
    let cookies = ops::cookies(&page, FLOW_URL).await?;
    if !has_auth_cookie(&cookies) {
        return Err(BrowserError::MissingAuthCookie(cookie_names_joined(
            &cookies,
        )));
    }
    let project_id = snap
        .proj
        .filter(|s| !s.is_empty())
        .or_else(|| opts.project_id.clone())
        .unwrap_or_default();
    Ok(HarvestedSession {
        cookie_header: join_cookie_header(&cookies),
        at,
        bl: snap.bl.unwrap_or_default(),
        f_sid: snap.fsid.unwrap_or_default(),
        project_id,
    })
}

/// Mo profile Chromium THAT tai `profile_dir` (do CALLER cung cap — module nay KHONG tu doan
/// duong dan app-base, giong `LaunchOpts::profile_dir` cua `dola/driver.rs`), harvest
/// `at`/`bl`/`f.sid` + cookie cua `flow.google.com`, roi DONG trinh duyet NGAY — du thanh cong hay
/// loi (ADR-R018: harvest 1 lan, khong giu session song).
///
/// # Errors
/// [`BrowserError::Launch`] khi `profile_dir` khong ton tai/khong phai thu muc;
/// [`BrowserError::ExecutableNotFound`] khong thay Chromium; [`BrowserError::NotLoggedIn`] khi URL
/// sau dieu huong cho thay profile chua dang nhap flow; [`BrowserError::Timeout`] khong doc duoc
/// `at` sau [`HarvestOpts::poll_attempts`] lan poll; [`BrowserError::MissingAuthCookie`] khi doc
/// duoc `at` nhung cookie thieu het cac khoa dang nhap ([`AUTH_COOKIE_KEYS`]); loi CDP/`Navigation`/
/// I-O khac tu chromiumoxide.
pub async fn harvest_session(
    profile_dir: &Path,
    opts: &HarvestOpts,
) -> Result<HarvestedSession, BrowserError> {
    if !profile_dir.is_dir() {
        return Err(BrowserError::Launch(format!(
            "profile khong ton tai hoac khong phai thu muc (can dang nhap Google truoc): {}",
            profile_dir.display()
        )));
    }
    let profile_dir_abs = std::path::absolute(profile_dir)?;
    let exe =
        executable::resolve_executable(BrowserKind::Chromium, opts.executable_override.as_deref())?;
    let mut builder = BrowserConfig::builder()
        .user_data_dir(&profile_dir_abs)
        .chrome_executable(exe)
        .launch_timeout(opts.launch_timeout)
        .viewport(None)
        .window_size(1000, 720)
        .disable_default_args()
        .with_head(); // KHONG headless — cung ly do voi captcha.rs (UA "HeadlessChrome" la dau bot)
    for arg in ACCOUNT_ARGS {
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
    let outcome = harvest_from_open_browser(&browser, opts).await;
    // Harvest MOT LAN roi DONG NGAY (ADR-R018) — best-effort, khong de loi dong che mat ket qua
    // harvest thuc (thanh cong hay loi deu tra ve dung `outcome` da tinh o tren).
    let _ = tokio::time::timeout(Duration::from_secs(10), browser.close()).await;
    handler_task.abort();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::CookieKv;

    #[test]
    fn target_url_goc_khi_khong_co_project_id() {
        assert_eq!(target_url(None), "https://flow.google.com/");
        assert_eq!(target_url(Some("")), "https://flow.google.com/");
    }

    #[test]
    fn target_url_co_project_id() {
        assert_eq!(
            target_url(Some("abc123")),
            "https://flow.google.com/project/abc123"
        );
    }

    #[test]
    fn looks_logged_out_nhan_dung_3_dau_hieu() {
        assert!(looks_logged_out("https://flow.google.com/about"));
        assert!(looks_logged_out(
            "https://accounts.google.com/signin/v2/identifier"
        ));
        assert!(looks_logged_out(
            "https://flow.google.com/SignIn?continue=x"
        ));
        assert!(!looks_logged_out(
            "https://flow.google.com/project/deadbeef-1234"
        ));
        assert!(!looks_logged_out("https://flow.google.com/"));
    }

    #[test]
    fn parse_wiz_snapshot_du_truong() {
        let v = serde_json::json!({
            "at": "xsrf-tok",
            "bl": "boq_flow_1",
            "fsid": "-1234567890",
            "proj": "deadbeef-aaaa-bbbb-cccc-1234567890ab",
        });
        let snap: WizSnapshot = serde_json::from_value(v).unwrap();
        assert_eq!(snap.at.as_deref(), Some("xsrf-tok"));
        assert_eq!(snap.bl.as_deref(), Some("boq_flow_1"));
        assert_eq!(snap.fsid.as_deref(), Some("-1234567890"));
        assert_eq!(
            snap.proj.as_deref(),
            Some("deadbeef-aaaa-bbbb-cccc-1234567890ab")
        );
    }

    /// JS tra `null` cho truong thieu (Python: `w.SNlM0e||null`) — phai deserialize thanh `None`,
    /// khong loi (khop `poll_wiz_snapshot` coi `at: None` la "chua co, poll tiep").
    #[test]
    fn parse_wiz_snapshot_thieu_truong_thanh_none() {
        let v = serde_json::json!({"at": null, "bl": null, "fsid": null, "proj": null});
        let snap: WizSnapshot = serde_json::from_value(v).unwrap();
        assert!(snap.at.is_none());
        assert!(snap.bl.is_none());
        assert!(snap.fsid.is_none());
        assert!(snap.proj.is_none());
    }

    #[test]
    fn has_auth_cookie_can_it_nhat_1_khoa_hop_le() {
        let ok = vec![CookieKv::new(".google.com", "SID", "gia-tri-bi-mat")];
        assert!(has_auth_cookie(&ok));
        let ok2 = vec![CookieKv::new("flow.google.com", "__Secure-1PSIDTS", "x")];
        assert!(has_auth_cookie(&ok2));
        let khong_lien_quan = vec![CookieKv::new(".google.com", "NID", "abc")];
        assert!(!has_auth_cookie(&khong_lien_quan));
        assert!(!has_auth_cookie(&[]));
    }

    /// Loi thuc da gap ben Python (dong 552-557): khoa dang nhap CO MAT nhung GIA TRI RONG (AV/
    /// khoa profile chan doc cookie DB) — PHAI coi la khong hop le, khong duoc pass guard.
    #[test]
    fn has_auth_cookie_khoa_dung_nhung_gia_tri_rong_thi_khong_hop_le() {
        let nua_vo = vec![
            CookieKv::new(".google.com", "SID", ""),
            CookieKv::new(".google.com", "__Secure-1PSID", ""),
        ];
        assert!(!has_auth_cookie(&nua_vo));
    }

    #[test]
    fn join_cookie_header_ghep_dung_dinh_dang() {
        let cookies = vec![
            CookieKv::new(".google.com", "SID", "a"),
            CookieKv::new(".google.com", "HSID", "b"),
        ];
        assert_eq!(join_cookie_header(&cookies), "SID=a; HSID=b");
        assert_eq!(join_cookie_header(&[]), "");
    }

    #[test]
    fn cookie_names_joined_sap_xep_va_khong_lo_gia_tri() {
        let cookies = vec![
            CookieKv::new(".google.com", "HSID", "bi-mat-1"),
            CookieKv::new(".google.com", "NID", "bi-mat-2"),
        ];
        let s = cookie_names_joined(&cookies);
        assert_eq!(s, "HSID,NID");
        assert!(!s.contains("bi-mat"));
    }

    #[test]
    fn cookie_names_joined_cat_toi_da_120_ky_tu() {
        let cookies: Vec<CookieKv> = (0..30)
            .map(|i| CookieKv::new(".google.com", &format!("COOKIE_TEN_DAI_{i:02}"), "v"))
            .collect();
        let s = cookie_names_joined(&cookies);
        assert!(s.chars().count() <= 120, "do dai={}", s.chars().count());
    }

    /// Quan trong (xem MEMORY.md): KHONG dua cua so ra ngoai man hinh cho profile ACCOUNT — khac
    /// han `captcha::GUEST_ARGS` (profile khach dung mot lan roi xoa). Chrome LUU toa do
    /// `window-position` vao `Preferences` cua profile THAT -> mo tay lan sau cung bi anh huong.
    #[test]
    fn account_args_khong_dua_cua_so_ra_ngoai_man_hinh() {
        assert!(!ACCOUNT_ARGS.iter().any(|a| a.contains("window-position")));
    }

    #[test]
    fn harvested_session_debug_che_bi_mat_giu_lai_bl_va_project_id() {
        let s = HarvestedSession {
            cookie_header: "SID=bi-mat-cookie".into(),
            at: "at-token-bi-mat".into(),
            bl: "boq_flow_1".into(),
            f_sid: "fsid-bi-mat".into(),
            project_id: "deadbeef-1234".into(),
        };
        let dbg = format!("{s:?}");
        assert!(!dbg.contains("bi-mat"), "lo bi mat: {dbg}");
        assert!(dbg.contains("boq_flow_1"), "{dbg}");
        assert!(dbg.contains("deadbeef-1234"), "{dbg}");
        assert!(dbg.contains("***"), "{dbg}");
    }

    #[test]
    fn default_hop_ly() {
        let d = HarvestOpts::default();
        assert!(d.executable_override.is_none());
        assert!(d.project_id.is_none());
        assert_eq!(d.poll_attempts, 30);
        assert_eq!(d.poll_interval, Duration::from_secs(1));
    }
}
