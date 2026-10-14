//! `lvc-tts` — provider sinh AUDIO (text-to-speech) dùng chung (`ElevenLabs` / `MiniMax`) cho `node-tts`.
//!
//! Kiến trúc (ADR-R014): provider này chứa CLIENT HTTP thật ([`TtsClient`], dùng `reqwest`) và khai
//! hợp đồng dịch vụ host [`TtsHost`]. Node (`node-tts`) chỉ lấy `Arc<dyn TtsHost>` qua `ctx.services`
//! và truyền TÊN credential + provider trong mỗi [`TtsJob`]; ENGINE impl [`TtsHost`] = resolve API
//! key từ vault (`lvc-engine::tts::TtsSecretResolver`, mirror `textgen::TextSecretResolver`) rồi gọi
//! [`TtsClient::generate_with_key`].
//!
//! Provider KHÔNG biết khái niệm node và KHÔNG giữ bí mật lâu: API key chỉ đi qua tham số hàm
//! [`TtsClient::generate_with_key`], KHÔNG lưu vào struct, KHÔNG log. Lỗi trả về ([`TtsError`])
//! KHÔNG chứa key/text đầy đủ.
//!
//! Chia module theo trách nhiệm (luật repo): [`req`] (Provider + `SynthReq`) · [`job`] (`TtsJob`
//! cho `TtsHost`, mirror `lvc_museai::job`) · [`error`] (lỗi) · [`client`] (client + điều phối) ·
//! [`elevenlabs`]/[`minimax`] (một nhà cung cấp = một file).

mod client;
mod elevenlabs;
mod error;
mod job;
mod minimax;
mod req;

use async_trait::async_trait;

pub use client::TtsClient;
pub use error::TtsError;
pub use job::{TtsJob, TtsJobDone, TtsJobsReport};
pub use req::{Provider, SynthReq};

/// Dịch vụ `TtsHost` do engine cấp: nhận danh sách job, sinh audio TỪNG job (thất bại một phần ->
/// `failed`, không chặn job khác), ghi file, trả đường dẫn. Node (`node-tts`) lấy qua
/// `ctx.services.get::<Arc<dyn TtsHost>>()`. Mirror CHÍNH XÁC
/// `lvc_museai::MuseaiVideoHost::generate_videos`.
#[async_trait]
pub trait TtsHost: Send + Sync {
    async fn generate_audios(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<TtsJob>,
    ) -> TtsJobsReport;
}
