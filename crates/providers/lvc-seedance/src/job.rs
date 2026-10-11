//! Kiểu "công việc" cho host trait cấp cao — khớp khuôn `lvc_google::job`/`lvc_veo3::job`:
//! `local_path` do phía impl (`lvc-engine`) tự quyết, không phải provider.

use std::path::PathBuf;

use crate::req::Aspect;

#[derive(Debug, Clone)]
pub struct ImageJob {
    pub scene_index: u32,
    /// TÊN (label) API key trong kho — engine resolve.
    pub credential_name: String,
    pub prompt: String,
    pub model: String,
    pub aspect: Aspect,
}

#[derive(Debug, Clone)]
pub struct ImageJobDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct ImageJobsReport {
    pub done: Vec<ImageJobDone>,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct VideoJob {
    pub scene_index: u32,
    pub credential_name: String,
    pub prompt: String,
    pub model: String,
    pub aspect: Aspect,
    /// Đường dẫn LOCAL ảnh gốc cho i2v — `None` = text-to-video. Impl tự đọc + dựng data URL.
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
