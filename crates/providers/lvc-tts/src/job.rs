//! Kiểu "công việc" cho [`crate::TtsHost`] — khớp khuôn `lvc_museai::job`: trait KHÔNG biết đường
//! dẫn file (`local_path` do phía impl — `lvc-engine` — tự quyết theo `run_id`/`node_id`/
//! `scene_index`), chỉ mang dữ liệu cần để gọi API.

use std::path::PathBuf;

use crate::req::Provider;

/// Một lần sinh audio — mỗi `TextItem` nối vào cổng `prompt` của node `tts` là MỘT job (khớp cách
/// `museai_video`/`grok_image` map nhiều cảnh).
#[derive(Debug, Clone, PartialEq)]
pub struct TtsJob {
    /// Chỉ số cảnh 1-based (báo lỗi dùng cho, khớp `scene_index` của Dola/Veo3/MuseAI).
    pub scene_index: u32,
    /// Tên (label) API key trong `lvc-vault` — engine resolve, KHÔNG phải bí mật.
    pub credential_id: String,
    pub provider: Provider,
    pub text: String,
    pub voice_id: String,
    pub model: String,
    pub speed: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TtsJobDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
}

/// Kết quả [`crate::TtsHost::generate_audios`] — có thể thành công MỘT PHẦN (`failed` là nhãn cảnh
/// + lý do, KHÔNG làm hỏng cả run).
#[derive(Debug, Default)]
pub struct TtsJobsReport {
    pub done: Vec<TtsJobDone>,
    pub failed: Vec<String>,
}
