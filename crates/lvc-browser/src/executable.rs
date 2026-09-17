//! Tìm file chạy trình duyệt theo [`BrowserKind`] (review P1 I-6, ADR-R012): chromiumoxide
//! KHÔNG được tự dò Google Chrome — profile do patchright (Chromium 1234) tạo chỉ mở đúng bằng
//! binary đó (Chrome không giải mã được cookie của Chromium khác), nên phải resolve tường minh và
//! lỗi rõ khi không thấy, thay vì "chạy đại" bằng Chrome đầu tiên tìm được.
//!
//! Thứ tự: `override_` (vd env `LVC_CHROME`) → theo loại:
//! - `Chromium`: `PLAYWRIGHT_BROWSERS_PATH` (nếu đặt) hoặc `%LOCALAPPDATA%\ms-playwright` →
//!   thư mục `chromium-<rev>` có `chrome-win64\chrome.exe`, lấy rev CAO NHẤT; bỏ qua
//!   `chromium_headless_shell-*` (không phải Chrome đầy đủ, UA `HeadlessChrome`).
//! - `Chrome`: `%ProgramFiles%`, `%ProgramFiles(x86)%`, `%LOCALAPPDATA%` +
//!   `Google\Chrome\Application\chrome.exe` (không đọc registry — không thêm crate).
//! - `Opera`: `%LOCALAPPDATA%\Programs\Opera\opera.exe`, `%ProgramFiles%\Opera\opera.exe`.
//!
//! Chỉ `resolve_executable` đọc env; `resolve_executable_in` nhận [`SearchRoots`] tường minh để
//! test không cần Chrome và không đụng env toàn tiến trình.

use std::path::{Path, PathBuf};

use lvc_core::BrowserKind;

use crate::BrowserError;

/// Các thư mục gốc để dò — tách khỏi env để test được bằng tempdir.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchRoots {
    /// Gốc cache Playwright (`PLAYWRIGHT_BROWSERS_PATH` hoặc `%LOCALAPPDATA%\ms-playwright`).
    pub playwright: Option<PathBuf>,
    /// `%ProgramFiles%`.
    pub program_files: Option<PathBuf>,
    /// `%ProgramFiles(x86)%`.
    pub program_files_x86: Option<PathBuf>,
    /// `%LOCALAPPDATA%`.
    pub local_app_data: Option<PathBuf>,
}

impl SearchRoots {
    /// Đọc từ env của tiến trình hiện tại. `PLAYWRIGHT_BROWSERS_PATH` rỗng coi như không đặt.
    #[must_use]
    pub fn from_env() -> Self {
        let env_dir = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let local_app_data = env_dir("LOCALAPPDATA");
        let playwright = env_dir("PLAYWRIGHT_BROWSERS_PATH")
            .or_else(|| local_app_data.as_ref().map(|d| d.join("ms-playwright")));
        Self {
            playwright,
            program_files: env_dir("ProgramFiles"),
            program_files_x86: env_dir("ProgramFiles(x86)"),
            local_app_data,
        }
    }
}

/// Liệt kê `(revision, <root>\chromium-<rev>\chrome-win64\chrome.exe)` cho mọi thư mục
/// `chromium-<số>` dưới `root` CÓ file `chrome.exe`, sắp xếp theo revision TĂNG dần. Thuần
/// (chỉ đọc thư mục), `root` không tồn tại → rỗng. `chromium_headless_shell-*` bị loại vì tiền
/// tố khác (`chromium_` ≠ `chromium-`).
#[must_use]
pub fn ms_playwright_chromium_dirs(root: &Path) -> Vec<(u32, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<(u32, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let rev = name
                .to_str()?
                .strip_prefix("chromium-")?
                .parse::<u32>()
                .ok()?;
            let exe = e.path().join("chrome-win64").join("chrome.exe");
            exe.is_file().then_some((rev, exe))
        })
        .collect();
    found.sort_by_key(|(rev, _)| *rev);
    found
}

/// Danh sách ứng viên (theo thứ tự ưu tiên) cho từng loại — thuần, không stat file.
fn candidates(kind: BrowserKind, roots: &SearchRoots) -> Vec<PathBuf> {
    match kind {
        BrowserKind::Chromium => roots
            .playwright
            .as_deref()
            .map(ms_playwright_chromium_dirs)
            .unwrap_or_default()
            .into_iter()
            .rev() // rev cao nhất trước
            .map(|(_, p)| p)
            .collect(),
        BrowserKind::Chrome => [
            roots.program_files.as_deref(),
            roots.program_files_x86.as_deref(),
            roots.local_app_data.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(|d| {
            d.join("Google")
                .join("Chrome")
                .join("Application")
                .join("chrome.exe")
        })
        .collect(),
        BrowserKind::Opera => [
            roots
                .local_app_data
                .as_deref()
                .map(|d| d.join("Programs").join("Opera").join("opera.exe")),
            roots
                .program_files
                .as_deref()
                .map(|d| d.join("Opera").join("opera.exe")),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

/// Như [`resolve_executable`] nhưng nhận gốc dò tường minh (test không cần env/Chrome).
///
/// # Errors
/// `BrowserError::ExecutableNotFound` — `override_` không tồn tại (khi có override thì KHÔNG dò
/// tiếp, `tried` chỉ chứa nó), hoặc không ứng viên nào tồn tại (`tried` = mọi đường dẫn đã thử;
/// với `Chromium` mà không có thư mục `chromium-*` nào thì `tried` chứa chính gốc Playwright).
pub fn resolve_executable_in(
    kind: BrowserKind,
    override_: Option<&Path>,
    roots: &SearchRoots,
) -> Result<PathBuf, BrowserError> {
    if let Some(p) = override_ {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        return Err(BrowserError::ExecutableNotFound {
            kind,
            tried: vec![p.to_path_buf()],
        });
    }
    let mut tried = candidates(kind, roots);
    if let Some(found) = tried.iter().find(|p| p.is_file()) {
        return Ok(found.clone());
    }
    if tried.is_empty() && kind == BrowserKind::Chromium {
        // Không có thư mục chromium-<rev> nào: báo gốc đã dò để người dùng biết cài ở đâu.
        tried.extend(roots.playwright.clone());
    }
    Err(BrowserError::ExecutableNotFound { kind, tried })
}

/// Tìm file chạy cho `kind`; `override_` (vd env `LVC_CHROME`) thắng mọi thứ nhưng PHẢI tồn tại.
/// Gốc dò lấy từ env ([`SearchRoots::from_env`]).
///
/// # Errors
/// Xem [`resolve_executable_in`].
pub fn resolve_executable(
    kind: BrowserKind,
    override_: Option<&Path>,
) -> Result<PathBuf, BrowserError> {
    resolve_executable_in(kind, override_, &SearchRoots::from_env())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
    }

    fn fake_playwright_root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for d in [
            "chromium-1200",
            "chromium-1234",
            "chromium_headless_shell-1234",
        ] {
            touch(&dir.path().join(d).join("chrome-win64").join("chrome.exe"));
        }
        // thư mục chromium-<rev> KHÔNG có chrome.exe → không được tính
        std::fs::create_dir_all(dir.path().join("chromium-9999")).unwrap();
        dir
    }

    #[test]
    fn ms_playwright_chromium_dirs_tang_dan_va_bo_headless_shell() {
        let dir = fake_playwright_root();
        let got = ms_playwright_chromium_dirs(dir.path());
        let revs: Vec<u32> = got.iter().map(|(r, _)| *r).collect();
        assert_eq!(revs, vec![1200, 1234], "{got:?}");
        for (_, p) in &got {
            assert!(p.ends_with("chrome-win64/chrome.exe"), "{}", p.display());
            assert!(!p.to_string_lossy().contains("headless_shell"));
        }
    }

    #[test]
    fn ms_playwright_chromium_dirs_goc_khong_ton_tai_thi_rong() {
        let dir = tempfile::tempdir().unwrap();
        assert!(ms_playwright_chromium_dirs(&dir.path().join("khong_co")).is_empty());
    }

    #[test]
    fn chromium_lay_rev_cao_nhat() {
        let dir = fake_playwright_root();
        let roots = SearchRoots {
            playwright: Some(dir.path().to_path_buf()),
            ..SearchRoots::default()
        };
        let exe = resolve_executable_in(BrowserKind::Chromium, None, &roots).unwrap();
        assert_eq!(
            exe,
            dir.path()
                .join("chromium-1234")
                .join("chrome-win64")
                .join("chrome.exe")
        );
    }

    #[test]
    fn override_ton_tai_thi_thang_khong_do_tiep() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("my_chrome.exe");
        touch(&exe);
        // roots rỗng hoàn toàn: nếu không dùng override sẽ ExecutableNotFound
        let got = resolve_executable_in(BrowserKind::Chromium, Some(&exe), &SearchRoots::default())
            .unwrap();
        assert_eq!(got, exe);
    }

    #[test]
    fn override_khong_ton_tai_thi_loi_va_tried_chua_no() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("khong_co.exe");
        let pw = fake_playwright_root(); // có Chromium hợp lệ nhưng KHÔNG được dùng thay override
        let roots = SearchRoots {
            playwright: Some(pw.path().to_path_buf()),
            ..SearchRoots::default()
        };
        let err = resolve_executable_in(BrowserKind::Chromium, Some(&missing), &roots).unwrap_err();
        match &err {
            BrowserError::ExecutableNotFound { kind, tried } => {
                assert_eq!(*kind, BrowserKind::Chromium);
                assert_eq!(tried, &vec![missing.clone()]);
            }
            other => panic!("sai bien the: {other:?}"),
        }
        let msg = err.to_string();
        assert!(msg.contains("chromium"), "{msg}");
        assert!(msg.contains("khong_co.exe"), "{msg}");
    }

    #[test]
    fn playwright_root_rong_khong_override_thi_loi_liet_ke_goc() {
        let empty = tempfile::tempdir().unwrap();
        let roots = SearchRoots {
            playwright: Some(empty.path().to_path_buf()),
            ..SearchRoots::default()
        };
        let err = resolve_executable_in(BrowserKind::Chromium, None, &roots).unwrap_err();
        match &err {
            BrowserError::ExecutableNotFound { kind, tried } => {
                assert_eq!(*kind, BrowserKind::Chromium);
                assert_eq!(tried, &vec![empty.path().to_path_buf()]);
            }
            other => panic!("sai bien the: {other:?}"),
        }
    }

    #[test]
    fn chrome_do_program_files_roi_x86_roi_local_app_data() {
        let dir = tempfile::tempdir().unwrap();
        let pf = dir.path().join("PF");
        let pf86 = dir.path().join("PF86");
        let lad = dir.path().join("LAD");
        let roots = SearchRoots {
            playwright: None,
            program_files: Some(pf.clone()),
            program_files_x86: Some(pf86.clone()),
            local_app_data: Some(lad.clone()),
        };
        let rel = Path::new("Google")
            .join("Chrome")
            .join("Application")
            .join("chrome.exe");

        let err = resolve_executable_in(BrowserKind::Chrome, None, &roots).unwrap_err();
        match &err {
            BrowserError::ExecutableNotFound { kind, tried } => {
                assert_eq!(*kind, BrowserKind::Chrome);
                assert_eq!(
                    tried,
                    &vec![pf.join(&rel), pf86.join(&rel), lad.join(&rel)],
                    "thu tu uu tien"
                );
            }
            other => panic!("sai bien the: {other:?}"),
        }

        touch(&lad.join(&rel));
        assert_eq!(
            resolve_executable_in(BrowserKind::Chrome, None, &roots).unwrap(),
            lad.join(&rel)
        );
        touch(&pf.join(&rel));
        assert_eq!(
            resolve_executable_in(BrowserKind::Chrome, None, &roots).unwrap(),
            pf.join(&rel),
            "ProgramFiles thang LOCALAPPDATA"
        );
    }

    #[test]
    fn opera_do_local_app_data_programs_roi_program_files() {
        let dir = tempfile::tempdir().unwrap();
        let pf = dir.path().join("PF");
        let lad = dir.path().join("LAD");
        let roots = SearchRoots {
            playwright: None,
            program_files: Some(pf.clone()),
            program_files_x86: None,
            local_app_data: Some(lad.clone()),
        };
        let err = resolve_executable_in(BrowserKind::Opera, None, &roots).unwrap_err();
        match &err {
            BrowserError::ExecutableNotFound { kind, tried } => {
                assert_eq!(*kind, BrowserKind::Opera);
                assert_eq!(
                    tried,
                    &vec![
                        lad.join("Programs").join("Opera").join("opera.exe"),
                        pf.join("Opera").join("opera.exe"),
                    ]
                );
            }
            other => panic!("sai bien the: {other:?}"),
        }
        let exe = pf.join("Opera").join("opera.exe");
        touch(&exe);
        assert_eq!(
            resolve_executable_in(BrowserKind::Opera, None, &roots).unwrap(),
            exe
        );
    }

    #[test]
    fn roots_rong_hoan_toan_thi_chromium_tried_rong_va_thong_diep_van_ro() {
        let err = resolve_executable_in(BrowserKind::Chromium, None, &SearchRoots::default())
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("chromium"), "{msg}");
        match err {
            BrowserError::ExecutableNotFound { tried, .. } => assert!(tried.is_empty()),
            other => panic!("sai bien the: {other:?}"),
        }
    }
}
