//! `lvc-grok` — provider API CHÍNH THỨC xAI (Grok): tạo ảnh (`/v1/images/generations`,
//! OpenAI-compatible). xAI CHƯA có API video công khai (knowledge cutoff 2026-01) nên crate này chỉ
//! có ảnh — KHÔNG có `GrokVideoHost`/node video Grok (xem plan P6 + MEMORY.md).
//!
//! Kiến trúc (ADR-R014): client HTTP thật ([`client::GrokClient`]) + host trait [`GrokImageHost`]
//! (node lấy qua `ctx.services`, engine impl = resolve key + tự quyết `local_path`). API key CHỈ
//! qua tham số, KHÔNG log. Chia module: [`error`] · [`req`] · [`image`] · [`client`] · [`job`].

pub mod client;
pub mod error;
pub mod image;
pub mod job;
pub mod req;

use async_trait::async_trait;

pub use client::GrokClient;
pub use error::GrokError;
pub use req::{ImageReq, default_model};

use job::{ImageJob, ImageJobsReport};

/// Bề mặt NODE `grok_image` (giống `lvc_google::GoogleImageHost`).
#[async_trait]
pub trait GrokImageHost: Send + Sync {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport;
}
