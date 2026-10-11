//! `lvc-google` — provider API CHÍNH THỨC của Google (`generativelanguage.googleapis.com`): tạo ảnh
//! (Imagen 3 `:predict`) và video (Veo `:predictLongRunning` + poll operation). Người dùng nhập API
//! key của chính họ (cùng key với Gemini text — provider vault `"gemini"`).
//!
//! Kiến trúc (ADR-R014): crate này chứa CLIENT HTTP thật ([`client::GoogleClient`], dùng `reqwest`)
//! và khai hai hợp đồng dịch vụ host [`GoogleImageHost`]/[`GoogleVideoHost`]. Node
//! (`node-google-image`/`node-google-video`) chỉ lấy `Arc<dyn GoogleImageHost>` qua `ctx.services`,
//! truyền TÊN credential trong job; ENGINE impl = resolve API key từ vault rồi gọi client + tự quyết
//! `local_path`. API key CHỈ đi qua tham số hàm [`client::GoogleClient`], KHÔNG log.
//!
//! Chia module theo trách nhiệm (luật repo): [`error`] · [`req`] (kiểu yêu cầu THUẦN) · [`image`]
//! (Imagen, một luồng) · [`video`] (Veo, một luồng) · [`client`] (client + điều phối) · [`job`]
//! (kiểu công việc cho host trait).

pub mod client;
pub mod error;
pub mod image;
pub mod job;
pub mod req;
pub mod video;

use async_trait::async_trait;

pub use client::GoogleClient;
pub use error::GoogleError;
pub use req::{Aspect, ImageInput, ImageReq, VideoReq, default_model};

use job::{ImageJob, ImageJobsReport, VideoJob, VideoJobsReport};

/// Bề mặt NODE `google_image` (giống `lvc_veo3::VeoImageHost`) — node KHÔNG biết `GoogleClient`/
/// cách resolve key, chỉ gọi trait này qua `ctx.services`. Impl thực (`lvc-engine`) resolve key +
/// tự quyết `local_path` theo `run_id`/`node_id`/`scene_index`.
#[async_trait]
pub trait GoogleImageHost: Send + Sync {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport;
}

/// Bề mặt NODE `google_video` — xem [`GoogleImageHost`] (cùng khuôn; video submit + tự poll trong
/// lúc impl chạy).
#[async_trait]
pub trait GoogleVideoHost: Send + Sync {
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport;
}
