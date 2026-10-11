//! Provider Google Labs Flow/Veo3 (spec `docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md`
//! v2, ADR-R018/R019). Batchexecute cua `flow.google.com` — KHONG con REST+Bearer
//! `aisandbox-pa.googleapis.com` (Google bo 09/2026). Xem `README.md` cho tong quan, `MEMORY.md`
//! cho quirk/bay da gap khi doc ban Python.
//!
//! LUAT (ADR-R014): KHONG biet khai niem node (khong phu thuoc `lvc-node-api`), KHONG phu thuoc
//! `lvc-engine`/`lvc-store`/`lvc-browser`/`lvc-app`/`tauri`. reqwest CHO PHEP (provider la client
//! dich vu AI, giong `lvc-dola`/`lvc-genai`).

pub mod client;
pub mod error;
pub mod flow;
pub mod job;
pub mod model;
pub mod rpc;
pub mod wire;

use async_trait::async_trait;
use job::{ImageJob, ImageJobsReport, VideoJob, VideoJobsReport};

/// Phien da "harvest" tu 1 tai khoan Google (mo trinh duyet 1 lan, doc `at`/`bl`/`f.sid` +
/// cookie tu `window.WIZ_global_data`, roi dung lai nhieu request HTTP thuan). Xem
/// `lvc_browser::veo3::harvest`.
#[derive(Debug, Clone)]
pub struct FlowSession {
    /// Cookie song cua domain `flow.google.com` (gom `httpOnly`), dung nguyen dang header
    /// `Cookie: k1=v1; k2=v2`.
    pub cookie_header: String,
    /// XSRF token (`window.WIZ_global_data.SNlM0e`).
    pub at: String,
    /// Build label (`window.WIZ_global_data.cfb2h`).
    pub bl: String,
    /// Session id (`window.WIZ_global_data.FdrFJe`).
    pub f_sid: String,
    /// Project id doc tu URL `/project/<id>` cua trang da harvest.
    pub project_id: String,
}

/// Mint token reCAPTCHA bang trinh duyet KHACH an danh (khong cookie tai khoan) — dung chung cho
/// MOI tai khoan, KHONG can credential Google. Impl thuc: `lvc_browser::veo3::captcha`.
#[async_trait]
pub trait VeoCaptchaHost: Send + Sync {
    /// `action`: `"IMAGE_GENERATION"` | `"IMAGE_UPLOAD"` | `"VIDEO_GENERATION"`.
    async fn mint_token(&self, action: &str) -> Result<String, error::VeoError>;
}

/// Harvest phien 1 tai khoan (mo profile Chrome thuc CUA TAI KHOAN 1 lan, doc `at`/`bl`/`fsid` +
/// cookie, roi co the dong trinh duyet). Impl thuc: `lvc_browser::veo3::harvest`. Impl NEN cache
/// theo `credential_id` (giong Python `get_shared_flow_client`) — `Flow` (xem `flow.rs`) goi ham
/// nay o MOI request, tin tuong impl tra nhanh sau lan dau.
#[async_trait]
pub trait VeoSessionHost: Send + Sync {
    /// `credential_id`: id credential `google/login` trong `lvc-vault`.
    async fn harvest(&self, credential_id: &str) -> Result<FlowSession, error::VeoError>;
}

/// Be mat NODE `veo3_image` thay (giong `lvc_dola::DolaVideoHost` cho `dola_video`) — node KHONG
/// biet `Flow`/`VeoCaptchaHost`/`VeoSessionHost`, chi goi trait nay qua `ctx.services`. Impl thuc
/// (`lvc-engine`) ghep [`flow::Flow`] + tu quyet `local_path` theo `run_id`/`node_id`/`scene_index`
/// (xem `job.rs`).
#[async_trait]
pub trait VeoImageHost: Send + Sync {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport;
}

/// Be mat NODE `veo3_video` thay — xem [`VeoImageHost`] (cung khuon, chi khac async: video can
/// submit + tu poll trong luc impl chay).
#[async_trait]
pub trait VeoVideoHost: Send + Sync {
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport;
}
