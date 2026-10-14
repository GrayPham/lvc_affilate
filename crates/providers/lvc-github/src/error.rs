//! Lỗi của provider GitHub. KHÔNG bao giờ chứa token (xem doc đầu crate) — thân phản hồi bị cắt
//! ngắn và chỉ giữ phần mô tả lỗi, không giữ header.

/// Số ký tự tối đa giữ lại của thân lỗi khi hiển thị/log.
const DETAIL_MAX: usize = 300;

#[derive(Debug, thiserror::Error)]
pub enum GithubError {
    #[error("loi mang: {0}")]
    Network(String),
    /// 401/403 — token sai, hết hạn, hoặc thiếu quyền ghi repo.
    #[error("token khong co quyen (HTTP {status})")]
    Unauthorized { status: u16 },
    #[error("khong tim thay: {what}")]
    NotFound { what: String },
    #[error("GitHub tra HTTP {status}: {detail}")]
    Http { status: u16, detail: String },
    #[error("phan hoi khong doc duoc: {0}")]
    Decode(String),
    #[error("index.json khong phai mang JSON")]
    BadIndex,
}

impl GithubError {
    /// Dựng lỗi HTTP từ status + thân phản hồi, tự cắt ngắn.
    #[must_use]
    pub fn http(status: u16, body: &str) -> Self {
        if status == 401 || status == 403 {
            return Self::Unauthorized { status };
        }
        Self::Http {
            status,
            detail: body.trim().chars().take(DETAIL_MAX).collect(),
        }
    }

    /// Mã ổn định để tầng lệnh ánh xạ sang `IpcError` (giống `AccountError::code`).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Network(_) => "GITHUB_NETWORK",
            Self::Unauthorized { .. } => "GITHUB_UNAUTHORIZED",
            Self::NotFound { .. } => "GITHUB_NOT_FOUND",
            Self::Http { .. } => "GITHUB_HTTP",
            Self::Decode(_) => "GITHUB_DECODE",
            Self::BadIndex => "GITHUB_BAD_INDEX",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_401_403_thanh_unauthorized() {
        assert!(matches!(
            GithubError::http(401, "bad credentials"),
            GithubError::Unauthorized { status: 401 }
        ));
        assert!(matches!(
            GithubError::http(403, "forbidden"),
            GithubError::Unauthorized { status: 403 }
        ));
    }

    #[test]
    fn http_khac_giu_detail_da_cat_ngan() {
        let long = "x".repeat(1000);
        let e = GithubError::http(500, &long);
        match e {
            GithubError::Http { status, detail } => {
                assert_eq!(status, 500);
                assert_eq!(detail.chars().count(), DETAIL_MAX);
            }
            other => panic!("mong Http, nhan {other:?}"),
        }
    }

    #[test]
    fn code_on_dinh_cho_tung_bien_the() {
        assert_eq!(GithubError::Network("x".into()).code(), "GITHUB_NETWORK");
        assert_eq!(
            GithubError::Unauthorized { status: 401 }.code(),
            "GITHUB_UNAUTHORIZED"
        );
        assert_eq!(GithubError::BadIndex.code(), "GITHUB_BAD_INDEX");
    }
}
