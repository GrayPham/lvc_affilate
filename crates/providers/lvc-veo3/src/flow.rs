//! Ghep [`crate::VeoCaptchaHost`] (mint) + [`crate::VeoSessionHost`] (harvest/cache phien) +
//! [`crate::client::BatchExecuteClient`] (goi HTTP) + `rpc::*` (build/extract) thanh vai ham
//! "mot lan goi la xong viec" — dung boi impl `VeoImageHost`/`VeoVideoHost` (thuong la
//! `lvc-engine`, xem `job.rs`). Node KHONG dung `Flow` truc tiep — chi 2 trait host cap cao moi
//! la be mat node thay (giong node chi biet `DolaVideoHost`, khong biet ben trong `dola::submit`).
//!
//! `Flow` KHONG tu quyet duong dan file dau ra (xem doc `job.rs`) — chi tra URL/`media_id`; ben
//! goi ([`Flow::download`]) tu chon `out_path`.

use std::path::Path;
use std::sync::Arc;

use crate::client::BatchExecuteClient;
use crate::error::VeoError;
use crate::job::{ImageJob, VideoJob, VideoSource};
use crate::rpc::{self, VideoOperation};
use crate::{FlowSession, VeoCaptchaHost, VeoSessionHost};

pub struct Flow {
    client: BatchExecuteClient,
    captcha: Arc<dyn VeoCaptchaHost>,
    session: Arc<dyn VeoSessionHost>,
}

impl Flow {
    #[must_use]
    pub fn new(captcha: Arc<dyn VeoCaptchaHost>, session: Arc<dyn VeoSessionHost>) -> Self {
        Self {
            client: BatchExecuteClient::new(),
            captcha,
            session,
        }
    }

    async fn session_for(&self, credential_id: &str) -> Result<FlowSession, VeoError> {
        self.session.harvest(credential_id).await
    }

    fn source_path(session: &FlowSession) -> String {
        format!("/project/{}", session.project_id)
    }

    /// Upload 1 anh THAM CHIEU tu file local -> `media_id`. Doc file + base64-hoa o day (khong
    /// bat node/engine tu lam) — mime doan tu duoi file (`.png` -> `image/png`, con lai
    /// `image/jpeg`, khop Python).
    ///
    /// # Errors
    /// [`VeoError`] khi doc file loi, harvest phien loi, mint token loi, hoac Google tra loi/rong.
    pub async fn upload_ref_image(
        &self,
        credential_id: &str,
        path: &Path,
    ) -> Result<String, VeoError> {
        let bytes = tokio::fs::read(path).await.map_err(|e| {
            VeoError::from_text(&format!("doc anh tham chieu {}: {e}", path.display()))
        })?;
        let base64_image = base64_encode(&bytes);
        let mime_type = mime_from_path(path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image.png")
            .to_string();
        let session = self.session_for(credential_id).await?;
        let token = self.captcha.mint_token("IMAGE_UPLOAD").await?;
        let ids = vec![rpc::new_uuid(), rpc::new_uuid()];
        let inner = rpc::upload::build_payload(
            &session.project_id,
            &token,
            &base64_image,
            &mime_type,
            &file_name,
            &ids,
        );
        let payload = self
            .client
            .call(
                &session,
                rpc::upload::RPC,
                &inner,
                &Self::source_path(&session),
            )
            .await?;
        rpc::upload::extract_media_id(&payload)
            .ok_or_else(|| VeoError::from_text("upload anh tham chieu: khong nhan duoc media_id"))
    }

    /// Tao 1 anh (dong bo — tra URL ngay, khong polling). Upload het `job.ref_image_paths` truoc
    /// (theo dung project cua phien harvest duoc — doi project phai upload lai, xem MEMORY.md).
    ///
    /// # Errors
    /// [`VeoError`] tu buoc upload, harvest, mint, hoac goi `rpc::image`.
    pub async fn generate_image(
        &self,
        job: &ImageJob,
    ) -> Result<rpc::image::ImageResult, VeoError> {
        let mut ref_media_ids = Vec::with_capacity(job.ref_image_paths.len());
        for path in &job.ref_image_paths {
            ref_media_ids.push(self.upload_ref_image(&job.credential_id, path).await?);
        }
        let session = self.session_for(&job.credential_id).await?;
        let token = self.captcha.mint_token("IMAGE_GENERATION").await?;
        let media_ids = vec![rpc::new_uuid(), rpc::new_uuid()];
        let workflow_id = rpc::new_uuid();
        let inner = rpc::image::build_payload(
            &job.prompt,
            &session.project_id,
            &token,
            &media_ids,
            &workflow_id,
            job.model.api_name(),
            job.seed,
            job.aspect.image_enum(),
            &ref_media_ids,
        );
        let payload = self
            .client
            .call(
                &session,
                rpc::image::RPC,
                &inner,
                &Self::source_path(&session),
            )
            .await?;
        Ok(rpc::image::extract(&payload))
    }

    /// Submit 1 job video (T2V hoac R2V theo [`VideoSource`]) — tra operation handle de
    /// [`Flow::poll_video_once`] lap sau (ben goi tu quan ly vong lap/huy, xem doc module).
    ///
    /// # Errors
    /// [`VeoError`] tu buoc upload ref (R2V), harvest, mint, hoac goi rpc submit.
    pub async fn submit_video(&self, job: &VideoJob) -> Result<VideoOperation, VeoError> {
        match &job.source {
            VideoSource::Text { prompt } => {
                let session = self.session_for(&job.credential_id).await?;
                let token = self.captcha.mint_token("VIDEO_GENERATION").await?;
                let n = job.seed_count.max(1) as usize;
                let media_ids: Vec<String> = (0..2 * n).map(|_| rpc::new_uuid()).collect();
                let workflow_id = rpc::new_uuid();
                let inner = rpc::text_to_video::build_payload(
                    prompt,
                    &job.model,
                    &session.project_id,
                    &token,
                    &media_ids,
                    &workflow_id,
                    job.aspect.video_enum(),
                    job.seed_count,
                );
                let payload = self
                    .client
                    .call(
                        &session,
                        rpc::text_to_video::RPC,
                        &inner,
                        &Self::source_path(&session),
                    )
                    .await?;
                Ok(rpc::extract_video_operation(&payload))
            }
            VideoSource::Reference {
                prompt,
                ref_image_paths,
            } => {
                let mut ref_media_ids = Vec::with_capacity(ref_image_paths.len());
                for path in ref_image_paths {
                    ref_media_ids.push(self.upload_ref_image(&job.credential_id, path).await?);
                }
                let session = self.session_for(&job.credential_id).await?;
                let token = self.captcha.mint_token("VIDEO_GENERATION").await?;
                let media_ids = vec![rpc::new_uuid(), rpc::new_uuid()];
                let workflow_id = rpc::new_uuid();
                // R2V: seed_count LUON = 1 o tang rpc (xem MEMORY.md) — khong dua job.seed_count vao.
                let inner = rpc::reference_to_video::build_payload(
                    prompt,
                    &job.model,
                    &session.project_id,
                    &token,
                    &media_ids,
                    &workflow_id,
                    &ref_media_ids,
                    job.aspect.video_enum(),
                    1,
                );
                let payload = self
                    .client
                    .call(
                        &session,
                        rpc::reference_to_video::RPC,
                        &inner,
                        &Self::source_path(&session),
                    )
                    .await?;
                Ok(rpc::extract_video_operation(&payload))
            }
        }
    }

    /// MOT lan poll `as29s` (khong sleep, khong lap) — ben goi tu quan ly khoang nghi + huy, giong
    /// `dola::poll` de engine kiem `CancellationToken` giua 2 lan (rpc nay KHONG can token
    /// reCAPTCHA moi, xem MEMORY.md).
    ///
    /// # Errors
    /// [`VeoError`] tu harvest hoac goi rpc.
    pub async fn poll_video_once(
        &self,
        credential_id: &str,
        operation_id: &str,
    ) -> Result<rpc::media::Media, VeoError> {
        let session = self.session_for(credential_id).await?;
        let inner = rpc::media::build_payload(operation_id);
        let payload = self
            .client
            .call(
                &session,
                rpc::media::RPC,
                &inner,
                &Self::source_path(&session),
            )
            .await?;
        Ok(rpc::media::extract(&payload, true))
    }

    /// Tai `url` (URL da ky cua Google, KHONG can cookie — chi Referer, giong Python
    /// `download()`) ve `out_path`. Tao thu muc cha neu chua co.
    ///
    /// # Errors
    /// [`VeoError`] khi goi mang loi, HTTP khong phai 2xx, hoac ghi file loi.
    pub async fn download(&self, url: &str, out_path: &Path) -> Result<(), VeoError> {
        if let Some(parent) = out_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                VeoError::from_text(&format!("tao thu muc {}: {e}", parent.display()))
            })?;
        }
        let response = reqwest::Client::new()
            .get(url)
            .header("Referer", "https://flow.google.com/")
            .send()
            .await
            .map_err(|e| VeoError::from_text(&format!("tai file loi mang: {e}")))?;
        if !response.status().is_success() {
            return Err(VeoError::from_text(&format!(
                "tai file HTTP {}",
                response.status().as_u16()
            )));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| VeoError::from_text(&format!("doc noi dung tai ve loi: {e}")))?;
        tokio::fs::write(out_path, &bytes)
            .await
            .map_err(|e| VeoError::from_text(&format!("ghi file {}: {e}", out_path.display())))
    }
}

fn mime_from_path(path: &Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
    {
        Some(ext) if ext == "png" => "image/png".to_string(),
        _ => "image/jpeg".to_string(),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_from_path_theo_duoi_file() {
        assert_eq!(mime_from_path(Path::new("a.png")), "image/png");
        assert_eq!(mime_from_path(Path::new("a.PNG")), "image/png");
        assert_eq!(mime_from_path(Path::new("a.jpg")), "image/jpeg");
        assert_eq!(mime_from_path(Path::new("a")), "image/jpeg");
    }

    #[test]
    fn base64_encode_dung_gia_tri() {
        assert_eq!(base64_encode(b"hi"), "aGk=");
    }
}
