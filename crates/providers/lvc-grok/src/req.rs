//! Kiểu yêu cầu THUẦN cho client Grok. xAI image API KHÔNG nhận `size`/`quality`/`style` — chỉ
//! `model`/`prompt`/`n`/`response_format` (xem MEMORY.md), nên không có `Aspect` ở đây.

/// Model mặc định khi tham số `model` rỗng.
pub mod default_model {
    /// Grok image (xAI). Sửa nếu xAI đổi tên — xem MEMORY.md.
    pub const IMAGE: &str = "grok-2-image-1212";
}

/// Yêu cầu tạo ảnh Grok.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReq {
    pub model: String,
    pub prompt: String,
}
