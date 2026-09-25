//! Lỗi kênh API app Dola. Engine ánh xạ sang `DolaError` của pool tài khoản (xoay/phạt nguội) —
//! crate này KHÔNG biết pool.

/// Lỗi kênh API app (port các exception `dola_app.py:175-202` + phân loại chữ bot
/// `video_worker_ui.py:2835-2869`, `:2912-2971`). Thông điệp KHÔNG chứa cookie/chữ ký/proxy có mật khẩu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// Lỗi mạng khi chưa tới Dola (DNS/TLS/kết nối) — gửi lại không tốn credit.
    #[error("API_NETWORK: {0}")]
    Network(String),
    /// Proxy chết ở bước CONNECT (`DolaProxyDeadError`) — bỏ IP, KHÔNG phạt tài khoản.
    #[error("PROXY_DOWN: {0}")]
    ProxyDown(String),
    /// Phiên/cookie hỏng: HTTP 401/403, không có `sessionid`, hoặc chữ bot "đăng nhập".
    #[error("LOGGED_OUT: {0}")]
    LoggedOut(String),
    /// Bot coi phiên là khách (chưa đăng nhập) trên kênh API.
    #[error("GUEST: {0}")]
    Guest(String),
    /// HTTP khác 200.
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    /// `STREAM_ERROR` trong SSE. `code` = mã số Dola (vd `710022002`).
    #[error("API_STREAM_ERROR {code}: {message}")]
    Stream { code: String, message: String },
    /// Hết hạn mức video ngày (chữ bot).
    #[error("ACCOUNT_LIMITED: {0}")]
    Limited(String),
    /// Hết credit/điểm.
    #[error("CREDIT: {0}")]
    Credit(String),
    /// Bot từ chối nội dung (chính sách/chân dung).
    #[error("PROMPT_VIOLATION: {0}")]
    PromptViolation(String),
    /// Không nhận được `conversation_id`.
    #[error("NO_CONVERSATION: {0}")]
    NoConversation(String),
    /// Agent trả lời bằng chữ rồi im, không tạo video.
    #[error("AGENT_NO_VIDEO: {0}")]
    AgentNoVideo(String),
    /// Agent nói "đang tạo" rồi im quá trần.
    #[error("AGENT_STUCK: {0}")]
    AgentStuck(String),
    /// Hết thời gian đợi video/ảnh (Dola vẫn có thể đang tạo — đã tốn credit).
    #[error("TIMEOUT: {0}")]
    Timeout(String),
    /// Người dùng huỷ.
    #[error("cancelled")]
    Cancelled,
    /// Dữ liệu trả về lạ (thiếu trường, JSON hỏng...).
    #[error("PROTOCOL: {0}")]
    Protocol(String),
    /// Lỗi đọc/ghi file cục bộ.
    #[error("IO: {0}")]
    Io(String),
}

impl ApiError {
    /// Mã số Dola trong lỗi (nếu có).
    #[must_use]
    pub fn dola_code(&self) -> Option<&str> {
        match self {
            Self::Stream { code, .. } if !code.is_empty() => Some(code),
            _ => None,
        }
    }

    /// Tài khoản bị Dola gắn cờ (710022002) — đổi tài khoản.
    #[must_use]
    pub fn is_account_blocked(&self) -> bool {
        self.to_string().contains("710022002")
    }

    /// Lỗi tạm 710022013 "system error" — nghỉ ngắn gửi lại.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        let s = self.to_string().to_lowercase();
        !self.is_account_blocked() && (s.contains("710022013") || s.contains("system error"))
    }

    /// Captcha / risk control (710022004).
    #[must_use]
    pub fn is_risk_control(&self) -> bool {
        self.to_string().contains("710022004")
    }
}
