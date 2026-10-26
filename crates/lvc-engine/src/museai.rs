//! Dịch vụ [`lvc_museai::MuseaiVideoHost`] do engine cấp cho node `museai_video` — xem
//! `crates/providers/lvc-museai/CLAUDE.md` mục "Cơ chế tạo video" (3 giai đoạn: đăng nhập tay →
//! bootstrap qua trình duyệt thật → sinh video qua API).
//!
//! KHÔNG generic theo `BrowserDriver` (giống `lvc_browser::veo3::harvest::harvest_session`) — cần
//! `eval_json`/`Page` CDP trực tiếp để `fetch()` 2 API bootstrap TRONG CHÍNH trang đã đăng nhập,
//! trait `BrowserDriver` không lộ việc này. Hệ quả CHẤP NHẬN ĐƯỢC (cùng giới hạn với Veo3 harvest):
//! luôn mở trình duyệt THẬT kể cả dưới `--features fake-driver` — chưa fake được.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use dashmap::DashMap;
use lvc_browser::chromium::ChromiumDriver;
use lvc_browser::{BrowserDriver, LaunchOpts, ops};
use lvc_museai::protocol::auth::{MuseBootstrap, parse_bootstrap_html};
use lvc_museai::protocol::client::{
    GenerateImageParams, GenerateVideoParams, generate_image, generate_video,
};
use lvc_museai::protocol::errors::MuseError;
use lvc_museai::protocol::transport::HatchConnection;
use lvc_museai::{
    ImageJob, ImageJobDone, ImageJobsReport, MuseaiImageHost, MuseaiVideoHost, VideoJob,
    VideoJobDone, VideoJobsReport,
};
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::museai_sched::{Lease, Outcome};

/// Phần A: khoá bootstrap per-`profile_dir` (một account = một profile Chromium). Khi chạy nhiều
/// node `MuseAI` song song (cùng account), `fetch_bootstrap` (mở Chromium tại `profile_dir`) PHẢI
/// nối tiếp — 2 Chromium cùng `profile_dir` sẽ kẹt lock. Engine giữ map này, chia cho cả 2 host
/// (image+video) để lease chung. Chỉ giữ khoá quanh `fetch_bootstrap`, thả trước WebSocket để
/// generation (~phút) chạy chồng.
pub(crate) type BootstrapLocks = Arc<DashMap<String, Arc<Mutex<()>>>>;

/// Lấy (hoặc tạo) mutex cho một `profile_dir`. Cùng `profile_dir` → CÙNG mutex (nối tiếp bootstrap).
pub(crate) fn bootstrap_lock(
    locks: &DashMap<String, Arc<Mutex<()>>>,
    profile_dir: &str,
) -> Arc<Mutex<()>> {
    locks
        .entry(profile_dir.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Bộ nhớ đệm phiên đăng nhập (bootstrap) per-`profile_dir`. Trước đây MỖI node `MuseAI` mở một cửa
/// sổ Chromium thật (tối đa 90s) chỉ để đọc phiên rồi đóng — đồ thị ~25 node là hàng chục lần mở trình
/// duyệt, và cùng một profile chỉ mở được một lần một lúc. Giờ phiên lấy được dùng lại trong
/// [`BOOTSTRAP_TTL`]; phiên cũ/hỏng bị bỏ và lấy lại đúng một lần (xem [`open_connection`]).
///
/// Chứa admission token: CHỈ trong bộ nhớ, KHÔNG BAO GIỜ log/ghi đĩa.
pub(crate) type BootstrapCache = Arc<DashMap<String, (Instant, MuseBootstrap)>>;

/// Phiên đã lấy còn được tin trong bao lâu. Ngắn để token hết hạn hiếm khi bị dùng; nếu vẫn hết hạn
/// thì kết nối lỗi xác thực → bỏ khỏi đệm và lấy lại.
const BOOTSTRAP_TTL: Duration = Duration::from_secs(300);

/// Tối đa số tài khoản thử cho MỘT node khi tài khoản đang dùng không mở được phiên/kết nối.
const MAX_ACCOUNT_ATTEMPTS: u32 = 3;

/// Bootstrap còn hạn trong đệm, nếu có. THUẦN theo `now`/`ttl` tiêm vào để test.
fn cached_bootstrap(
    cache: &DashMap<String, (Instant, MuseBootstrap)>,
    key: &str,
    now: Instant,
    ttl: Duration,
) -> Option<MuseBootstrap> {
    let entry = cache.get(key)?;
    let (at, b) = entry.value();
    (now.saturating_duration_since(*at) < ttl).then(|| b.clone())
}

/// Mở kết nối Hatch cho MỘT `profile_dir`: dùng phiên trong đệm nếu còn hạn, không thì mở Chromium
/// lấy mới (nối tiếp theo `profile_dir` qua `locks`) rồi lưu đệm. Phiên lấy từ ĐỆM mà kết nối bị từ
/// chối vì xác thực (không phải lỗi tạm) → bỏ đệm, lấy lại ĐÚNG MỘT lần.
async fn open_connection(
    profile_dir: &Path,
    logs_dir: &Path,
    locks: &BootstrapLocks,
    cache: &BootstrapCache,
    cancel: &CancellationToken,
) -> Result<HatchConnection, String> {
    let key = profile_dir.to_string_lossy().into_owned();
    for attempt in 0..2 {
        // Khoá CHỈ quanh bước lấy phiên (2 Chromium cùng profile sẽ kẹt lock); kiểm đệm LẠI sau khi
        // có khoá vì người đứng trước có thể vừa lấy xong — tránh xếp hàng mở Chromium thừa.
        let (bootstrap, from_cache) = {
            let lock = bootstrap_lock(locks, &key);
            let _guard = lock.lock().await;
            if let Some(b) = cached_bootstrap(cache, &key, Instant::now(), BOOTSTRAP_TTL) {
                (b, true)
            } else {
                let b = fetch_bootstrap(profile_dir, logs_dir)
                    .await
                    .map_err(|e| format!("khong lay duoc phien dang nhap muse.ai: {e}"))?;
                cache.insert(key.clone(), (Instant::now(), b.clone()));
                (b, false)
            }
        };
        match connect_with_retry(&bootstrap.websocket_url(), &bootstrap.locale, cancel).await {
            Ok(c) => return Ok(c),
            Err(e) => {
                cache.remove(&key);
                if attempt == 0 && from_cache && !e.is_transient() && !cancel.is_cancelled() {
                    info!("museai: phien trong dem bi tu choi, lay phien moi");
                    continue;
                }
                return Err(format!("ket noi Hatch that bai: {e}"));
            }
        }
    }
    Err("ket noi Hatch that bai (het luot thu)".to_string())
}

/// Mượn một tài khoản (rảnh nhất) rồi mở kết nối; tài khoản không mở được phiên/kết nối thì báo
/// [`Outcome::AccountFailed`] (bị cách ly) và THỬ TÀI KHOẢN KHÁC, tối đa [`MAX_ACCOUNT_ATTEMPTS`].
/// Lease không quản lý (resolver mặc định, khoá rỗng) không có "tài khoản khác" nên không thử lại —
/// tránh mở Chromium 90s lặp lại vô ích trên cùng profile.
///
/// Chỉ chuyển dự phòng ở bước MỞ KẾT NỐI (chưa sinh gì) nên không bao giờ sinh/tính phí hai lần.
async fn acquire_connection(
    resolver: &dyn MuseaiAccountResolver,
    credential_id: &str,
    logs_dir: &Path,
    locks: &BootstrapLocks,
    cache: &BootstrapCache,
    cancel: &CancellationToken,
) -> Result<(Lease, HatchConnection), String> {
    let mut avoid: Vec<String> = Vec::new();
    let mut last_err = String::new();
    for attempt in 1..=MAX_ACCOUNT_ATTEMPTS {
        if cancel.is_cancelled() {
            return Err("da huy".to_string());
        }
        let lease = match resolver.lease(credential_id, &avoid, cancel).await {
            Ok(l) => l,
            Err(e) if last_err.is_empty() => return Err(e),
            Err(e) => return Err(format!("{last_err}; {e}")),
        };
        match open_connection(lease.profile_dir(), logs_dir, locks, cache, cancel).await {
            Ok(conn) => return Ok((lease, conn)),
            Err(e) => {
                lease.report(Outcome::AccountFailed);
                warn!(
                    attempt,
                    account = lease.key(),
                    "museai: tai khoan khong mo duoc phien, thu tai khoan khac neu con"
                );
                if lease.key().is_empty() {
                    return Err(e);
                }
                avoid.push(lease.key().to_string());
                last_err = e;
            }
        }
    }
    Err(last_err)
}

/// Báo kết quả node cho bộ chia tải: có job xong → tài khoản tốt; toàn thất bại → cách ly NGẮN để
/// lần chọn sau ưu tiên tài khoản khác (đa phần là 502/quá hạn chờ ảnh — lỗi tạm thời của muse.ai).
fn settle_lease(lease: &Lease, any_done: bool, any_failed: bool) {
    if any_done {
        lease.report(Outcome::Ok);
    } else if any_failed {
        lease.report(Outcome::Transient);
    }
}

/// Nguồn `profile_dir` cho MỘT `credential_id` (`museai/login` trong `lvc-vault`) — dùng khuôn
/// `crate::veo3::VeoAccountResolver`, tách riêng (không tái dùng trait đó) vì tên gắn với Veo3,
/// dễ gây nhầm khi đọc code `MuseAI`.
#[async_trait]
pub trait MuseaiAccountResolver: Send + Sync {
    /// # Errors
    /// Chuỗi mô tả AN TOÀN (không chứa bí mật) khi không tìm/không đọc được `profile_dir`.
    async fn profile_dir(&self, credential_id: &str) -> Result<PathBuf, String>;

    /// Mượn một tài khoản cho MỘT node: chọn tài khoản rảnh nhất, chờ nếu tất cả đang bận, bỏ các
    /// khoá trong `avoid` (tài khoản vừa thất bại ở lượt trước). Thả `Lease` (drop) là trả chỗ.
    ///
    /// Mặc định: dùng [`Self::profile_dir`], KHÔNG chia tải, KHÔNG xoay (hành vi cũ — CLI/test).
    /// Resolver thật (`lvc-app`) ghi đè để chia tải theo [`crate::AccountScheduler`].
    ///
    /// # Errors
    /// Chuỗi mô tả AN TOÀN khi không có tài khoản dùng được / chờ quá lâu / đã huỷ.
    async fn lease(
        &self,
        credential_id: &str,
        _avoid: &[String],
        _cancel: &CancellationToken,
    ) -> Result<Lease, String> {
        Ok(Lease::unmanaged(self.profile_dir(credential_id).await?))
    }
}

/// Resolver mặc định: LUÔN lỗi (dùng khi chưa cắm kho tài khoản — CLI/test) — dùng ý
/// `crate::veo3::NoAccountResolver`.
pub(crate) struct NoAccountResolver;

#[async_trait]
impl MuseaiAccountResolver for NoAccountResolver {
    async fn profile_dir(&self, _credential_id: &str) -> Result<PathBuf, String> {
        Err("chua cau hinh kho tai khoan (khong resolve duoc profile_dir cho MuseAI)".to_string())
    }
}

/// Tên file lưu (trong `downloads_dir`) cho MỘT video đã tạo xong — mirror quy ước
/// `<run_id>_<node_id>_<scene_index>` dùng chung cho Dola/Veo3 (xem `crate::dag`/`crate::veo3`).
fn video_out_path(downloads_dir: &Path, run_id: &str, node_id: &str, scene_index: u32) -> PathBuf {
    downloads_dir.join(format!("{run_id}_{node_id}_{scene_index}.mp4"))
}

/// Như [`video_out_path`] nhưng cho ẢNH — `ext` lấy từ file đã tải THẬT (`.webp` quan sát được
/// trên trang thật, nhưng KHÔNG ép cứng — muse.ai có thể trả định dạng khác cho prompt khác).
fn image_out_path(
    downloads_dir: &Path,
    run_id: &str,
    node_id: &str,
    scene_index: u32,
    ext: &str,
) -> PathBuf {
    downloads_dir.join(format!("{run_id}_{node_id}_{scene_index}.{ext}"))
}

/// Quá hạn chờ bắt tay Noise/mở WebSocket — KHÔNG phải quá hạn chờ video (đó là tham số riêng của
/// `generate_video`, lâu hơn nhiều vì phải đợi AI sinh xong).
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

/// Quá hạn cho CẢ bước lấy phiên đăng nhập (mở Chromium → vào muse.ai → đọc bootstrap).
///
/// Trước đây bước này KHÔNG có hạn: Chromium không mở được (profile đang bị một cửa sổ Chrome khác
/// khoá, thiếu bản Chrome, cửa sổ mở ngoài màn hình) là node treo VĨNH VIỄN — người dùng chỉ thấy
/// "Đang chạy 0/1" mãi mãi, không có lỗi nào để đọc. 90 giây đủ rộng cho một lần mở trình duyệt
/// chậm, và đủ ngắn để biết là hỏng.
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Deserialize)]
struct TokenRefreshResponse {
    token: Option<String>,
    notary_token: Option<String>,
}

/// Mở trình duyệt THẬT tại `profile_dir` (đã đăng nhập tay từ trước — xem `node-museai-video/
/// CLAUDE.md` giai đoạn [1]), điều hướng `muse.ai`, đọc HTML trang + gọi 2 API bootstrap qua
/// `fetch()` TRONG CHÍNH trang (giai đoạn [2] — không giả mạo gì, trình duyệt thật). Đóng trình
/// duyệt ngay sau khi lấy xong (không giữ mở suốt lúc sinh video — giai đoạn [3] chỉ cần WebSocket
/// qua `lvc-museai`, không cần trình duyệt nữa).
///
/// ĐÃ XÁC NHẬN đúng field/endpoint qua trang thật 2026-10-11 (`crates/lvc-browser/examples/
/// museai_probe.rs`, cookie thật của người dùng) — xem `MEMORY.md`.
async fn fetch_bootstrap(profile_dir: &Path, logs_dir: &Path) -> Result<MuseBootstrap, String> {
    match tokio::time::timeout(
        BOOTSTRAP_TIMEOUT,
        fetch_bootstrap_inner(profile_dir, logs_dir),
    )
    .await
    {
        Ok(r) => r,
        Err(_) => Err(format!(
            "qua {}s van chua lay duoc phien dang nhap muse.ai — thuong la Chromium khong mo duoc              (profile dang bi mot cua so Chrome khac khoa, hoac chua dang nhap muse.ai trong              profile nay). Dong het cua so Chrome dang dung profile do roi thu lai.",
            BOOTSTRAP_TIMEOUT.as_secs()
        )),
    }
}

async fn fetch_bootstrap_inner(
    profile_dir: &Path,
    logs_dir: &Path,
) -> Result<MuseBootstrap, String> {
    let driver = ChromiumDriver;
    let opts = LaunchOpts {
        profile_dir: profile_dir.to_path_buf(),
        logs_dir: logs_dir.to_path_buf(),
        // KHONG headless — dung ly do voi harvest_session Veo3 (UA "HeadlessChrome" la dau bot;
        // phien nay la phien THAT da dang nhap, khong muon trong giong tu dong hoa).
        headless: false,
        ..Default::default()
    };
    // Log TUNG BUOC: khi node treo, day la thu duy nhat cho biet no dung o dau. KHONG log cookie/
    // URL co chu ky — chi ten buoc (xem luat #3 trong CLAUDE.md).
    info!("museai: mo trinh duyet lay phien dang nhap");
    let session = driver.launch(&opts).await.map_err(|e| e.to_string())?;
    info!("museai: da mo trinh duyet, dang doc bootstrap tu muse.ai");
    let result = fetch_bootstrap_with_session(&driver, &session).await;
    let _ = driver.close(session).await;
    match &result {
        Ok(_) => info!("museai: lay phien dang nhap xong"),
        Err(e) => warn!(error = %e, "museai: lay phien dang nhap that bai"),
    }
    result
}

/// PROBE đăng nhập muse.ai cho nút "Kiểm tra" tài khoản cookie: mở profile → vào muse.ai → có
/// `gatewayUrl` trong trang = ĐÃ đăng nhập. KHÔNG gọi `hatch/vm/wake` + `hatch/token` (nhẹ hơn
/// [`fetch_bootstrap`], KHÔNG tốn credit, không đánh thức VM). Đóng trình duyệt ngay sau khi đọc.
///
/// - `Ok(true)`  = đã đăng nhập (parse bootstrap OK).
/// - `Ok(false)` = chưa đăng nhập / phiên hết hạn (trang login — thiếu `gatewayUrl`/`authToken`).
/// - `Err(..)`   = lỗi HẠ TẦNG (Chromium không mở được / profile bị khoá / quá `BOOTSTRAP_TIMEOUT`).
///
/// # Errors
/// Chuỗi mô tả an toàn (không chứa bí mật) khi mở trình duyệt/điều hướng thất bại hoặc quá hạn.
pub async fn probe_login(profile_dir: &Path, logs_dir: &Path) -> Result<bool, String> {
    match tokio::time::timeout(BOOTSTRAP_TIMEOUT, probe_login_inner(profile_dir, logs_dir)).await {
        Ok(r) => r,
        Err(_) => Err(format!(
            "qua {}s chua mo duoc muse.ai (profile dang bi cua so Chrome khac khoa?). Dong het cua \
             so Chrome dang dung profile do roi thu lai.",
            BOOTSTRAP_TIMEOUT.as_secs()
        )),
    }
}

async fn probe_login_inner(profile_dir: &Path, logs_dir: &Path) -> Result<bool, String> {
    let driver = ChromiumDriver;
    let opts = LaunchOpts {
        profile_dir: profile_dir.to_path_buf(),
        logs_dir: logs_dir.to_path_buf(),
        headless: false, // phien THAT da dang nhap — khong muon UA "HeadlessChrome" (dau bot).
        ..Default::default()
    };
    info!("museai: mo trinh duyet KIEM TRA dang nhap");
    let session = driver.launch(&opts).await.map_err(|e| e.to_string())?;
    let html = read_page_html(&driver, &session).await;
    let _ = driver.close(session).await;
    // Lỗi `?` ở đây là lỗi HẠ TẦNG (goto/eval). `parse_bootstrap_html` thất bại KHÔNG phải lỗi hạ
    // tầng — chỉ nghĩa là chưa đăng nhập (`Ok(false)`).
    let logged_in = parse_bootstrap_html(&html?).is_ok();
    if logged_in {
        info!("museai: KIEM TRA — da dang nhap");
    } else {
        info!("museai: KIEM TRA — chua dang nhap (trang login)");
    }
    Ok(logged_in)
}

impl<D: BrowserDriver + 'static> crate::generate::Engine<D> {
    /// Kiểm tra một profile `MuseAI` đã đăng nhập chưa — vỏ mỏng quanh [`probe_login`] để lệnh Tauri
    /// `credential_verify` không cần biết `logs_dir`. KHÔNG generic theo `D` thực sự (probe luôn mở
    /// `ChromiumDriver` thật, giống [`fetch_bootstrap`]).
    ///
    /// # Errors
    /// Xem [`probe_login`].
    pub async fn probe_museai_login(&self, profile_dir: &Path) -> Result<bool, String> {
        probe_login(profile_dir, &self.cfg.logs_dir).await
    }
}

/// Mở `muse.ai` trên phiên đã có + đọc toàn bộ HTML trang. Tách riêng để DÙNG CHUNG giữa lấy phiên
/// đầy đủ ([`fetch_bootstrap_with_session`]) và PROBE đăng nhập ([`probe_login`]). Lỗi ở đây là lỗi
/// HẠ TẦNG (goto/eval) — khác với `parse_bootstrap_html` thất bại (trang login, chưa đăng nhập).
async fn read_page_html(
    driver: &ChromiumDriver,
    session: &<ChromiumDriver as BrowserDriver>::Session,
) -> Result<String, String> {
    driver
        .goto(session, "https://muse.ai/")
        .await
        .map_err(|e| e.to_string())?;
    let page = session.page();
    ops::eval_json(page, "document.documentElement.outerHTML")
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_bootstrap_with_session(
    driver: &ChromiumDriver,
    session: &<ChromiumDriver as BrowserDriver>::Session,
) -> Result<MuseBootstrap, String> {
    let html = read_page_html(driver, session).await?;
    let bootstrap = parse_bootstrap_html(&html)?;
    let page = session.page();
    let vm_id = bootstrap.vm_id();

    let wake_js = format!(
        "fetch('/api/hatch/vm/wake',{{method:'POST',headers:{{'Accept':'*/*','Content-Type':\
         'application/json','Origin':location.origin,'Referer':location.href}},credentials:\
         'same-origin',body:JSON.stringify({{vm_id:{vm_id:?},retry_count:0,connect_attempt_id:\
         crypto.randomUUID()}})}}).then(r=>r.json())"
    );
    let _wake: serde_json::Value = ops::eval_json(page, &wake_js)
        .await
        .map_err(|e| format!("hatch/vm/wake: {e}"))?;

    let token_js = format!(
        "fetch('/api/hatch/token',{{method:'POST',headers:{{'Accept':'*/*','Content-Type':\
         'application/json','Origin':location.origin,'Referer':location.href}},credentials:\
         'same-origin',body:JSON.stringify({{vmAddress:{gw:?},vmName:{vm_id:?}}})}})\
         .then(r=>r.json())",
        gw = bootstrap.gateway_url,
    );
    let refreshed: TokenRefreshResponse = ops::eval_json(page, &token_js)
        .await
        .map_err(|e| format!("hatch/token: {e}"))?;

    let admission = refreshed
        .token
        .filter(|s| !s.is_empty())
        .unwrap_or(bootstrap.admission);
    if admission.is_empty() {
        return Err("thieu admission token sau khi lam moi qua hatch/token".to_string());
    }
    let notary = refreshed.notary_token.or(bootstrap.notary);
    Ok(MuseBootstrap {
        admission,
        notary,
        ..bootstrap
    })
}

/// Phần fix 502: kết nối Hatch có TỰ THỬ LẠI khi lỗi TẠM THỜI (502/5xx/429) — mirror cách Veo3
/// retry 5xx (`api.py`). Backoff 3s → ×2, trần 30s, tối đa 4 lượt; huỷ thì dừng ngay. Lỗi CỐ ĐỊNH
/// (vd prompt sai) thì fail luôn, không thử lại vô ích.
async fn connect_with_retry(
    ws_url: &str,
    locale: &str,
    cancel: &CancellationToken,
) -> Result<HatchConnection, MuseError> {
    const MAX_ATTEMPTS: u32 = 4;
    let mut delay = Duration::from_secs(3);
    let mut last_err: Option<MuseError> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        if cancel.is_cancelled() {
            break;
        }
        match HatchConnection::connect(ws_url, locale, HANDSHAKE_TIMEOUT).await {
            Ok(c) => return Ok(c),
            Err(e) => {
                let transient = e.is_transient();
                warn!(
                    attempt,
                    transient, "museai: ket noi Hatch that bai, se thu lai neu tam thoi"
                );
                last_err = Some(e);
                if !transient || attempt == MAX_ATTEMPTS {
                    break;
                }
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => break,
                    () = tokio::time::sleep(delay) => {}
                }
                delay = (delay * 2).min(Duration::from_secs(30));
            }
        }
    }
    Err(last_err
        .unwrap_or_else(|| MuseError::Protocol("ket noi Hatch that bai (het luot thu)".into())))
}

/// Bề mặt NODE thực `MuseaiVideoHost` cho MỘT lần chạy — mở trình duyệt lấy bootstrap + kết nối
/// Hatch MỘT LẦN cho cả lô `jobs` (giả định mọi job trong CÙNG một lần gọi `generate_videos` dùng
/// CHUNG một `credential_id` — node chỉ có một tham số `account` tĩnh, giống `veo3_video`).
pub(crate) struct EngineMuseaiJobs {
    downloads_dir: PathBuf,
    logs_dir: PathBuf,
    resolver: Arc<dyn MuseaiAccountResolver>,
    /// Token CUA RUN (fix: nut "Huy") — `MuseAI` sinh video co the lau nhieu phut/canh nen phai
    /// cat ngang duoc; giong `EngineVeoJobs`/`EngineGoogleVideo`/`EngineSeedanceVideo`.
    cancel: CancellationToken,
    /// Phần A: lease bootstrap per-`profile_dir` (xem [`BootstrapLocks`]).
    bootstrap_locks: BootstrapLocks,
    /// Đệm phiên đăng nhập per-`profile_dir` (xem [`BootstrapCache`]).
    bootstrap_cache: BootstrapCache,
}

impl EngineMuseaiJobs {
    pub(crate) fn new(
        downloads_dir: PathBuf,
        logs_dir: PathBuf,
        resolver: Arc<dyn MuseaiAccountResolver>,
        cancel: CancellationToken,
        bootstrap_locks: BootstrapLocks,
        bootstrap_cache: BootstrapCache,
    ) -> Self {
        Self {
            downloads_dir,
            logs_dir,
            resolver,
            cancel,
            bootstrap_locks,
            bootstrap_cache,
        }
    }

    fn fail_all(jobs: &[VideoJob], reason: &str) -> VideoJobsReport {
        VideoJobsReport {
            done: Vec::new(),
            failed: jobs
                .iter()
                .map(|j| format!("canh {}: {reason}", j.scene_index))
                .collect(),
        }
    }
}

#[async_trait]
impl MuseaiVideoHost for EngineMuseaiJobs {
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport {
        let Some(first) = jobs.first() else {
            return VideoJobsReport::default();
        };
        if self.cancel.is_cancelled() {
            return Self::fail_all(&jobs, "da huy");
        }
        // Mượn MỘT tài khoản (rảnh nhất, chia tải giữa các node song song) + mở kết nối Hatch; tài
        // khoản không mở được phiên thì tự đổi sang tài khoản khác. `lease` giữ chỗ tới hết hàm —
        // drop là trả chỗ cho node đang chờ.
        let (lease, conn) = match acquire_connection(
            self.resolver.as_ref(),
            &first.credential_id,
            &self.logs_dir,
            &self.bootstrap_locks,
            &self.bootstrap_cache,
            &self.cancel,
        )
        .await
        {
            Ok(x) => x,
            Err(e) => return Self::fail_all(&jobs, &e),
        };

        let mut report = VideoJobsReport::default();
        for job in jobs {
            // Nut "Huy": bo cac canh con lai NGAY, va cat ngang canh dang sinh (WebSocket RPC,
            // an toan khi drop) thay vi doi het timeout 600s.
            if self.cancel.is_cancelled() {
                report
                    .failed
                    .push(format!("canh {}: da huy", job.scene_index));
                continue;
            }
            // MỖI cảnh một SESSION (chat) RIÊNG — UUID mới, mirror bản tham chiếu
            // (`service.py`: `session_id or str(uuid.uuid4())`). Tránh dồn cả chục cảnh vào MỘT
            // chat chính của muse.ai (nặng → chậm/502/ảnh đính kèm không lên).
            let session_id = uuid::Uuid::new_v4().to_string();
            let params = GenerateVideoParams {
                prompt: &job.prompt,
                device_id: node_id,
                output_dir: &self.downloads_dir,
                session_id: Some(&session_id),
                // Giờ cao điểm MuseAI sinh chậm → nới trần 15 phút/video (trước 10p) để video chậm
                // vẫn kịp thay vì timeout oan.
                timeout: Duration::from_secs(900),
                min_videos: 1,
                aspect: job.aspect.as_deref(),
                duration_secs: job.duration_secs,
                images: &job.images,
            };
            let outcome = tokio::select! {
                biased;
                () = self.cancel.cancelled() => {
                    report
                        .failed
                        .push(format!("canh {}: da huy", job.scene_index));
                    continue;
                }
                r = generate_video(&conn, &params) => r,
            };
            match outcome {
                Ok(result) if !result.downloaded.is_empty() => {
                    let target =
                        video_out_path(&self.downloads_dir, run_id, node_id, job.scene_index);
                    if let Err(e) = std::fs::rename(&result.downloaded[0], &target) {
                        report.failed.push(format!(
                            "canh {}: doi ten file that bai: {e}",
                            job.scene_index
                        ));
                        continue;
                    }
                    report.done.push(VideoJobDone {
                        scene_index: job.scene_index,
                        local_path: target,
                    });
                }
                Ok(result) => {
                    let detail = if result.download_errors.is_empty() {
                        "khong co video nao duoc tao".to_string()
                    } else {
                        result.download_errors.join("; ")
                    };
                    report
                        .failed
                        .push(format!("canh {}: {detail}", job.scene_index));
                }
                Err(e) => report.failed.push(format!("canh {}: {e}", job.scene_index)),
            }
        }
        conn.close().await;
        settle_lease(&lease, !report.done.is_empty(), !report.failed.is_empty());
        report
    }
}

/// Bề mặt NODE thực `MuseaiImageHost` cho MỘT lần chạy — khớp khuôn [`EngineMuseaiJobs`], chỉ
/// khác bước sinh (`generate_image` thay vì `generate_video`) + tên file giữ nguyên đuôi thật
/// (ảnh không cố định `.mp4` như video).
pub(crate) struct EngineMuseaiImageJobs {
    downloads_dir: PathBuf,
    logs_dir: PathBuf,
    resolver: Arc<dyn MuseaiAccountResolver>,
    /// Token CUA RUN (fix nut "Huy") — xem `EngineMuseaiJobs`.
    cancel: CancellationToken,
    /// Phần A: lease bootstrap per-`profile_dir` (xem [`BootstrapLocks`]).
    bootstrap_locks: BootstrapLocks,
    /// Đệm phiên đăng nhập per-`profile_dir` (xem [`BootstrapCache`]).
    bootstrap_cache: BootstrapCache,
}

impl EngineMuseaiImageJobs {
    pub(crate) fn new(
        downloads_dir: PathBuf,
        logs_dir: PathBuf,
        resolver: Arc<dyn MuseaiAccountResolver>,
        cancel: CancellationToken,
        bootstrap_locks: BootstrapLocks,
        bootstrap_cache: BootstrapCache,
    ) -> Self {
        Self {
            downloads_dir,
            logs_dir,
            resolver,
            cancel,
            bootstrap_locks,
            bootstrap_cache,
        }
    }

    fn fail_all(jobs: &[ImageJob], reason: &str) -> ImageJobsReport {
        ImageJobsReport {
            done: Vec::new(),
            failed: jobs
                .iter()
                .map(|j| format!("canh {}: {reason}", j.scene_index))
                .collect(),
        }
    }
}

#[async_trait]
impl MuseaiImageHost for EngineMuseaiImageJobs {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport {
        let Some(first) = jobs.first() else {
            return ImageJobsReport::default();
        };
        if self.cancel.is_cancelled() {
            return Self::fail_all(&jobs, "da huy");
        }
        // Mượn MỘT tài khoản (rảnh nhất, chia tải giữa các node song song) + mở kết nối Hatch; tài
        // khoản không mở được phiên thì tự đổi sang tài khoản khác. `lease` giữ chỗ tới hết hàm —
        // drop là trả chỗ cho node đang chờ.
        let (lease, conn) = match acquire_connection(
            self.resolver.as_ref(),
            &first.credential_id,
            &self.logs_dir,
            &self.bootstrap_locks,
            &self.bootstrap_cache,
            &self.cancel,
        )
        .await
        {
            Ok(x) => x,
            Err(e) => return Self::fail_all(&jobs, &e),
        };

        let mut report = ImageJobsReport::default();
        for job in jobs {
            // Nut "Huy": bo canh con lai + cat ngang canh dang sinh (xem `generate_videos`).
            if self.cancel.is_cancelled() {
                report
                    .failed
                    .push(format!("canh {}: da huy", job.scene_index));
                continue;
            }
            // MỖI cảnh một SESSION (chat) RIÊNG — xem ghi chú ở `generate_videos`.
            let session_id = uuid::Uuid::new_v4().to_string();
            let params = GenerateImageParams {
                prompt: &job.prompt,
                device_id: node_id,
                output_dir: &self.downloads_dir,
                session_id: Some(&session_id),
                // Giờ cao điểm: ảnh MuseAI đang mất ~5 phút (310s suýt timeout) → nới trần 10 phút
                // (trước 5p) để ảnh chậm vẫn kịp.
                timeout: Duration::from_secs(600),
                min_images: 1,
                images: &job.images,
            };
            let outcome = tokio::select! {
                biased;
                () = self.cancel.cancelled() => {
                    report
                        .failed
                        .push(format!("canh {}: da huy", job.scene_index));
                    continue;
                }
                r = generate_image(&conn, &params) => r,
            };
            match outcome {
                Ok(result) if !result.downloaded.is_empty() => {
                    let ext = result.downloaded[0]
                        .extension()
                        .map_or_else(|| "webp".to_string(), |e| e.to_string_lossy().into_owned());
                    let target =
                        image_out_path(&self.downloads_dir, run_id, node_id, job.scene_index, &ext);
                    if let Err(e) = std::fs::rename(&result.downloaded[0], &target) {
                        report.failed.push(format!(
                            "canh {}: doi ten file that bai: {e}",
                            job.scene_index
                        ));
                        continue;
                    }
                    report.done.push(ImageJobDone {
                        scene_index: job.scene_index,
                        local_path: target,
                    });
                }
                Ok(result) => {
                    let detail = if result.download_errors.is_empty() {
                        "khong co anh nao duoc tao".to_string()
                    } else {
                        result.download_errors.join("; ")
                    };
                    report
                        .failed
                        .push(format!("canh {}: {detail}", job.scene_index));
                }
                Err(e) => report.failed.push(format!("canh {}: {e}", job.scene_index)),
            }
        }
        conn.close().await;
        settle_lease(&lease, !report.done.is_empty(), !report.failed.is_empty());
        report
    }
}

#[cfg(test)]
mod tests {
    use super::{BOOTSTRAP_TTL, bootstrap_lock, cached_bootstrap};
    use dashmap::DashMap;
    use lvc_museai::protocol::auth::MuseBootstrap;
    use std::time::{Duration, Instant};

    fn boot(admission: &str) -> MuseBootstrap {
        MuseBootstrap {
            gateway_url: "https://vm1.example".into(),
            vm_name: "vm1".into(),
            admission: admission.into(),
            notary: None,
            locale: "vi".into(),
        }
    }

    #[test]
    fn dem_phien_con_han_tra_ve_ban_sao() {
        let cache = DashMap::new();
        let t0 = Instant::now();
        cache.insert("p".to_string(), (t0, boot("tok")));
        let got = cached_bootstrap(&cache, "p", t0 + Duration::from_secs(10), BOOTSTRAP_TTL);
        assert_eq!(got, Some(boot("tok")));
    }

    #[test]
    fn dem_phien_het_han_thi_khong_dung_lai() {
        let cache = DashMap::new();
        let t0 = Instant::now();
        cache.insert("p".to_string(), (t0, boot("tok")));
        let late = t0 + BOOTSTRAP_TTL + Duration::from_secs(1);
        assert_eq!(cached_bootstrap(&cache, "p", late, BOOTSTRAP_TTL), None);
    }

    #[test]
    fn dem_khac_profile_khong_lan_nhau() {
        let cache = DashMap::new();
        let t0 = Instant::now();
        cache.insert("p1".to_string(), (t0, boot("tok1")));
        assert_eq!(cached_bootstrap(&cache, "p2", t0, BOOTSTRAP_TTL), None);
    }

    /// Phần A: cùng `profile_dir` trả CÙNG mutex (nối tiếp bootstrap); khác → mutex khác.
    #[test]
    fn bootstrap_lock_cung_key_cung_arc() {
        let locks = DashMap::new();
        let a = bootstrap_lock(&locks, "C:/p/acc1");
        let b = bootstrap_lock(&locks, "C:/p/acc1");
        let c = bootstrap_lock(&locks, "C:/p/acc2");
        assert!(
            std::sync::Arc::ptr_eq(&a, &b),
            "cung profile_dir -> cung mutex"
        );
        assert!(
            !std::sync::Arc::ptr_eq(&a, &c),
            "khac profile_dir -> khac mutex"
        );
    }
}
