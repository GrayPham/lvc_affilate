//! Giải quyết đường chạy `ffmpeg` — ƯU TIÊN bản ĐI KÈM TOOL (cạnh file thực thi / thư mục con quen
//! thuộc), KHÔNG dựa vào PATH mặc định. Thứ tự:
//! 1. Biến môi trường `LVC_FFMPEG` (override tường minh — dev/CI đặt tay).
//! 2. Bản đi kèm tool: dò cạnh file thực thi hiện tại + vài thư mục con thường gặp (bin/, tools/,
//!    ffmpeg/bin/…). Đây là "chạy theo tool" khi đóng gói (ffmpeg ship cùng app).
//! 3. Cuối cùng mới về `"ffmpeg"` (PATH) như phương án dự phòng.
//!
//! THUẦN (chỉ std) — dùng chung cho mọi node ffmpeg (trim/frame_capture/concat) và lệnh media của app.
#![allow(
    clippy::doc_markdown,
    reason = "doc tiếng Việt nhắc tên đường dẫn/nhị phân, không phải mã"
)]

use std::path::PathBuf;

/// Các vị trí ứng viên (tương đối thư mục chứa file thực thi) cho ffmpeg đi kèm tool.
const BUNDLED_CANDIDATES: &[&str] = &[
    "ffmpeg.exe",
    "ffmpeg",
    "bin/ffmpeg.exe",
    "bin/ffmpeg",
    "tools/ffmpeg.exe",
    "tools/ffmpeg",
    "tools/ffmpeg/ffmpeg.exe",
    "tools/ffmpeg/bin/ffmpeg.exe",
    "ffmpeg/bin/ffmpeg.exe",
    "ffmpeg/bin/ffmpeg",
];

/// Đường chạy ffmpeg theo thứ tự ưu tiên ở trên. Luôn trả về một chuỗi chạy được (ứng viên tìm thấy,
/// hoặc `"ffmpeg"` cuối cùng).
#[must_use]
pub fn ffmpeg_path() -> String {
    resolve(std::env::var("LVC_FFMPEG").ok(), exe_dir())
}

/// Lõi THUẦN (test được, không đụng env/hệ thống): `override_env` = giá trị `LVC_FFMPEG` (nếu có),
/// `exe_dir` = thư mục chứa file thực thi (nếu biết).
fn resolve(override_env: Option<String>, exe_dir: Option<PathBuf>) -> String {
    if let Some(p) = override_env {
        if !p.trim().is_empty() {
            return p;
        }
    }
    if let Some(dir) = exe_dir {
        for cand in BUNDLED_CANDIDATES {
            let p = dir.join(cand);
            if p.is_file() {
                return p.to_string_lossy().into_owned();
            }
        }
    }
    "ffmpeg".to_string()
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_env_thang() {
        assert_eq!(
            resolve(Some("C:/tool/ffmpeg.exe".to_string()), None),
            "C:/tool/ffmpeg.exe"
        );
    }

    #[test]
    fn override_rong_bo_qua_ve_fallback() {
        assert_eq!(resolve(Some("   ".to_string()), None), "ffmpeg");
        assert_eq!(resolve(None, None), "ffmpeg");
    }

    #[test]
    fn co_ung_vien() {
        assert!(!BUNDLED_CANDIDATES.is_empty());
    }
}
