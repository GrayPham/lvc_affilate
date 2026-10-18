//! Dò `firefox.exe` cho engine `FirefoxPortable` (xem `lvc_core::BrowserEngine`).
//!
//! Firefox Portable có thể ở BẤT KỲ đâu (bản portable), nên cách đáng tin nhất là người dùng trỏ
//! thẳng: env `LVC_FIREFOX` = đường dẫn `firefox.exe` (vd
//! `...\FirefoxPortable\App\Firefox64\firefox.exe`). Nếu không đặt, thử vài vị trí cài đặt tiêu
//! chuẩn `Mozilla Firefox\firefox.exe`.
//!
//! Tách gốc dò khỏi env (giống `executable.rs`) để test bằng tempdir, không cần Firefox thật.

use std::path::{Path, PathBuf};

use crate::BrowserError;

/// Các thư mục gốc để dò bản cài tiêu chuẩn — tách khỏi env để test được.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FirefoxRoots {
    /// `%ProgramFiles%`.
    pub program_files: Option<PathBuf>,
    /// `%ProgramFiles(x86)%`.
    pub program_files_x86: Option<PathBuf>,
    /// `%LOCALAPPDATA%` (Firefox cài theo người dùng nằm ở `…\Mozilla Firefox`).
    pub local_app_data: Option<PathBuf>,
}

impl FirefoxRoots {
    /// Đọc từ env của tiến trình hiện tại.
    #[must_use]
    pub fn from_env() -> Self {
        let env_dir = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            program_files: env_dir("ProgramFiles"),
            program_files_x86: env_dir("ProgramFiles(x86)"),
            local_app_data: env_dir("LOCALAPPDATA"),
        }
    }
}

/// Ứng viên `firefox.exe` theo thứ tự ưu tiên — thuần, không stat file.
fn firefox_candidates(roots: &FirefoxRoots) -> Vec<PathBuf> {
    [
        roots.program_files.as_deref(),
        roots.program_files_x86.as_deref(),
        roots.local_app_data.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(|d| d.join("Mozilla Firefox").join("firefox.exe"))
    .collect()
}

/// Như [`resolve_firefox`] nhưng nhận gốc dò tường minh (test không cần env/Firefox).
///
/// # Errors
/// [`BrowserError::FirefoxNotFound`] — `override_` không tồn tại (có override thì KHÔNG dò tiếp),
/// hoặc không ứng viên nào tồn tại (`tried` = mọi đường dẫn đã thử).
pub fn resolve_firefox_in(
    override_: Option<&Path>,
    roots: &FirefoxRoots,
) -> Result<PathBuf, BrowserError> {
    if let Some(p) = override_ {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        return Err(BrowserError::FirefoxNotFound {
            tried: vec![p.to_path_buf()],
        });
    }
    let tried = firefox_candidates(roots);
    if let Some(found) = tried.iter().find(|p| p.is_file()) {
        return Ok(found.clone());
    }
    Err(BrowserError::FirefoxNotFound { tried })
}

/// Tìm `firefox.exe`; env `LVC_FIREFOX` (đường dẫn exe) thắng nhưng PHẢI tồn tại.
///
/// # Errors
/// Xem [`resolve_firefox_in`].
pub fn resolve_firefox() -> Result<PathBuf, BrowserError> {
    let override_ = std::env::var_os("LVC_FIREFOX")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    resolve_firefox_in(override_.as_deref(), &FirefoxRoots::from_env())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
    }

    #[test]
    fn override_ton_tai_thi_thang() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("portable_firefox.exe");
        touch(&exe);
        let got = resolve_firefox_in(Some(&exe), &FirefoxRoots::default()).unwrap();
        assert_eq!(got, exe);
    }

    #[test]
    fn override_khong_ton_tai_thi_loi_chi_tried_no() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("khong_co.exe");
        let err = resolve_firefox_in(Some(&missing), &FirefoxRoots::default()).unwrap_err();
        match &err {
            BrowserError::FirefoxNotFound { tried } => assert_eq!(tried, &vec![missing.clone()]),
            other => panic!("sai bien the: {other:?}"),
        }
        assert!(err.to_string().contains("LVC_FIREFOX"));
    }

    #[test]
    fn do_program_files_roi_x86_roi_local_app_data() {
        let dir = tempfile::tempdir().unwrap();
        let pf = dir.path().join("PF");
        let pf86 = dir.path().join("PF86");
        let lad = dir.path().join("LAD");
        let roots = FirefoxRoots {
            program_files: Some(pf.clone()),
            program_files_x86: Some(pf86.clone()),
            local_app_data: Some(lad.clone()),
        };
        let rel = Path::new("Mozilla Firefox").join("firefox.exe");

        let err = resolve_firefox_in(None, &roots).unwrap_err();
        match &err {
            BrowserError::FirefoxNotFound { tried } => assert_eq!(
                tried,
                &vec![pf.join(&rel), pf86.join(&rel), lad.join(&rel)],
                "thu tu uu tien"
            ),
            other => panic!("sai bien the: {other:?}"),
        }

        touch(&lad.join(&rel));
        assert_eq!(resolve_firefox_in(None, &roots).unwrap(), lad.join(&rel));
        touch(&pf.join(&rel));
        assert_eq!(
            resolve_firefox_in(None, &roots).unwrap(),
            pf.join(&rel),
            "ProgramFiles thang LOCALAPPDATA"
        );
    }

    #[test]
    fn roots_rong_thi_loi_tried_rong() {
        let err = resolve_firefox_in(None, &FirefoxRoots::default()).unwrap_err();
        match err {
            BrowserError::FirefoxNotFound { tried } => assert!(tried.is_empty()),
            other => panic!("sai bien the: {other:?}"),
        }
    }
}
