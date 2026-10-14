//! Kiểu "công việc" cho [`crate::MuseaiVideoHost`] — khớp khuôn `lvc_veo3::job`/`lvc_dola::JobsReport`:
//! trait KHÔNG biết đường dẫn file (`local_path` do phía impl — `lvc-engine` — tự quyết theo
//! `run_id`/`node_id`/`scene_index`), chỉ mang dữ liệu cần để gọi RPC.

use std::path::PathBuf;

/// Một lần sinh video — BẤT ĐỐI XỨNG: `MuseaiVideoHost::generate_videos` gửi prompt rồi TỰ chờ
/// (poll `history`) tới khi xong/quá hạn, giống `VeoVideoHost`/`DolaVideoHost`.
#[derive(Debug, Clone)]
pub struct VideoJob {
    /// Chỉ số cảnh 1-based (báo lỗi dùng cho, khớp `scene_index` của Dola/Veo3).
    pub scene_index: u32,
    /// Id credential `museai/login` trong `lvc-vault`.
    pub credential_id: String,
    pub prompt: String,
    /// Tỉ lệ khung hình MONG MUỐN (vd `"9:16"`) — muse.ai KHÔNG có tham số API cho việc này (xác
    /// nhận qua `GET /model`, xem `node-museai-video/CLAUDE.md`), nên chỉ CHÈN THÊM vào câu prompt
    /// làm chỉ thị (xem `protocol::client::build_video_prompt`) — KHÔNG đảm bảo AI tuân theo,
    /// khác hẳn Veo3/Dola nơi đây là tham số API thật. `None`/rỗng = không chèn gì.
    pub aspect: Option<String>,
    /// Thời lượng MONG MUỐN (giây) — cùng cảnh báo như `aspect`, chỉ là chỉ thị trong prompt.
    pub duration_secs: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct VideoJobDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
}

/// Kết quả [`crate::MuseaiVideoHost::generate_videos`] — có thể thành công MỘT PHẦN (`failed` là
/// nhãn cảnh + lý do, KHÔNG làm hỏng cả run).
#[derive(Debug, Default)]
pub struct VideoJobsReport {
    pub done: Vec<VideoJobDone>,
    pub failed: Vec<String>,
}

/// Như [`VideoJob`] nhưng cho ẢNH TĨNH — `prompt` cần tự mô tả rõ muốn ảnh (không phải video), xem
/// doc [`crate::protocol::client::generate_image`].
#[derive(Debug, Clone)]
pub struct ImageJob {
    pub scene_index: u32,
    pub credential_id: String,
    pub prompt: String,
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
