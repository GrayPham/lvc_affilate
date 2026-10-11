//! Lỗi gọi API Google. KHÔNG BAO GIỜ chứa API key — chỉ mã HTTP + mô tả ngắn (đã cắt). Giống khuôn
//! `lvc_genai::TextGenError` để các tầng trên xử lý đồng nhất.

/// Lỗi khi gọi API tạo ảnh/video Google.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GoogleError {
    /// Không lấy được API key từ kho (sai tên credential, bị tắt...). Mô tả AN TOÀN (không chứa key).
    #[error("khong lay duoc API key: {0}")]
    Credential(String),
    /// Tham số yêu cầu không hợp lệ (vd prompt rỗng, `sample_count = 0`).
    #[error("tham so khong hop le: {0}")]
    BadRequest(String),
    /// Xác thực thất bại (HTTP 401/403) — key sai/hết hạn/chưa bật API.
    #[error("xac thuc that bai (HTTP {0})")]
    Auth(u16),
    /// Vượt hạn mức (HTTP 429).
    #[error("vuot han muc (HTTP 429)")]
    Quota,
    /// API trả mã lỗi khác. `detail` là phần đầu thân phản hồi (đã cắt ngắn, không chứa key).
    #[error("API loi HTTP {status}: {detail}")]
    Http { status: u16, detail: String },
    /// Lỗi mạng (đã bỏ URL để không lộ query `?key=`).
    #[error("loi mang: {0}")]
    Network(String),
    /// Phản hồi không đọc được (JSON sai hình dạng / thiếu trường).
    #[error("phan hoi khong doc duoc: {0}")]
    Decode(String),
    /// API trả về nhưng không có nội dung (ảnh/video) nào.
    #[error("API tra ve rong")]
    Empty,
    /// Chờ render video quá hạn.
    #[error("qua han cho video")]
    Timeout,
    /// Google báo operation lỗi (trường `error` trong operation đã done).
    #[error("Google bao loi: {0}")]
    Operation(String),
}
