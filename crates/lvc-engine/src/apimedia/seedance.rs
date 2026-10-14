//! Dịch vụ host Seedance/Seedream (Volcengine) — resolve key qua [`TextSecretResolver`] rồi gọi
//! [`lvc_seedance::SeedanceClient`]. i2v: đọc ảnh gốc thành data URL.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lvc_seedance::job::{
    ImageJob, ImageJobDone, ImageJobsReport, VideoJob, VideoJobDone, VideoJobsReport,
};
use lvc_seedance::{ImageReq, SeedanceClient, SeedanceImageHost, SeedanceVideoHost, VideoReq};
use lvc_store::TaskRepo;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::task_tracking::{
    announce_batch, create_pending_task, finish_task_done, finish_task_failed,
};
use super::util::{media_out_path, read_data_url, write_bytes};
use crate::dag::TaskRef;
use crate::textgen::TextSecretResolver;

const PROVIDER_TAG: &str = "seedance";
const VIDEO_POLL_INTERVAL: Duration = Duration::from_secs(7);
const VIDEO_POLL_MAX_ATTEMPTS: u32 = 90;

pub(crate) struct EngineSeedanceImage {
    client: Arc<SeedanceClient>,
    resolver: Arc<dyn TextSecretResolver>,
    downloads_dir: PathBuf,
}

impl EngineSeedanceImage {
    pub(crate) fn new(
        client: Arc<SeedanceClient>,
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
        };
        let bytes = self
            .client
            .generate_image_with_key(&secret.api_key, secret.base_url.as_deref(), &req)
            .await
            .map_err(|e| e.to_string())?;
        let path = media_out_path(&self.downloads_dir, run_id, node_id, job.scene_index, "png");
        write_bytes(&path, &bytes).await?;
        Ok(path)
    }
}

#[async_trait]
impl SeedanceImageHost for EngineSeedanceImage {
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

pub(crate) struct EngineSeedanceVideo {
    client: Arc<SeedanceClient>,
    resolver: Arc<dyn TextSecretResolver>,
    downloads_dir: PathBuf,
    cancel: CancellationToken,
    tasks: TaskRepo,
    tasks_ready_tx: watch::Sender<Vec<TaskRef>>,
}

impl EngineSeedanceVideo {
    pub(crate) fn new(
        client: Arc<SeedanceClient>,
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
        let image_url = match &job.ref_image_path {
            Some(path) => Some(read_data_url(path).await?),
            None => None,
        };
        Ok(VideoReq {
            model: job.model.clone(),
            prompt: job.prompt.clone(),
            aspect: job.aspect,
            image_url,
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
        let task_id = self
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
                .poll_video_once_with_key(&secret.api_key, secret.base_url.as_deref(), &task_id)
                .await
                .map_err(|e| e.to_string())?;
            if poll.done {
                let url = poll.url.ok_or("task xong nhung khong co video_url")?;
                let bytes = self
                    .client
                    .download_bytes(&url)
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
impl SeedanceVideoHost for EngineSeedanceVideo {
    /// 2 giai đoạn — xem doc cùng tên ở `EngineGoogleVideo::generate_videos`/`task_tracking`.
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
