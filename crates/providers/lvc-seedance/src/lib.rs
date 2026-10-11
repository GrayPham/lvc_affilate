//! `lvc-seedance` — provider API CHÍNH THỨC Seedance/Seedream của `ByteDance` (Volcengine Ark). Tạo
//! ảnh (Seedream `images/generations`) và video (Seedance `contents/generations/tasks` + poll).
//! Người dùng nhập API key Volcengine Ark + (thường) model/endpoint id của chính họ.
//!
//! Kiến trúc (ADR-R014): client HTTP thật ([`client::SeedanceClient`], `reqwest`) + hai host trait
//! [`SeedanceImageHost`]/[`SeedanceVideoHost`] (node lấy qua `ctx.services`, engine impl = resolve
//! key + tự quyết `local_path`). API key CHỈ qua tham số, KHÔNG log.
//!
//! Chia module: [`error`] · [`req`] · [`image`] · [`video`] · [`client`] · [`job`].

pub mod client;
pub mod error;
pub mod image;
pub mod job;
pub mod req;
pub mod video;

use async_trait::async_trait;

pub use client::SeedanceClient;
pub use error::SeedanceError;
pub use req::{Aspect, ImageReq, VideoReq, default_model};

use job::{ImageJob, ImageJobsReport, VideoJob, VideoJobsReport};

/// Bề mặt NODE `seedance_image` (giống `lvc_google::GoogleImageHost`).
#[async_trait]
pub trait SeedanceImageHost: Send + Sync {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport;
}

/// Bề mặt NODE `seedance_video` — submit task + tự poll trong lúc impl chạy.
#[async_trait]
pub trait SeedanceVideoHost: Send + Sync {
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport;
}
