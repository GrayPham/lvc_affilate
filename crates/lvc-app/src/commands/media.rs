//! Lệnh media phụ trợ cho UI — hiện chỉ có [`frame_save_png`]: nhận PNG (base64) do UI chụp từ thẻ
//! `<video>` (canvas) rồi ghi thành FILE thật để dùng như một ảnh trên canvas (node ảnh cần đường
//! dẫn file, không dùng data URL). Ghi vào thư mục tạm `lvc_frames` của hệ điều hành. Không cần
//! `AppState` (thuần I/O tệp), không tài khoản.

use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::IpcError;

/// Tên file an toàn từ `hint` (chỉ giữ chữ/số/`._-`), rỗng thì dùng `frame`.
fn safe_stem(hint: &str) -> String {
    let s: String = hint
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "frame".to_string() } else { s }
}

/// Thư mục chứa khung hình đã chụp (tạo nếu chưa có).
fn frames_dir() -> PathBuf {
    std::env::temp_dir().join("lvc_frames")
}

/// Giải mã PNG base64 -> ghi file trong `lvc_frames`, trả ĐƯỜNG DẪN tuyệt đối. `data` có thể kèm
/// tiền tố `data:image/png;base64,` (sẽ tự cắt). `name_hint` (tuỳ chọn) làm phần đầu tên file.
///
/// # Errors
/// `FRAME_DECODE` (base64 sai), `FRAME_WRITE` (không tạo được thư mục/ghi file).
#[tauri::command]
pub async fn frame_save_png(data: String, name_hint: Option<String>) -> Result<String, IpcError> {
    let b64 = data.rsplit(',').next().unwrap_or(&data);
    let bytes = STANDARD
        .decode(b64.trim())
        .map_err(|e| IpcError::new("FRAME_DECODE", format!("PNG base64 sai: {e}")))?;
    let dir = frames_dir();
    let stem = safe_stem(name_hint.as_deref().unwrap_or("frame"));
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let path = dir.join(format!("{stem}_{ts}.png"));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| IpcError::new("FRAME_WRITE", format!("tạo thư mục lỗi: {e}")))?;
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| IpcError::new("FRAME_WRITE", format!("ghi file lỗi: {e}")))?;
    Ok(path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------------------------
// Cắt video tương tác (trim) — UI chọn đoạn [start, end] trên timeline rồi xác nhận; ghi file mới.
// ---------------------------------------------------------------------------------------------

fn ffmpeg_bin() -> String {
    std::env::var("LVC_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string())
}

/// `[start, end]` -> tham số thời gian ffmpeg (đặt SAU `-i`). `end <= start` -> chỉ `-ss`.
fn time_args(start: f64, end: f64) -> Vec<String> {
    let mut v = Vec::new();
    if start > 0.0 {
        v.push("-ss".to_string());
        v.push(format!("{start}"));
    }
    if end > start {
        v.push("-to".to_string());
        v.push(format!("{end}"));
    }
    v
}

async fn run_ffmpeg(args: &[&str]) -> Result<(), String> {
    let out = tokio::process::Command::new(ffmpeg_bin())
        .args(args)
        .output()
        .await
        .map_err(|e| format!("khong chay duoc ffmpeg: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    Err(err.lines().rev().take(3).collect::<Vec<_>>().join(" | "))
}

/// Cắt `path` theo `[start, end]` giây -> file MP4 mới trong `lvc_trims`, trả đường dẫn. Thử `-c copy`
/// (nhanh) trước, lỗi thì re-encode. `end <= start` (hoặc 0) = tới hết video.
///
/// # Errors
/// `TRIM_INPUT` (thiếu file vào), `TRIM_FAIL` (ffmpeg lỗi / không ghi được).
#[tauri::command]
pub async fn trim_video(
    path: String,
    start: f64,
    end: f64,
    name_hint: Option<String>,
) -> Result<String, IpcError> {
    if tokio::fs::metadata(&path).await.is_err() {
        return Err(IpcError::new(
            "TRIM_INPUT",
            format!("khong tim thay video: {path}"),
        ));
    }
    let dir = std::env::temp_dir().join("lvc_trims");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| IpcError::new("TRIM_FAIL", format!("tao thu muc loi: {e}")))?;
    let stem = safe_stem(name_hint.as_deref().unwrap_or("trim"));
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let out = dir.join(format!("{stem}_{ts}.mp4"));
    let out_s = out.to_string_lossy().to_string();
    let t = time_args(start, end);
    let t_ref: Vec<&str> = t.iter().map(String::as_str).collect();

    let mut copy_args = vec!["-y", "-i", &path];
    copy_args.extend_from_slice(&t_ref);
    copy_args.extend_from_slice(&["-c", "copy", &out_s]);
    if run_ffmpeg(&copy_args).await.is_ok() {
        return Ok(out_s);
    }
    let mut enc = vec!["-y", "-i", &path];
    enc.extend_from_slice(&t_ref);
    enc.extend_from_slice(&[
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        &out_s,
    ]);
    run_ffmpeg(&enc)
        .await
        .map_err(|e| IpcError::new("TRIM_FAIL", format!("cat video loi: {e}")))?;
    Ok(out_s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_args_dung() {
        assert_eq!(time_args(0.0, 0.0), Vec::<String>::new());
        assert_eq!(time_args(0.0, 5.0), vec!["-to", "5"]);
        assert_eq!(time_args(2.0, 7.0), vec!["-ss", "2", "-to", "7"]);
        assert_eq!(time_args(5.0, 3.0), vec!["-ss", "5"]);
    }

    #[test]
    fn safe_stem_loc_ky_tu() {
        assert_eq!(safe_stem("Frame 00:15.08s"), "Frame_00_15.08s");
        assert_eq!(safe_stem(""), "frame");
        assert_eq!(safe_stem("___"), "frame");
        assert_eq!(safe_stem("a/b\\c"), "a_b_c");
    }

    #[tokio::test]
    async fn frame_save_png_ghi_duoc_file() {
        // PNG 1x1 trong suốt (base64), có tiền tố data URL.
        let data = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M8AAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
        let path = frame_save_png(data.to_string(), Some("test frame".into()))
            .await
            .unwrap();
        assert!(
            std::path::Path::new(&path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png")),
            "{path}"
        );
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 0);
        let _ = std::fs::remove_file(&path);
    }
}
