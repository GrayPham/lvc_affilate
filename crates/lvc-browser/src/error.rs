//! Loi cua lvc-browser (ADR-R002): bao boc loi CDP/I-O/JSON thanh thong diep tieng Viet.

use std::path::PathBuf;

use lvc_core::BrowserKind;

/// Hien thi danh sach duong dan da thu trong `ExecutableNotFound` (rong -> ghi ro ly do).
fn tried_display(tried: &[PathBuf]) -> String {
    if tried.is_empty() {
        return "(khong co duong dan nao de thu - thieu bien moi truong goc)".to_string();
    }
    tried
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    /// Review P1 I-6 / ADR-R012: khong resolve duoc file chay cho `kind` - KHONG de chromiumoxide
    /// tu do Chrome (profile patchright chi mo dung bang Chromium cung ban). `tried` = moi duong
    /// dan da kiem tra theo thu tu (xem `executable.rs`).
    #[error(
        "khong tim thay file chay trinh duyet '{}' - da thu: {}",
        .kind.as_str(),
        tried_display(.tried)
    )]
    ExecutableNotFound {
        kind: BrowserKind,
        tried: Vec<PathBuf>,
    },
    /// Ngoai le JavaScript khi `Runtime.evaluate` (ops.rs): thong diep da rut gon
    /// (`exception.description` hoac `text`) thay vi `Debug` cua ca `ExceptionDetails`.
    #[error("loi JavaScript trong trang: {0}")]
    Js(String),
    #[error("khong khoi dong duoc trinh duyet: {0}")]
    Launch(String),
    /// I-1: `CdpError::LaunchExit` tach duoc ma thoat + stderr co cau truc (khac `Launch`
    /// von chi con mot chuoi `Display` da render san) khi tien trinh Chrome thoat truoc khi
    /// websocket URL duoc doc xong, MA KHONG phai loi `ProfileBusy` (ma 21/24 - xem `ProfileBusy`).
    #[error("trinh duyet thoat truoc khi ket noi CDP (ma thoat {code:?}): {stderr}")]
    LaunchExit { code: Option<i32>, stderr: String },
    #[error("loi CDP: {0}")]
    Cdp(#[from] chromiumoxide::error::CdpError),
    #[error("profile dang bi tien trinh khac dung: {0}")]
    ProfileBusy(String),
    #[error("loi I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("loi JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("dieu huong that bai toi {url}: {reason}")]
    Navigation { url: String, reason: String },
    /// M-6: het thoi gian cho (vd `goto` khong "load" xong trong 60s).
    #[error("qua thoi gian cho: {0}")]
    Timeout(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_not_found_liet_ke_tried_tieng_viet() {
        let e = BrowserError::ExecutableNotFound {
            kind: BrowserKind::Chrome,
            tried: vec![
                PathBuf::from(r"C:\a\chrome.exe"),
                PathBuf::from(r"D:\b\chrome.exe"),
            ],
        };
        let s = e.to_string();
        assert!(s.contains("'chrome'"), "{s}");
        assert!(s.contains(r"C:\a\chrome.exe; D:\b\chrome.exe"), "{s}");
    }

    #[test]
    fn executable_not_found_tried_rong_van_co_thong_diep() {
        let e = BrowserError::ExecutableNotFound {
            kind: BrowserKind::Opera,
            tried: vec![],
        };
        let s = e.to_string();
        assert!(s.contains("'opera'"), "{s}");
        assert!(s.contains("khong co duong dan nao"), "{s}");
    }
}
