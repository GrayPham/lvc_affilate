//! Kiểu "công việc" cho host trait cấp cao ([`crate::GoogleImageHost`]/[`crate::GoogleVideoHost`]) —
//! khớp khuôn `lvc_veo3::job`: trait KHÔNG biết đường dẫn file (`local_path` do phía impl —
//! `lvc-engine` — tự quyết theo `run_id`/`node_id`/`scene_index`), chỉ mang dữ liệu để GỌI API +
//! đường dẫn ảnh gốc cần đọc (i2v).

use std::path::PathBuf;

use crate::req::Aspect;

/// Một lần tạo ảnh.
#[derive(Debug, Clone)]
pub struct ImageJob {
    /// Chỉ số cảnh 1-based (báo lỗi/đặt tên file).
    pub scene_index: u32,
    /// TÊN (label) API key trong kho — engine resolve, node KHÔNG thấy key.
    pub credential_name: String,
    pub prompt: String,
    /// Rỗng = mặc định theo provider (`req::default_model::IMAGE`).
    pub model: String,
    pub aspect: Aspect,
}

/// Một ảnh đã tạo + lưu xong.
#[derive(Debug, Clone)]
pub struct ImageJobDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
}

/// Kết quả nhiều job ảnh — thành công MỘT PHẦN (giống `lvc_veo3`: `failed` là nhãn cảnh + lý do,
/// KHÔNG giết cả run).
#[derive(Debug, Default)]
pub struct ImageJobsReport {
    pub done: Vec<ImageJobDone>,
    /// Mỗi dòng: `"canh <scene_index>: <thông điệp>"`.
    pub failed: Vec<String>,
}

/// Một lần tạo video — bất đối xứng (submit rồi tự poll tới khi xong/quá hạn).
#[derive(Debug, Clone)]
pub struct VideoJob {
    pub scene_index: u32,
    pub credential_name: String,
    pub prompt: String,
    pub model: String,
    pub aspect: Aspect,
    /// Đường dẫn LOCAL ảnh gốc cho i2v — `None` = text-to-video. Impl tự đọc + base64.
    pub ref_image_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct VideoJobDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct VideoJobsReport {
    pub done: Vec<VideoJobDone>,
    pub failed: Vec<String>,
}
