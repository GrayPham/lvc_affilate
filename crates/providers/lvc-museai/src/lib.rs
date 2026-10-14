//! Provider `MuseAI` (`muse.ai`) — port từ `E:\ProjectAILVC\Lvc_veo3_automation\src\core\museai\`
//! (tham khảo, CHỈ ĐỌC). Xem `CLAUDE.md` mục "Cơ chế tạo video/ảnh" cho kiến trúc 3 giai đoạn
//! (đăng nhập tay → bootstrap qua trình duyệt thật → sinh video/ảnh qua API) và `MEMORY.md` cho
//! ranh giới rủi ro/quirk kỹ thuật.
//!
//! Tầng giao thức (`protocol::*`) ĐÃ port xong + test, clippy sạch, cây phụ thuộc không dính
//! `lvc-store`/`lvc-engine`/`tauri`/thư viện giả mạo TLS fingerprint nào:
//! [`protocol::noise`] (bắt tay chuẩn công khai) · [`protocol::wire`] (codec) ·
//! [`protocol::errors`] · [`protocol::transport`] (`HatchConnection` qua WebSocket) ·
//! [`protocol::auth`] (`MuseBootstrap`, thuần) · [`protocol::media`] (trích `VideoRef`/`ImageRef`,
//! thuần) · [`protocol::filesystem`] (tải file) · [`protocol::client`] (`generate_video`/
//! `generate_image` đầu-cuối — ĐÃ XÁC NHẬN qua trang thật 2026-10-12 rằng muse.ai sinh được CẢ
//! video lẫn ảnh tĩnh qua cùng `/chat/stream`, phân biệt bằng ý prompt).
//!
//! CHƯA làm (thuộc về `lvc-engine`, không phải crate này): lấy giá trị thật cho `MuseBootstrap`
//! qua `eval_json` trong trình duyệt đã đăng nhập, impl [`MuseaiVideoHost`]/[`MuseaiImageHost`]
//! nối node với provider này.

mod job;
pub mod protocol;

use async_trait::async_trait;

pub use job::{ImageJob, ImageJobDone, ImageJobsReport, VideoJob, VideoJobDone, VideoJobsReport};

/// Bề mặt NODE thực cho VIDEO (`museai_video` chỉ biết trait này qua `ctx.services`, dùng khuôn
/// `lvc_dola::DolaVideoHost`/`lvc_veo3::VeoVideoHost`) — KHÔNG biết bootstrap/`HatchConnection`
/// tồn tại (chi tiết đó nằm trong impl, thường ở `lvc-engine`).
#[async_trait]
pub trait MuseaiVideoHost: Send + Sync {
    async fn generate_videos(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<VideoJob>,
    ) -> VideoJobsReport;
}

/// Như [`MuseaiVideoHost`] nhưng cho ẢNH TĨNH (`museai_image`).
#[async_trait]
pub trait MuseaiImageHost: Send + Sync {
    async fn generate_images(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<ImageJob>,
    ) -> ImageJobsReport;
}
