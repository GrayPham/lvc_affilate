//! Lỗi gọi API Seedance/Seedream. KHÔNG BAO GIỜ chứa API key — chỉ mã HTTP + mô tả ngắn.

/// Lỗi khi gọi API tạo ảnh/video Seedance.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SeedanceError {
    #[error("khong lay duoc API key: {0}")]
    Credential(String),
    #[error("tham so khong hop le: {0}")]
    BadRequest(String),
    #[error("xac thuc that bai (HTTP {0})")]
    Auth(u16),
    #[error("vuot han muc (HTTP 429)")]
    Quota,
    #[error("API loi HTTP {status}: {detail}")]
    Http { status: u16, detail: String },
    #[error("loi mang: {0}")]
    Network(String),
    #[error("phan hoi khong doc duoc: {0}")]
    Decode(String),
    #[error("API tra ve rong")]
    Empty,
    #[error("qua han cho video")]
    Timeout,
    /// Task video bị server báo lỗi (`status = failed`).
    #[error("task that bai: {0}")]
    Task(String),
}
