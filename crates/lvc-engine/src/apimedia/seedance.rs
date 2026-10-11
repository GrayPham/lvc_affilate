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
use tokio_util::sync::CancellationToken;

use super::util::{media_out_path, read_data_url, write_bytes};
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
}

impl EngineSeedanceVideo {
    pub(crate) fn new(
        client: Arc<SeedanceClient>,
        resolver: Arc<dyn TextSecretResolver>,
        downloads_dir: PathBuf,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            client,
            resolver,
            downloads_dir,
            cancel,
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
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport {
        let mut report = VideoJobsReport::default();
        for job in jobs {
            let scene_index = job.scene_index;
            match self.run_one(run_id, node_id, &job).await {
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
