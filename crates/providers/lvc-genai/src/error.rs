//! Lỗi sinh text. KHÔNG BAO GIỜ chứa API key hay prompt đầy đủ — chỉ mã HTTP + mô tả ngắn.

/// Lỗi khi gọi API sinh text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TextGenError {
    /// Không lấy được API key từ kho (sai tên credential, bị tắt, lỗi giải mã...). `String` là mô
    /// tả AN TOÀN (không chứa key).
    #[error("khong lay duoc API key: {0}")]
    Credential(String),
    /// Tham số yêu cầu không hợp lệ (vd `num_scenes = 0`, prompt rỗng).
    #[error("tham so khong hop le: {0}")]
    BadRequest(String),
    /// Xác thực thất bại (HTTP 401/403) — key sai/hết hạn.
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
    /// Phản hồi không đọc được (JSON sai hình dạng, thiếu trường text).
    #[error("phan hoi khong doc duoc: {0}")]
    Decode(String),
    /// API trả về nhưng không có nội dung text nào.
    #[error("API tra ve rong")]
    Empty,
}
