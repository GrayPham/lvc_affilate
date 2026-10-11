//! Hàm dùng chung cho các dịch vụ host API chính thức: đặt tên file đầu ra, ghi bytes, đọc ảnh gốc
//! thành data URL (cho i2v của provider nhận `image_url`).

use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

/// Tên file đầu ra trong `downloads_dir` — mirror quy ước `<run_id>_<node_id>_<scene_index>.<ext>`
/// của `veo3::EngineVeoJobs`.
pub(super) fn media_out_path(
    downloads_dir: &Path,
    run_id: &str,
    node_id: &str,
    scene_index: u32,
    ext: &str,
) -> PathBuf {
    downloads_dir.join(format!("{run_id}_{node_id}_{scene_index}.{ext}"))
}

/// Đuôi file ảnh theo mime phản hồi (mặc định `png`).
pub(super) fn ext_from_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        _ => "png",
    }
}

/// Đoán mime ảnh từ đuôi file.
pub(super) fn mime_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

/// Ghi bytes ra file (tạo thư mục cha nếu chưa có). Trả `Err(String)` AN TOÀN.
pub(super) async fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| e.to_string())
}

/// Đọc ảnh local → data URL `data:<mime>;base64,<...>` (cho provider nhận `image_url`).
pub(super) async fn read_data_url(path: &Path) -> Result<String, String> {
    let bytes = tokio::fs::read(path).await.map_err(|e| e.to_string())?;
    let mime = mime_from_path(path);
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_mime_dung() {
        assert_eq!(ext_from_mime("image/png"), "png");
        assert_eq!(ext_from_mime("image/jpeg"), "jpg");
        assert_eq!(ext_from_mime(""), "png");
    }

    #[test]
    fn mime_path_dung() {
        assert_eq!(mime_from_path(Path::new("a/b.jpg")), "image/jpeg");
        assert_eq!(mime_from_path(Path::new("a/b.PNG")), "image/png");
    }

    #[test]
    fn out_path_theo_quy_uoc() {
        let p = media_out_path(Path::new("/d"), "run1", "n1", 2, "mp4");
        assert!(p.ends_with("run1_n1_2.mp4"));
    }
}
