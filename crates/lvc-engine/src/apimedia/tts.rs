//! Dịch vụ host TTS (`ElevenLabs`/`MiniMax`) — resolve key qua [`TextSecretResolver`] (dùng lại NGUYÊN
//! VẸN, không thêm resolver riêng — xem doc `apimedia/mod.rs`) rồi gọi [`lvc_tts::TtsClient`].
//! Khác Grok/Google/Seedance (provider tag CỐ ĐỊNH một dòng): mỗi [`TtsJob`] tự mang
//! [`lvc_tts::Provider`] riêng (node `tts` chọn "elevenlabs"/"minimax" qua tham số `provider`), nên
//! tag resolve lấy từ `job.provider.as_tag()` thay vì hằng số.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lvc_tts::{SynthReq, TtsClient, TtsHost, TtsJob, TtsJobDone, TtsJobsReport};

use super::util::{media_out_path, write_bytes};
use crate::textgen::TextSecretResolver;

pub(crate) struct EngineTts {
    client: Arc<TtsClient>,
    resolver: Arc<dyn TextSecretResolver>,
    downloads_dir: PathBuf,
}

impl EngineTts {
    pub(crate) fn new(
        client: Arc<TtsClient>,
        resolver: Arc<dyn TextSecretResolver>,
        downloads_dir: PathBuf,
    ) -> Self {
        Self {
            client,
            resolver,
            downloads_dir,
        }
    }

    async fn run_one(&self, run_id: &str, node_id: &str, job: &TtsJob) -> Result<PathBuf, String> {
        let secret = self
            .resolver
            .resolve(job.provider.as_tag(), &job.credential_id)
            .await?;
        let req = SynthReq {
            provider: job.provider,
            text: job.text.clone(),
            voice_id: job.voice_id.clone(),
            model: job.model.clone(),
            speed: job.speed,
        };
        let bytes = self
            .client
            .generate_with_key(&secret.api_key, secret.base_url.as_deref(), &req)
            .await
            .map_err(|e| e.to_string())?;
        let path = media_out_path(&self.downloads_dir, run_id, node_id, job.scene_index, "mp3");
        write_bytes(&path, &bytes).await?;
        Ok(path)
    }
}

#[async_trait]
impl TtsHost for EngineTts {
    async fn generate_audios(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<TtsJob>,
    ) -> TtsJobsReport {
        let mut report = TtsJobsReport::default();
        for job in jobs {
            let scene_index = job.scene_index;
            match self.run_one(run_id, node_id, &job).await {
                Ok(local_path) => report.done.push(TtsJobDone {
                    scene_index,
                    local_path,
                }),
                Err(msg) => report.failed.push(format!("canh {scene_index}: {msg}")),
            }
        }
        report
    }
}
