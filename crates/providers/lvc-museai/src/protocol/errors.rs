//! Lỗi của client giao thức Muse — port từ `errors.py` (tham khảo, CHỈ ĐỌC).

#[derive(Debug, thiserror::Error)]
pub enum MuseError {
    #[error("loi xac thuc Muse: {0}")]
    Auth(String),
    #[error("loi giao thuc Muse: {0}")]
    Protocol(String),
    #[error("Hatch RPC loi: {message} (status={status:?})")]
    Rpc {
        message: String,
        status: Option<u16>,
        body: Vec<u8>,
    },
}

impl MuseError {
    /// Lỗi TẠM THỜI của server (nên thử lại): 429 (rate limit) hoặc 5xx (502 Bad Gateway, 503, 504…).
    /// `Rpc` xét theo `status`; `Protocol`/`Auth` (vd lỗi kết nối WebSocket) không có mã nên dò chuỗi
    /// các dấu hiệu 5xx/gateway. Lỗi KHÁC (prompt sai, 4xx trừ 429, lỗi logic) coi là CỐ ĐỊNH —
    /// thử lại vô ích.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Rpc { status, .. } => {
                matches!(status, Some(429 | 500..=599))
            }
            Self::Protocol(s) | Self::Auth(s) => {
                let s = s.to_ascii_lowercase();
                s.contains("502")
                    || s.contains("503")
                    || s.contains("504")
                    || s.contains("500")
                    || s.contains("bad gateway")
                    || s.contains("gateway time")
                    || s.contains("service unavailable")
                    || s.contains("too many requests")
                    || s.contains("429")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MuseError;

    #[test]
    fn transient_nhan_dung_502_5xx_429() {
        assert!(
            MuseError::Rpc {
                message: "x".into(),
                status: Some(502),
                body: vec![]
            }
            .is_transient()
        );
        assert!(
            MuseError::Rpc {
                message: "x".into(),
                status: Some(503),
                body: vec![]
            }
            .is_transient()
        );
        assert!(
            MuseError::Rpc {
                message: "x".into(),
                status: Some(429),
                body: vec![]
            }
            .is_transient()
        );
        assert!(
            MuseError::Protocol(
                "ket noi WebSocket Hatch that bai: HTTP error: 502 Bad Gateway".into()
            )
            .is_transient()
        );
    }

    #[test]
    fn co_dinh_khong_transient() {
        assert!(
            !MuseError::Rpc {
                message: "x".into(),
                status: Some(400),
                body: vec![]
            }
            .is_transient()
        );
        assert!(!MuseError::Protocol("prompt khong hop le".into()).is_transient());
        assert!(
            !MuseError::Rpc {
                message: "x".into(),
                status: None,
                body: vec![]
            }
            .is_transient()
        );
    }
}
