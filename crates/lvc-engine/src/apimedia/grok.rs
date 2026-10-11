//! Dịch vụ host Grok (xAI, chỉ ảnh) — resolve key qua [`TextSecretResolver`] rồi gọi
//! [`lvc_grok::GrokClient`].

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lvc_grok::job::{ImageJob, ImageJobDone, ImageJobsReport};
use lvc_grok::{GrokClient, GrokImageHost, ImageReq};

use super::util::{media_out_path, write_bytes};
use crate::textgen::TextSecretResolver;

const PROVIDER_TAG: &str = "grok";

pub(crate) struct EngineGrokImage {
    client: Arc<GrokClient>,
    resolver: Arc<dyn TextSecretResolver>,
    downloads_dir: PathBuf,
}

impl EngineGrokImage {
    pub(crate) fn new(
        client: Arc<GrokClient>,
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
impl GrokImageHost for EngineGrokImage {
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
