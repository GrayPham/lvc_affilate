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
use std::time::Duration;

use async_trait::async_trait;
use lvc_browser::chromium::ChromiumDriver;
use lvc_browser::{BrowserDriver, LaunchOpts, ops};
use lvc_museai::protocol::auth::{MuseBootstrap, parse_bootstrap_html};
use lvc_museai::protocol::client::{
    GenerateImageParams, GenerateVideoParams, generate_image, generate_video,
};
use lvc_museai::protocol::transport::HatchConnection;
use lvc_museai::{
    ImageJob, ImageJobDone, ImageJobsReport, MuseaiImageHost, MuseaiVideoHost, VideoJob,
    VideoJobDone, VideoJobsReport,
};
use serde::Deserialize;
use tracing::{info, warn};

/// Nguồn `profile_dir` cho MỘT `credential_id` (`museai/login` trong `lvc-vault`) — dùng khuôn
/// `crate::veo3::VeoAccountResolver`, tách riêng (không tái dùng trait đó) vì tên gắn với Veo3,
/// dễ gây nhầm khi đọc code `MuseAI`.
#[async_trait]
pub trait MuseaiAccountResolver: Send + Sync {
    /// # Errors
    /// Chuỗi mô tả AN TOÀN (không chứa bí mật) khi không tìm/không đọc được `profile_dir`.
    async fn profile_dir(&self, credential_id: &str) -> Result<PathBuf, String>;
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

async fn fetch_bootstrap_with_session(
    driver: &ChromiumDriver,
    session: &<ChromiumDriver as BrowserDriver>::Session,
) -> Result<MuseBootstrap, String> {
    driver
        .goto(session, "https://muse.ai/")
        .await
        .map_err(|e| e.to_string())?;
    let page = session.page();
    let html: String = ops::eval_json(page, "document.documentElement.outerHTML")
        .await
        .map_err(|e| e.to_string())?;
    let bootstrap = parse_bootstrap_html(&html)?;
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

/// Bề mặt NODE thực `MuseaiVideoHost` cho MỘT lần chạy — mở trình duyệt lấy bootstrap + kết nối
/// Hatch MỘT LẦN cho cả lô `jobs` (giả định mọi job trong CÙNG một lần gọi `generate_videos` dùng
/// CHUNG một `credential_id` — node chỉ có một tham số `account` tĩnh, giống `veo3_video`).
pub(crate) struct EngineMuseaiJobs {
    downloads_dir: PathBuf,
    logs_dir: PathBuf,
    resolver: Arc<dyn MuseaiAccountResolver>,
}

impl EngineMuseaiJobs {
    pub(crate) fn new(
        downloads_dir: PathBuf,
        logs_dir: PathBuf,
        resolver: Arc<dyn MuseaiAccountResolver>,
    ) -> Self {
        Self {
            downloads_dir,
            logs_dir,
            resolver,
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
        let profile_dir = match self.resolver.profile_dir(&first.credential_id).await {
            Ok(dir) => dir,
            Err(e) => return Self::fail_all(&jobs, &e),
        };
        let bootstrap = match fetch_bootstrap(&profile_dir, &self.logs_dir).await {
            Ok(b) => b,
            Err(e) => {
                return Self::fail_all(
                    &jobs,
                    &format!("khong lay duoc phien dang nhap muse.ai: {e}"),
                );
            }
        };
        let ws_url = bootstrap.websocket_url();
        let conn =
            match HatchConnection::connect(&ws_url, &bootstrap.locale, HANDSHAKE_TIMEOUT).await {
                Ok(c) => c,
                Err(e) => return Self::fail_all(&jobs, &format!("ket noi Hatch that bai: {e}")),
            };

        let mut report = VideoJobsReport::default();
        for job in jobs {
            let params = GenerateVideoParams {
                prompt: &job.prompt,
                device_id: node_id,
                output_dir: &self.downloads_dir,
                session_id: None,
                timeout: Duration::from_secs(600),
                min_videos: 1,
                aspect: job.aspect.as_deref(),
                duration_secs: job.duration_secs,
            };
            match generate_video(&conn, &params).await {
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
}

impl EngineMuseaiImageJobs {
    pub(crate) fn new(
        downloads_dir: PathBuf,
        logs_dir: PathBuf,
        resolver: Arc<dyn MuseaiAccountResolver>,
    ) -> Self {
        Self {
            downloads_dir,
            logs_dir,
            resolver,
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
        let profile_dir = match self.resolver.profile_dir(&first.credential_id).await {
            Ok(dir) => dir,
            Err(e) => return Self::fail_all(&jobs, &e),
        };
        let bootstrap = match fetch_bootstrap(&profile_dir, &self.logs_dir).await {
            Ok(b) => b,
            Err(e) => {
                return Self::fail_all(
                    &jobs,
                    &format!("khong lay duoc phien dang nhap muse.ai: {e}"),
                );
            }
        };
        let ws_url = bootstrap.websocket_url();
        let conn =
            match HatchConnection::connect(&ws_url, &bootstrap.locale, HANDSHAKE_TIMEOUT).await {
                Ok(c) => c,
                Err(e) => return Self::fail_all(&jobs, &format!("ket noi Hatch that bai: {e}")),
            };

        let mut report = ImageJobsReport::default();
        for job in jobs {
            let params = GenerateImageParams {
                prompt: &job.prompt,
                device_id: node_id,
                output_dir: &self.downloads_dir,
                session_id: None,
                timeout: Duration::from_secs(300),
                min_images: 1,
            };
            match generate_image(&conn, &params).await {
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
        report
    }
}
