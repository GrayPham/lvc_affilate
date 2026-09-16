use std::fmt;

/// Mô phỏng `repr()` của Python cho chuỗi: `'x'`; nếu chuỗi có `'` mà không có
/// `"` thì dùng `"x"`; ngược lại escape `\` và `'`.
#[must_use]
pub fn py_repr(s: &str) -> String {
    if s.contains('\'') && !s.contains('"') {
        return format!("\"{s}\"");
    }
    let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

/// Lỗi miền. Thông điệp khớp từng ký tự với bản Python.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error("mode must be fast, pro or pro_max (got {})", py_repr(.0))]
    InvalidMode(String),
    #[error("ratio must be one of 9:16, 16:9, 1:1, 4:3, 3:4 (got {})", py_repr(.0))]
    InvalidRatio(String),
    #[error("model must be 2.0 or 2.5 (got {})", py_repr(.0))]
    InvalidModel(String),
    #[error("logo must be off, crop or delogo (got {})", py_repr(.0))]
    InvalidLogo(String),
    #[error("dur must be 10, 15 or 30 (got {})", py_repr(.0))]
    InvalidDuration(String),
    #[error("prompt is empty")]
    EmptyPrompt,
    #[error("prompt too long ({actual} > {limit} chars)")]
    PromptTooLong { actual: usize, limit: usize },
    #[error("too many reference images (max {0})")]
    TooManyReferenceImages(usize),
    #[error("acc must be an account name (letters, digits, _ -; got {})", py_repr(.0))]
    InvalidAccount(String),
    #[error("too many scenes (max {0})")]
    TooManyScenes(usize),
    #[error("no scenes (text is empty)")]
    NoScenes,
    #[error("expected key=value, got {}", py_repr(.0))]
    ExpectedKeyValue(String),
    #[error("unknown key {} (allowed: ratio, dur, model, mode, logo, refs, acc)", py_repr(.0))]
    UnknownKey(String),
}

/// Lỗi một cảnh kèm số dòng TXT (1-based) hoặc số thứ tự cảnh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneError {
    pub line: Option<usize>,
    pub kind: CoreError,
}

impl SceneError {
    #[must_use]
    pub fn new(line: Option<usize>, kind: CoreError) -> Self {
        Self { line, kind }
    }
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(n) => write!(f, "line {n}: {}", self.kind),
            None => write!(f, "{}", self.kind),
        }
    }
}

impl std::error::Error for SceneError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_error_co_so_dong_thi_hien_line() {
        let e = SceneError::new(Some(3), CoreError::EmptyPrompt);
        assert_eq!(e.to_string(), "line 3: prompt is empty");
    }

    #[test]
    fn scene_error_khong_co_dong_thi_hien_tran() {
        let e = SceneError::new(None, CoreError::EmptyPrompt);
        assert_eq!(e.to_string(), "prompt is empty");
    }

    #[test]
    fn thong_diep_khop_ban_python() {
        assert_eq!(
            CoreError::InvalidMode("xyz".into()).to_string(),
            "mode must be fast, pro or pro_max (got 'xyz')"
        );
        assert_eq!(
            CoreError::InvalidLogo("x".into()).to_string(),
            "logo must be off, crop or delogo (got 'x')"
        );
        assert_eq!(
            CoreError::PromptTooLong {
                actual: 12,
                limit: 10
            }
            .to_string(),
            "prompt too long (12 > 10 chars)"
        );
    }

    #[test]
    fn py_repr_chuoi_thuong_dung_nhay_don() {
        assert_eq!(py_repr("abc"), "'abc'");
    }

    #[test]
    fn py_repr_chuoi_co_nhay_don_thi_doi_sang_nhay_kep() {
        assert_eq!(py_repr("it's"), "\"it's\"");
    }
}
