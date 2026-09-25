//! `lvc-dola` — provider Dola dùng chung cho các node Dola (ảnh, video).
//!
//! P4a chỉ có [`DolaVideoHost`]: dịch vụ "chạy N cảnh video" do host (engine) cung cấp — engine
//! sở hữu tài nguyên dùng chung (pool tài khoản, limiter, bảng `tasks`, trình duyệt) nên node KHÔNG
//! tự làm những việc đó. P4b thêm client API app Android.

// json! lon (body chat/completion ~60 khoa long nhau) can gioi han macro cao hon mac dinh 128.
#![recursion_limit = "512"]

pub mod device;
pub mod error;
pub mod payload;
pub mod response;
pub mod session;
pub mod sign;

use std::path::PathBuf;

use async_trait::async_trait;
use lvc_core::Scene;
use tokio_util::sync::CancellationToken;

/// Một cảnh cần tạo video (`scene_index` 1-based, liên tục trong node).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneJob {
    pub scene_index: u32,
    pub scene: Scene,
}

/// Một video đã tạo xong và tải về.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
    pub account: String,
    pub conversation_id: String,
    pub watermarked: bool,
}

/// Kết quả chạy một nhóm cảnh. `done` sắp theo `scene_index`; `failed` là nhãn cảnh lỗi (số cảnh,
/// hoặc `"panic"`) theo thứ tự phát hiện — dùng cho thông điệp `"node X: canh 1,3 that bai"`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JobsReport {
    pub done: Vec<VideoDone>,
    pub failed: Vec<String>,
}

/// Dịch vụ tạo video Dola do host cấp (`NodeCtx::services`). Một cảnh lỗi KHÔNG dừng cảnh khác;
/// huỷ qua `cancel` (token của run — host tự tạo token con cho từng cảnh).
#[async_trait]
pub trait DolaVideoHost: Send + Sync {
    async fn run_jobs(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<SceneJob>,
        cancel: &CancellationToken,
    ) -> JobsReport;
}
