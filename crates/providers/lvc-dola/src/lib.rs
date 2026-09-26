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

// ---------------------------------------------------------------------------------------------
// Tạo ảnh Dola (Seedream) — KÊNH TRÌNH DUYỆT (browser), KHÔNG ký, KHÔNG giả thiết bị.
//
// QUYẾT ĐỊNH RÀNG BUỘC (giữ nguyên qua mọi thay đổi): node ảnh Dola chỉ chạy qua PHIÊN ĐĂNG NHẬP
// THẬT trên dola.com — trang tự ký request của nó — GIỐNG HỆT cách `DolaVideoHost` chạy video.
// Đường app Android (`Up1080pDoLa/module/dola_image.py` → `dola_client` → `x_signatures/`) dựng lại
// chữ ký chống bot ByteDance (X-Gorgon/X-Argus/X-Ladon) + đăng ký/giả thiết bị: KHÔNG dùng ở đây.
// `sign/` và `device/` trong crate này là stub và phải giữ nguyên stub (xem MEMORY.md).
// ---------------------------------------------------------------------------------------------

/// Một yêu cầu tạo ảnh (một prompt → `count` ảnh cùng model/tỉ lệ). `job_index` 1-based, liên tục
/// trong node (mỗi `TextItem` trên cổng prompt = một job).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageJob {
    pub job_index: u32,
    pub prompt: String,
    /// Nhãn model Seedream đúng như UI Dola hiển thị (vd `"5.0 Pro"`, `"4.5"`); rỗng = để server tự
    /// chọn (chat thường). KHÔNG phải id nội bộ — tránh phụ thuộc chi tiết app-API.
    pub model: String,
    /// Tỉ lệ khung như UI (vd `"1:1"`, `"16:9"`); rỗng = mặc định của Dola.
    pub ratio: String,
    /// Số ảnh muốn lấy (Dola trả tối đa ~4/lần). `0` = mặc định của Dola.
    pub count: u16,
    /// Ảnh tham chiếu đính vào prompt (đường dẫn cục bộ) — upload qua composer như node video.
    pub reference_images: Vec<PathBuf>,
    /// Tài khoản chỉ định (rỗng = host tự chọn từ pool).
    pub account: String,
}

/// Một ảnh đã tạo xong và tải về.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageDone {
    pub job_index: u32,
    /// Thứ tự ảnh trong một job (1-based) — một job có thể trả nhiều ảnh.
    pub image_index: u32,
    pub local_path: PathBuf,
    pub account: String,
    pub conversation_id: String,
}

/// Kết quả chạy một nhóm job ảnh. `done` sắp theo (`job_index`, `image_index`); `failed` là nhãn job
/// lỗi theo thứ tự phát hiện.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageReport {
    pub done: Vec<ImageDone>,
    pub failed: Vec<String>,
}

/// Dịch vụ tạo ảnh Dola (Seedream) do host cấp (`NodeCtx::services`) — cùng khuôn với
/// [`DolaVideoHost`]. Một job lỗi KHÔNG dừng job khác; huỷ qua `cancel`.
#[async_trait]
pub trait DolaImageHost: Send + Sync {
    async fn run_image_jobs(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
        cancel: &CancellationToken,
    ) -> ImageReport;
}
