//! Lỗi gọi registry. Không có bí mật nào liên quan (endpoint công khai, không token) nên không cần
//! che giấu nội dung lỗi như các provider có API key.

/// Lỗi khi fetch/giải mã `index.json`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RegistryError {
    /// Lỗi mạng (timeout, DNS, mất kết nối…).
    #[error("loi mang: {0}")]
    Network(String),
    /// Server trả mã không phải 2xx.
    #[error("registry tra loi HTTP {status}: {detail}")]
    Http { status: u16, detail: String },
    /// Thân phản hồi không phải JSON hợp lệ theo hình dạng mong đợi.
    #[error("khong doc duoc index.json: {0}")]
    Decode(String),
}
