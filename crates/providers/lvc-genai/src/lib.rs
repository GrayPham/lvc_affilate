//! `lvc-genai` — provider sinh TEXT dùng chung giữa các node text (Gemini, `OpenAI`).
//!
//! Kiến trúc (ADR-R014): provider này chứa CLIENT HTTP thật ([`GenAiClient`], dùng `reqwest`) và
//! khai hợp đồng dịch vụ host [`TextGenHost`]. Node (`node-script-gen`) chỉ lấy `Arc<dyn TextGenHost>`
//! qua `ctx.services` và truyền TÊN credential + provider trong [`TextGenReq`]; ENGINE impl
//! [`TextGenHost`] = resolve API key từ vault rồi gọi [`GenAiClient::generate_with_key`].
//!
//! Provider KHÔNG biết khái niệm node và KHÔNG giữ bí mật lâu: API key chỉ đi qua tham số hàm
//! [`GenAiClient::generate_with_key`], KHÔNG lưu vào struct, KHÔNG log. Lỗi trả về ([`TextGenError`])
//! KHÔNG chứa key/prompt đầy đủ.
//!
//! Chia module theo trách nhiệm (luật repo): [`req`] (kiểu yêu cầu) · [`error`] (lỗi) ·
//! [`prompt`] (dựng system/user + tách cảnh, THUẦN) · [`client`] (client + điều phối) ·
//! [`gemini`]/[`openai`] (một nhà cung cấp = một file).

mod client;
mod error;
mod gemini;
mod openai;
mod prompt;
mod req;

use async_trait::async_trait;

pub use client::GenAiClient;
pub use error::TextGenError;
pub use prompt::split_scenes;
pub use req::{Provider, TextGenReq};

/// Dịch vụ sinh text do host (engine) cấp cho node qua `NodeCtx::services`. Node khai kiểu
/// `Arc<dyn TextGenHost>` mà không phụ thuộc engine; engine impl (resolve API key từ vault rồi gọi
/// [`GenAiClient`]). Bí mật KHÔNG bao giờ nằm trong [`TextGenReq`] — chỉ có TÊN credential.
#[async_trait]
pub trait TextGenHost: Send + Sync {
    /// Sinh kịch bản nhiều cảnh theo `req`; trả về văn bản thô (mỗi cảnh một dòng — node tự tách
    /// bằng [`split_scenes`]).
    ///
    /// # Errors
    /// [`TextGenError`] khi thiếu/không lấy được API key, tham số sai, API lỗi, hoặc phản hồi rỗng.
    async fn generate(&self, req: TextGenReq) -> Result<String, TextGenError>;
}
