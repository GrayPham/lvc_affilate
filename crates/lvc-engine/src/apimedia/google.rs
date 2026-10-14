//! Dịch vụ host Google (Nano Banana ảnh, Veo video) — resolve key qua [`TextSecretResolver`] rồi gọi
//! [`lvc_google::GoogleClient`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lvc_google::job::{
    ImageJob, ImageJobDone, ImageJobsReport, VideoJob, VideoJobDone, VideoJobsReport,
};
use lvc_google::{GoogleClient, GoogleImageHost, GoogleVideoHost, ImageInput, ImageReq, VideoReq};
use lvc_store::TaskRepo;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::task_tracking::{
    announce_batch, create_pending_task, finish_task_done, finish_task_failed,
};
use super::util::{ext_from_mime, media_out_path, mime_from_path, write_bytes};
use crate::dag::TaskRef;
use crate::textgen::TextSecretResolver;

/// Provider tag trong vault — Google official DÙNG CHUNG key với Gemini text (xem `lvc-google/MEMORY.md`).
const PROVIDER_TAG: &str = "gemini";
const VIDEO_POLL_INTERVAL: Duration = Duration::from_secs(10);
const VIDEO_POLL_MAX_ATTEMPTS: u32 = 90;

pub(crate) struct EngineGoogleImage {
    client: Arc<GoogleClient>,
    resolver: Arc<dyn TextSecretResolver>,
    downloads_dir: PathBuf,
}

impl EngineGoogleImage {
    pub(crate) fn new(
        client: Arc<GoogleClient>,
        resolver: Arc<dyn TextSecretResolver>,
        downloads_dir: PathBuf,
    ) -> Self {
        Self {
            client,
            resolver,
            downloads_dir,
        }
    }

    async fn run_one(
        &self,
        run_id: &str,
        node_id: &str,
        job: &ImageJob,
    ) -> Result<PathBuf, String> {
        let secret = self
            .resolver
            .resolve(PROVIDER_TAG, &job.credential_name)
            .await?;
        let req = ImageReq {
            model: job.model.clone(),
            prompt: job.prompt.clone(),
            aspect: job.aspect,
            sample_count: 1,
        };
        let images = self
            .client
            .generate_image_with_key(&secret.api_key, secret.base_url.as_deref(), &req)
            .await
            .map_err(|e| e.to_string())?;
        let first = images.into_iter().next().ok_or("API tra ve rong")?;
        let path = media_out_path(
            &self.downloads_dir,
            run_id,
            node_id,
            job.scene_index,
            ext_from_mime(&first.mime),
        );
        write_bytes(&path, &first.bytes).await?;
        Ok(path)
    }
}

#[async_trait]
impl GoogleImageHost for EngineGoogleImage {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport {
        let mut report = ImageJobsReport::default();
        for job in jobs {
            let scene_index = job.scene_index;
            match self.run_one(run_id, node_id, &job).await {
                Ok(local_path) => report.done.push(ImageJobDone {
                    scene_index,
                    local_path,
                }),
                Err(msg) => report.failed.push(format!("canh {scene_index}: {msg}")),
            }
        }
        report
    }
}

pub(crate) struct EngineGoogleVideo {
    client: Arc<GoogleClient>,
    resolver: Arc<dyn TextSecretResolver>,
    downloads_dir: PathBuf,
    cancel: CancellationToken,
    tasks: TaskRepo,
    tasks_ready_tx: watch::Sender<Vec<TaskRef>>,
}

impl EngineGoogleVideo {
    pub(crate) fn new(
        client: Arc<GoogleClient>,
        resolver: Arc<dyn TextSecretResolver>,
        downloads_dir: PathBuf,
        cancel: CancellationToken,
        tasks: TaskRepo,
        tasks_ready_tx: watch::Sender<Vec<TaskRef>>,
    ) -> Self {
        Self {
            client,
            resolver,
            downloads_dir,
            cancel,
            tasks,
            tasks_ready_tx,
        }
    }

    async fn build_req(&self, job: &VideoJob) -> Result<VideoReq, String> {
        let image = match &job.ref_image_path {
            Some(path) => {
                let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
                Some(ImageInput {
                    bytes,
                    mime: mime_from_path(path).to_string(),
                })
            }
            None => None,
        };
        Ok(VideoReq {
            model: job.model.clone(),
            prompt: job.prompt.clone(),
            aspect: job.aspect,
            sample_count: 1,
            image,
        })
    }

    async fn run_one(
        &self,
        run_id: &str,
        node_id: &str,
        job: &VideoJob,
    ) -> Result<PathBuf, String> {
        if self.cancel.is_cancelled() {
            return Err("da huy".to_string());
        }
        let secret = self
            .resolver
            .resolve(PROVIDER_TAG, &job.credential_name)
            .await?;
        let req = self.build_req(job).await?;
        let operation = self
            .client
            .submit_video_with_key(&secret.api_key, secret.base_url.as_deref(), &req)
            .await
            .map_err(|e| e.to_string())?;

        for attempt in 0..VIDEO_POLL_MAX_ATTEMPTS {
            if self.cancel.is_cancelled() {
                return Err("da huy".to_string());
            }
            let poll = self
                .client
                .poll_video_once_with_key(&secret.api_key, secret.base_url.as_deref(), &operation)
                .await
                .map_err(|e| e.to_string())?;
            if poll.done {
                let uri = poll.uri.ok_or("operation xong nhung khong co URI video")?;
                let bytes = self
                    .client
                    .download_video_with_key(&secret.api_key, &uri)
                    .await
                    .map_err(|e| e.to_string())?;
                let path =
                    media_out_path(&self.downloads_dir, run_id, node_id, job.scene_index, "mp4");
                write_bytes(&path, &bytes).await?;
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
impl GoogleVideoHost for EngineGoogleVideo {
    /// 2 giai đoạn (xem `task_tracking` module doc): (1) tạo HẾT dòng `TaskRow` + báo
    /// `tasks_ready` MỘT LẦN — để `Engine::run_script` ("kịch bản") ánh xạ `scene_id -> task_id`
    /// NGAY, không phải chờ video tạo xong; (2) mới chạy việc thật (submit/poll/tải), ghi lại
    /// dòng tương ứng khi xong. Dòng nào tạo `TaskRow` thất bại thì bỏ qua giai đoạn 2 cho job đó
    /// (đã nằm trong `report.failed`), không chặn các job còn lại.
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport {
        let mut report = VideoJobsReport::default();

        let mut pending: Vec<(VideoJob, String)> = Vec::with_capacity(jobs.len());
        let mut refs: Vec<(u32, String)> = Vec::with_capacity(jobs.len());
        for job in jobs {
            match create_pending_task(
                &self.tasks,
                run_id,
                node_id,
                job.scene_index,
                &job.prompt,
                &job.credential_name,
            )
            .await
            {
                Ok(task_id) => {
                    refs.push((job.scene_index, task_id.clone()));
                    pending.push((job, task_id));
                }
                Err(e) => report.failed.push(format!(
                    "canh {}: khong tao duoc dong task: {e}",
                    job.scene_index
                )),
            }
        }
        announce_batch(&self.tasks_ready_tx, node_id, &refs);

        for (job, task_id) in pending {
            let scene_index = job.scene_index;
            match self.run_one(run_id, node_id, &job).await {
                Ok(local_path) => {
                    finish_task_done(&self.tasks, &task_id, &local_path).await;
                    report.done.push(VideoJobDone {
                        scene_index,
                        local_path,
                    });
                }
                Err(msg) => {
                    finish_task_failed(&self.tasks, &task_id, &msg).await;
                    report.failed.push(format!("canh {scene_index}: {msg}"));
                }
            }
        }
        report
    }
}
