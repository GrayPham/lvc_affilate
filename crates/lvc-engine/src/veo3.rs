//! P5 Task 9: dịch vụ [`lvc_veo3::VeoCaptchaHost`] do engine cấp — bọc
//! [`lvc_browser::veo3::captcha::GuestCaptchaBrowser`] trong `tokio::sync::Mutex` để nhiều lời gọi
//! `mint_token` đồng thời tự xếp hàng (xem `crates/lvc-browser/src/veo3/MEMORY.md`: `chromiumoxide`
//! là async thuần, KHÔNG cần tự chế "pool"/luồng thợ riêng như bản Python — Selenium không
//! thread-safe nên Python buộc phải làm vậy, `Mutex` là đủ ở đây).
//!
//! Trình duyệt khách được MỞ MỘT LẦN rồi DÙNG LẠI TUẦN TỰ cho nhiều lần mint (không phải mở mới mỗi
//! lần chạy workflow) — trạng thái sống trên [`crate::generate::Engine`] (dài hạn, xuyên suốt nhiều
//! `run_graph`), KHÔNG phải trên adapter chèn vào `Services` mỗi lần chạy (khác với
//! `RunDolaHost`/`EngineTextGenHost` vốn không cần giữ trạng thái xuyên-run kiểu này) — xem
//! `dag::RunVeoCaptchaHost`, chỉ là lớp mỏng chuyển tiếp cuộc gọi vào đây.
//!
//! **`root_dir`**: thư mục gốc chứa các profile Chrome KHÁCH tạm (`GuestCaptchaBrowser::open` tự
//! tạo thư mục con `guest_<uuid>` bên trong). `lvc-engine` KHÔNG tự đoán đường dẫn — không có quy
//! ước "thư mục dữ liệu app" chung ở tầng này (`EngineConfig` chỉ có `accounts_dir`/`downloads_dir`
//! riêng lẻ, không có trường "`app_data_dir`" bắt buộc — xem `settings.rs`). `lvc-app` (nơi biết
//! `app_data_dir` thật qua Tauri, `paths::app_data_dir`) cần gọi
//! [`crate::generate::Engine::set_veo3_guest_root_dir`] MỘT LẦN lúc khởi động (composition root),
//! gợi ý `<app_data_dir>/profiles/temp_profile` (cạnh `accounts/`). Chưa gọi = [`mint_token`] trả
//! lỗi rõ ràng (không panic, không tự bịa đường dẫn tuyệt đối).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use lvc_browser::veo3::captcha::{GuestCaptchaBrowser, GuestCaptchaOpts};
use lvc_browser::veo3::harvest::{HarvestOpts, harvest_session};
use lvc_veo3::error::VeoError;
use lvc_veo3::flow::Flow;
use lvc_veo3::job::{
    ImageJob, ImageJobDone, ImageJobsReport, VideoJob, VideoJobDone, VideoJobsReport,
};
use lvc_veo3::{FlowSession, VeoImageHost, VeoVideoHost};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Trạng thái dùng chung cho dịch vụ `VeoCaptchaHost` — một bản DUY NHẤT sống trên `Engine` (không
/// phải per-run) vì mục tiêu là "1 trình duyệt khách sống, dùng lại tuần tự", không phải mở mới mỗi
/// lần chạy workflow.
pub(crate) struct EngineVeoCaptcha {
    root_dir: OnceLock<PathBuf>,
    browser: Mutex<Option<GuestCaptchaBrowser>>,
}

impl EngineVeoCaptcha {
    pub(crate) fn new() -> Self {
        Self {
            root_dir: OnceLock::new(),
            browser: Mutex::new(None),
        }
    }

    /// `lvc-app` gọi MỘT LẦN lúc khởi động (xem doc module) — lần sau bị bỏ qua, trả `false`.
    pub(crate) fn set_root_dir(&self, dir: impl Into<PathBuf>) -> bool {
        self.root_dir.set(dir.into()).is_ok()
    }

    /// Mint 1 token reCAPTCHA cho `action` (`"IMAGE_GENERATION"` | `"IMAGE_UPLOAD"` |
    /// `"VIDEO_GENERATION"` — xem `lvc_veo3::VeoCaptchaHost::mint_token`). Khoá mutex suốt cả bước
    /// mở (nếu cần)/mint/xoay để hai lời gọi đồng thời không cùng đụng vào một `GuestCaptchaBrowser`.
    ///
    /// # Errors
    /// [`VeoError`] khi CHƯA gọi [`Self::set_root_dir`], hoặc khi mở/mint trình duyệt lỗi
    /// (`BrowserError` được bọc qua `Display` — chuỗi này KHÔNG chứa cookie tài khoản, trình duyệt
    /// khách không hề nạp cookie tài khoản nào).
    pub(crate) async fn mint_token(&self, action: &str) -> Result<String, VeoError> {
        let Some(root_dir) = self.root_dir.get() else {
            return Err(VeoError {
                code: None,
                class: None,
                message: "chua cau hinh thu muc profile khach reCAPTCHA Veo3 - lvc-app can goi \
                          Engine::set_veo3_guest_root_dir luc khoi dong"
                    .to_string(),
            });
        };
        let opts = GuestCaptchaOpts::default();
        let mut guard = self.browser.lock().await;
        if guard.is_none() {
            let browser = GuestCaptchaBrowser::open(root_dir, &opts)
                .await
                .map_err(|e| VeoError::from_text(&e.to_string()))?;
            *guard = Some(browser);
        }
        // Vay muon ngan de goi mint() roi tra lai quyen quyet dinh xoay/dong cho khoi duoi - tranh
        // giu 2 borrow (`&mut` cho mint, `&` cho should_rotate) chong cheo tren cung `guard`.
        let mint_result = {
            let browser = guard.as_mut().expect("vua dam bao Some ngay tren");
            browser.mint(action, opts.mint_timeout).await
        };
        let token = mint_result.map_err(|e| VeoError::from_text(&e.to_string()))?;
        let should_rotate = guard
            .as_ref()
            .expect("vua dam bao Some ngay tren")
            .should_rotate();
        if should_rotate {
            if let Some(old) = guard.take() {
                old.close().await;
            }
        }
        Ok(token)
    }
}

/// P5 Task 9b: nguon `profile_dir` cho MOT `credential_id` (`google/login` trong `lvc-vault`) — tach
/// trait de `lvc-engine` KHONG phu thuoc `lvc-vault` truc tiep, dung y `TextSecretResolver` (xem
/// `crate::textgen`). `lvc-app` se impl bang Vault (doc `credential.meta.profile_dir` — xem doc dau
/// `crate::login`: credential kind `session`/`login` cua Google luu `profile_dir` tuyet doi trong
/// `meta`) — KHONG phai viec cua Task 9b.
#[async_trait]
pub trait VeoAccountResolver: Send + Sync {
    /// # Errors
    /// Chuoi mo ta AN TOAN (khong chua bi mat) khi khong tim/khong doc duoc `profile_dir`.
    async fn profile_dir(&self, credential_id: &str) -> Result<PathBuf, String>;
}

/// Resolver mac dinh: LUON loi (dung khi chua cam kho tai khoan — vd CLI/test). Nho vay engine van
/// chen dich vu `VeoSessionHost` VO DIEU KIEN; node bao loi ro rang thay vi thieu service — dung y
/// [`crate::textgen::NoResolver`].
pub(crate) struct NoAccountResolver;

#[async_trait]
impl VeoAccountResolver for NoAccountResolver {
    async fn profile_dir(&self, _credential_id: &str) -> Result<PathBuf, String> {
        Err("chua cau hinh kho tai khoan (khong resolve duoc profile_dir cho Veo3)".to_string())
    }
}

/// P5 Task 9b: dich vu `VeoSessionHost` do engine cap — harvest phien 1 `credential_id` (mo profile
/// Chromium THAT cua tai khoan do, doc `at`/`bl`/`fsid`/cookie, dong trinh duyet ngay) roi CACHE lai
/// theo `credential_id` (giong Python `get_shared_flow_client`: harvest 1 LAN, tai su dung cho moi
/// lan goi sau — KHONG harvest lai moi request). `resolver` do `lvc-app` cap MOT LAN qua
/// [`Self::set_resolver`] (mac dinh [`NoAccountResolver`], giong `root_dir` cua [`EngineVeoCaptcha`]).
///
/// CHUA co co che het-han/401-invalidate (Python `drop_shared_flow_client`) — de sau, xem bao cao
/// Task 9b/9c (khong co `MEMORY.md` san co trong `lvc-engine` de ghi tien le nay, xem doc module).
pub(crate) struct EngineVeoSession {
    resolver: OnceLock<Arc<dyn VeoAccountResolver>>,
    cache: Mutex<HashMap<String, FlowSession>>,
}

impl EngineVeoSession {
    pub(crate) fn new() -> Self {
        Self {
            resolver: OnceLock::new(),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// `lvc-app` goi MOT LAN luc khoi dong (composition root, giong `TextSecretResolver`) — lan sau
    /// bi bo qua, tra `false`.
    pub(crate) fn set_resolver(&self, resolver: Arc<dyn VeoAccountResolver>) -> bool {
        self.resolver.set(resolver).is_ok()
    }

    fn resolver(&self) -> Arc<dyn VeoAccountResolver> {
        self.resolver
            .get()
            .cloned()
            .unwrap_or_else(|| Arc::new(NoAccountResolver))
    }

    /// Tra phien da cache theo `credential_id` neu co; khong thi resolve `profile_dir` roi harvest
    /// MOI (mo trinh duyet thuc — xem `lvc_browser::veo3::harvest::harvest_session`), luu cache roi
    /// tra ve. Khoa mutex suot ca buoc kiem-cache + harvest de hai loi goi cung `credential_id`
    /// khong cung mo 2 trinh duyet mot luc (don gian hon Python — khong hang doi rieng, chi tuan tu
    /// qua `Mutex`, dung y [`EngineVeoCaptcha`]).
    ///
    /// # Errors
    /// [`VeoError`] khi resolver loi (chua cau hinh/khong tim credential) hoac `harvest_session` loi
    /// (chua dang nhap, timeout doc token, thieu cookie dang nhap, loi CDP).
    pub(crate) async fn harvest(&self, credential_id: &str) -> Result<FlowSession, VeoError> {
        let mut cache = self.cache.lock().await;
        if let Some(session) = cache.get(credential_id) {
            return Ok(session.clone());
        }
        let profile_dir = self
            .resolver()
            .profile_dir(credential_id)
            .await
            .map_err(|e| VeoError::from_text(&e))?;
        let harvested = harvest_session(&profile_dir, &HarvestOpts::default())
            .await
            .map_err(|e| VeoError::from_text(&e.to_string()))?;
        let session = FlowSession {
            cookie_header: harvested.cookie_header,
            at: harvested.at,
            bl: harvested.bl,
            f_sid: harvested.f_sid,
            project_id: harvested.project_id,
        };
        cache.insert(credential_id.to_string(), session.clone());
        Ok(session)
    }
}

/// P5 Task 9c: ten file luu (trong `downloads_dir`) cho MOT canh ANH da tao xong — mirror quy uoc
/// dat ten `<run_id>_<node_id>_<scene_index>` ma buoc Luu dung cho video Dola (xem doc `crate::dag`,
/// `SaveJob::from_node`), ap dung THANG luc tai ve (Veo3 khong co buoc browser-download trung gian
/// voi ten ngau nhien nhu Dola — `Flow::download` nhan duong dan CHINH XAC tu dau, node/engine
/// quyet truoc, xem `lvc_veo3::flow::Flow::download`). LUON `.png` (model `GEM_PIX_2`/Nano Banana
/// cua Google luon tra PNG; URL Google la ky + tam thoi, khong dung duoi URL de doan mime — don
/// gian hon la dung).
fn image_out_path(downloads_dir: &Path, run_id: &str, node_id: &str, scene_index: u32) -> PathBuf {
    downloads_dir.join(format!("{run_id}_{node_id}_{scene_index}.png"))
}

/// Nhu [`image_out_path`] nhung cho VIDEO (`.mp4` — Veo3 luon tra mp4).
fn video_out_path(downloads_dir: &Path, run_id: &str, node_id: &str, scene_index: u32) -> PathBuf {
    downloads_dir.join(format!("{run_id}_{node_id}_{scene_index}.mp4"))
}

/// Nhip nghi giua 2 lan poll video + so lan toi da (`7s * 60 ~ 7 phut`, trong khoang 5-10 phut spec
/// goi y — xem Python `monitor_video_generation`/`wait_media`, khong can khop chinh xac con so).
const VIDEO_POLL_INTERVAL: Duration = Duration::from_secs(7);
const VIDEO_POLL_MAX_ATTEMPTS: u32 = 60;

/// P5 Task 9c: dich vu `VeoImageHost`/`VeoVideoHost` cho MOT lan chay — be mat NODE thuc
/// (`veo3_image`/`veo3_video` chi biet 2 trait nay qua `ctx.services`, dung khuon
/// `lvc_dola::DolaVideoHost`). Ghep [`Flow`] (da gan `VeoCaptchaHost`+`VeoSessionHost` cua Task
/// 9/9b luc tao, xem `crate::dag::start_run`) + tu quyet `local_path`
/// ([`image_out_path`]/[`video_out_path`]) — `Flow` KHONG tu quyet duong dan (xem doc
/// `lvc_veo3::flow`).
///
/// KHONG generic theo `BrowserDriver` (khac `RunDolaHost`/`RunVeoCaptchaHost` trong `dag.rs`) — chi
/// can `downloads_dir` (sao tu `EngineConfig`) + `Flow` (da ghep san 2 host con) + token huy CUA RUN
/// (truyen vao luc tao trong `start_run` — video co the poll nhieu phut nen PHAI kiem huy giua cac
/// lan poll, giong `hooks.token` cua Dola; KHONG kiem huy GIUA mot lan poll dang cho HTTP tra loi —
/// mot lan poll la 1 request ngan, chap nhan do tre nay, xem doc [`EngineVeoJobs::run_one_video`]).
pub(crate) struct EngineVeoJobs {
    downloads_dir: PathBuf,
    flow: Flow,
    cancel: CancellationToken,
}

impl EngineVeoJobs {
    pub(crate) fn new(downloads_dir: PathBuf, flow: Flow, cancel: CancellationToken) -> Self {
        Self {
            downloads_dir,
            flow,
            cancel,
        }
    }

    /// Mot canh VIDEO: submit → poll toi da [`VIDEO_POLL_MAX_ATTEMPTS`] lan (nghi
    /// [`VIDEO_POLL_INTERVAL`] giua 2 lan) toi khi `media.done` (co URL) hoac qua han/huy → tai ve.
    async fn run_one_video(
        &self,
        run_id: &str,
        node_id: &str,
        job: &VideoJob,
    ) -> Result<PathBuf, String> {
        let scene_index = job.scene_index;
        if self.cancel.is_cancelled() {
            return Err("da huy".to_string());
        }
        let op = self
            .flow
            .submit_video(job)
            .await
            .map_err(|e| e.to_string())?;
        let Some(operation_id) = op.operation_id else {
            return Err("khong nhan duoc operation_id tu Google".to_string());
        };
        for attempt in 0..VIDEO_POLL_MAX_ATTEMPTS {
            if self.cancel.is_cancelled() {
                return Err("da huy".to_string());
            }
            let media = self
                .flow
                .poll_video_once(&job.credential_id, &operation_id)
                .await
                .map_err(|e| e.to_string())?;
            if media.done {
                let Some(url) = media.url else {
                    return Err("media.done nhung khong co URL".to_string());
                };
                let path = video_out_path(&self.downloads_dir, run_id, node_id, scene_index);
                self.flow
                    .download(&url, &path)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(path);
            }
            if attempt + 1 < VIDEO_POLL_MAX_ATTEMPTS {
                tokio::select! {
                    () = self.cancel.cancelled() => return Err("da huy".to_string()),
                    () = tokio::time::sleep(VIDEO_POLL_INTERVAL) => {}
                }
            }
        }
        Err(format!(
            "qua han cho video sau {VIDEO_POLL_MAX_ATTEMPTS} lan poll"
        ))
    }
}

#[async_trait]
impl VeoImageHost for EngineVeoJobs {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport {
        let mut report = ImageJobsReport::default();
        for job in jobs {
            let scene_index = job.scene_index;
            let outcome: Result<ImageJobDone, String> = async {
                let result = self
                    .flow
                    .generate_image(&job)
                    .await
                    .map_err(|e| e.to_string())?;
                let url = result
                    .url
                    .ok_or_else(|| "khong nhan duoc URL anh".to_string())?;
                let path = image_out_path(&self.downloads_dir, run_id, node_id, scene_index);
                self.flow
                    .download(&url, &path)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(ImageJobDone {
                    scene_index,
                    local_path: path,
                    width: result.width,
                    height: result.height,
                })
            }
            .await;
            match outcome {
                Ok(done) => report.done.push(done),
                Err(msg) => report.failed.push(format!("canh {scene_index}: {msg}")),
            }
        }
        report
    }
}

#[async_trait]
impl VeoVideoHost for EngineVeoJobs {
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport {
        let mut report = VideoJobsReport::default();
        for job in jobs {
            let scene_index = job.scene_index;
            match self.run_one_video(run_id, node_id, &job).await {
                Ok(local_path) => report.done.push(VideoJobDone {
                    scene_index,
                    local_path,
                }),
                Err(msg) => report.failed.push(format!("canh {scene_index}: {msg}")),
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn chua_dat_root_dir_thi_bao_loi_ro_rang() {
        let state = EngineVeoCaptcha::new();
        let err = state.mint_token("IMAGE_GENERATION").await.unwrap_err();
        assert!(err.message.contains("set_veo3_guest_root_dir"), "{err:?}");
    }

    #[test]
    fn set_root_dir_lan_dau_true_lan_sau_false() {
        let state = EngineVeoCaptcha::new();
        assert!(state.set_root_dir("a"));
        assert!(!state.set_root_dir("b"));
    }

    // ---- EngineVeoSession (Task 9b) — mirror 2 test cua EngineVeoCaptcha o tren + 1 test cache ----

    struct DummyResolver;

    #[async_trait]
    impl VeoAccountResolver for DummyResolver {
        async fn profile_dir(&self, _credential_id: &str) -> Result<PathBuf, String> {
            Err("khong dung toi trong test nay".to_string())
        }
    }

    #[tokio::test]
    async fn veo_session_chua_dat_resolver_thi_bao_loi_ro_rang() {
        let state = EngineVeoSession::new();
        let err = state.harvest("cred1").await.unwrap_err();
        assert!(
            err.message.contains("khong resolve duoc profile_dir"),
            "{err:?}"
        );
    }

    #[test]
    fn veo_session_set_resolver_lan_dau_true_lan_sau_false() {
        let state = EngineVeoSession::new();
        assert!(state.set_resolver(Arc::new(DummyResolver)));
        assert!(!state.set_resolver(Arc::new(DummyResolver)));
    }

    /// Cache theo `credential_id` phai tra ve NGAY khi da co, KHONG goi lai resolver/harvest —
    /// resolver mac dinh ([`NoAccountResolver`]) chua duoc `set_resolver` va SE loi neu bi goi, nen
    /// `Ok` o day chung minh cache duoc dung truoc.
    #[tokio::test]
    async fn veo_session_da_co_trong_cache_thi_tra_ngay_khong_can_resolver() {
        let state = EngineVeoSession::new();
        let cached = FlowSession {
            cookie_header: "SID=x".to_string(),
            at: "at1".to_string(),
            bl: "bl1".to_string(),
            f_sid: "fsid1".to_string(),
            project_id: "proj1".to_string(),
        };
        state
            .cache
            .lock()
            .await
            .insert("cred1".to_string(), cached.clone());
        let got = state.harvest("cred1").await.unwrap();
        assert_eq!(got.at, cached.at);
        assert_eq!(got.project_id, cached.project_id);
    }

    // ---- EngineVeoJobs (Task 9c) ----

    struct FakeCaptcha {
        err: Option<VeoError>,
    }

    #[async_trait]
    impl lvc_veo3::VeoCaptchaHost for FakeCaptcha {
        async fn mint_token(&self, _action: &str) -> Result<String, VeoError> {
            match &self.err {
                Some(e) => Err(e.clone()),
                None => Ok("token".to_string()),
            }
        }
    }

    struct FakeSession {
        err: Option<VeoError>,
    }

    #[async_trait]
    impl lvc_veo3::VeoSessionHost for FakeSession {
        async fn harvest(&self, _credential_id: &str) -> Result<FlowSession, VeoError> {
            match &self.err {
                Some(e) => Err(e.clone()),
                None => Ok(FlowSession {
                    cookie_header: "SID=x".into(),
                    at: "at".into(),
                    bl: "bl".into(),
                    f_sid: "fsid".into(),
                    project_id: "proj".into(),
                }),
            }
        }
    }

    fn jobs_host(captcha_err: Option<VeoError>, session_err: Option<VeoError>) -> EngineVeoJobs {
        let flow = Flow::new(
            Arc::new(FakeCaptcha { err: captcha_err }),
            Arc::new(FakeSession { err: session_err }),
        );
        EngineVeoJobs::new(PathBuf::from("dl"), flow, CancellationToken::new())
    }

    fn image_job(scene_index: u32) -> ImageJob {
        ImageJob {
            scene_index,
            credential_id: "cred1".into(),
            prompt: "mot con meo".into(),
            model: lvc_veo3::model::ImageModel::GemPix2,
            aspect: lvc_veo3::model::Aspect::Portrait,
            seed: 1,
            ref_image_paths: vec![],
        }
    }

    fn video_job(scene_index: u32) -> VideoJob {
        VideoJob {
            scene_index,
            credential_id: "cred1".into(),
            source: lvc_veo3::job::VideoSource::Text {
                prompt: "mot con meo".into(),
            },
            model: lvc_veo3::model::default_video_model::TEXT_TO_VIDEO.to_string(),
            aspect: lvc_veo3::model::Aspect::Portrait,
            seed_count: 1,
        }
    }

    #[test]
    fn image_out_path_luon_png() {
        let p = image_out_path(Path::new("dl"), "run1", "img", 2);
        assert_eq!(p, PathBuf::from("dl").join("run1_img_2.png"));
    }

    #[test]
    fn video_out_path_luon_mp4() {
        let p = video_out_path(Path::new("dl"), "run1", "vid", 3);
        assert_eq!(p, PathBuf::from("dl").join("run1_vid_3.mp4"));
    }

    /// Loi mint captcha xay ra TRUOC khi cham mang (`generate_image` goi `mint_token` sau
    /// `session_for`) — canh do phai vao `failed`, KHONG panic, node khac (khong co o day) khong bi
    /// anh huong.
    #[tokio::test]
    async fn generate_images_loi_mint_thi_vao_failed_khong_goi_mang() {
        let host = jobs_host(Some(VeoError::from_text("loi mint gia")), None);
        let report = host
            .generate_images("run1", "img", vec![image_job(1)])
            .await;
        assert!(report.done.is_empty());
        assert_eq!(report.failed.len(), 1);
        assert!(
            report.failed[0].starts_with("canh 1:"),
            "{:?}",
            report.failed
        );
    }

    /// Loi harvest phien (`VeoSessionHost`) cung phai vao `failed` giong loi mint — mot canh loi
    /// khong lam hong canh khac (o day chi co 1 canh, nhung cau truc vong lap ho tro nhieu canh).
    #[tokio::test]
    async fn generate_images_loi_harvest_thi_vao_failed() {
        let host = jobs_host(None, Some(VeoError::from_text("loi harvest gia")));
        let report = host
            .generate_images("run1", "img", vec![image_job(1)])
            .await;
        assert!(report.done.is_empty());
        assert_eq!(report.failed.len(), 1);
    }

    /// `submit_video` (T2V) cung goi `session_for` roi `mint_token` TRUOC khi cham mang — loi mint
    /// phai vao `failed` ma KHONG can vong poll nao.
    #[tokio::test]
    async fn generate_videos_loi_mint_thi_vao_failed_khong_can_poll() {
        let host = jobs_host(Some(VeoError::from_text("loi mint gia")), None);
        let report = host
            .generate_videos("run1", "vid", vec![video_job(1)])
            .await;
        assert!(report.done.is_empty());
        assert_eq!(report.failed.len(), 1);
        assert!(
            report.failed[0].starts_with("canh 1:"),
            "{:?}",
            report.failed
        );
    }

    /// Token huy CUA RUN da huy TRUOC khi submit -> dung ngay, khong goi mang (kiem tra o dau
    /// [`EngineVeoJobs::run_one_video`], truoc `submit_video`).
    #[tokio::test]
    async fn generate_videos_da_huy_truoc_khi_submit_thi_dung_ngay() {
        let flow = Flow::new(
            Arc::new(FakeCaptcha { err: None }),
            Arc::new(FakeSession { err: None }),
        );
        let cancel = CancellationToken::new();
        cancel.cancel();
        let host = EngineVeoJobs::new(PathBuf::from("dl"), flow, cancel);
        let report = host
            .generate_videos("run1", "vid", vec![video_job(1)])
            .await;
        assert!(report.done.is_empty());
        assert_eq!(report.failed, vec!["canh 1: da huy".to_string()]);
    }
}
