//! `fs.*` RPC qua cổng Hatch — port TỪNG PHẦN từ `filesystem.py` (tham khảo, CHỈ ĐỌC): chỉ port
//! các hàm ĐỌC/TẢI (`raw`/`idea_media`/`idea_media_from_url`/`download`) mà
//! `client.rs::generate_video` thực sự dùng để tải video sinh ra — KHÔNG port `stat`/`list`/
//! `rename`/`delete`/`upload_file` (tải lên ảnh tham chiếu, chưa có trong phạm vi v1 của node
//! `museai_video` — xem `node-museai-video/CLAUDE.md` mục "Tham số").

use std::time::Duration;

use super::errors::MuseError;
use super::transport::HatchConnection;
use super::wire::Header;

type Result<T> = std::result::Result<T, MuseError>;

/// Chuẩn hoá đường dẫn phía Hatch — bỏ query/fragment, tiền tố `file://`, gộp `//` thừa, cắt `/`
/// đầu, và nếu có đoạn `workspace` thì cắt mọi thứ TRƯỚC nó (khớp `normalize_gateway_path.py`).
#[must_use]
pub fn normalize_gateway_path(value: &str) -> String {
    let mut text = value.trim();
    if let Some(idx) = text.find(['?', '#']) {
        text = &text[..idx];
    }
    let text = text
        .strip_prefix("file://")
        .unwrap_or(text)
        .replace('\\', "/");
    let mut collapsed = text;
    while collapsed.contains("//") {
        collapsed = collapsed.replace("//", "/");
    }
    let trimmed = collapsed.trim_start_matches('/');
    let mut parts: Vec<&str> = trimmed.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return String::new();
    }
    if let Some(idx) = parts.iter().position(|p| *p == "workspace") {
        parts = parts[idx..].to_vec();
    } else if matches!(parts[0], "home" | "Users") && parts.len() >= 3 {
        parts = parts[2..].to_vec();
    }
    parts.join("/")
}

/// Mã hoá phần trăm (percent-encoding) tối giản cho một đoạn path — chỉ escape ký tự NGOÀI tập
/// "an toàn trong path" (chữ/số + `-_.~/`), đủ cho route `/fs/raw/<path>`/`/api/idea-cards/media/
/// <handle>` (khớp `urllib.parse.quote(..., safe="/")`/`safe=""` bên Python).
fn percent_encode_path(value: &str, keep_slash: bool) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        let allowed_slash = keep_slash && byte == b'/';
        if unreserved || allowed_slash {
            out.push(byte as char);
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

pub struct MuseFilesystem {
    conn: HatchConnection,
}

impl MuseFilesystem {
    #[must_use]
    pub fn new(conn: HatchConnection) -> Self {
        Self { conn }
    }

    /// Tải nguyên văn bytes một file qua `/fs/raw/<path>` — `byte_range` (nếu có) gửi header
    /// `Range: bytes=start-end`.
    ///
    /// # Errors
    /// [`MuseError`] khi `byte_range` không hợp lệ hoặc RPC thất bại/trả lỗi HTTP.
    pub async fn raw(
        &self,
        path: &str,
        byte_range: Option<(u64, u64)>,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        let normalized = normalize_gateway_path(path);
        let route = format!("/fs/raw/{}", percent_encode_path(&normalized, true));
        let mut headers = Vec::new();
        if let Some((start, end)) = byte_range {
            if end < start {
                return Err(MuseError::Protocol(
                    "byte_range khong hop le (end < start)".into(),
                ));
            }
            headers.push(Header {
                key: "Range".into(),
                value: format!("bytes={start}-{end}"),
            });
        }
        let response = self
            .conn
            .request_with_headers("GET", &route, None, headers, timeout)
            .await?
            .raise_for_status()?;
        Ok(response.body)
    }

    /// Tải media qua handle "idea card" (`/api/idea-cards/media/<handle>`).
    ///
    /// # Errors
    /// [`MuseError`] khi RPC thất bại/trả lỗi HTTP.
    pub async fn idea_media(&self, handle: &str, timeout: Duration) -> Result<Vec<u8>> {
        let route = format!(
            "/api/idea-cards/media/{}",
            percent_encode_path(handle, false)
        );
        let response = self
            .conn
            .request("GET", &route, None, timeout)
            .await?
            .raise_for_status()?;
        Ok(response.body)
    }

    /// Như [`MuseFilesystem::idea_media`] nhưng tự tách `handle` từ một URL dạng
    /// `.../idea-cards/media/<handle>` — frontend Muse coi route này là RPC Hatch, KHÔNG phải URL
    /// công khai bình thường (xem ghi chú gốc `download_video.py`).
    ///
    /// # Errors
    /// [`MuseError::Protocol`] khi `value` không chứa đoạn `/idea-cards/media/` hoặc handle rỗng.
    pub async fn idea_media_from_url(&self, value: &str, timeout: Duration) -> Result<Vec<u8>> {
        const MARKER: &str = "/idea-cards/media/";
        let path = value.split('?').next().unwrap_or(value);
        let Some(idx) = path.find(MARKER) else {
            return Err(MuseError::Protocol(
                "khong phai URL idea-cards media".into(),
            ));
        };
        let handle = path[idx + MARKER.len()..].trim_matches('/');
        if handle.is_empty() {
            return Err(MuseError::Protocol("handle idea-cards media rong".into()));
        }
        self.idea_media(handle, timeout).await
    }

    /// Tải `path` rồi ghi ra `destination` (tạo thư mục cha nếu chưa có).
    ///
    /// # Errors
    /// [`MuseError`] khi tải thất bại; lỗi IO khi ghi file thất bại.
    pub async fn download(
        &self,
        path: &str,
        destination: &std::path::Path,
        timeout: Duration,
    ) -> std::result::Result<(), DownloadError> {
        let data = self
            .raw(path, None, timeout)
            .await
            .map_err(DownloadError::Muse)?;
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(DownloadError::Io)?;
        }
        std::fs::write(destination, data).map_err(DownloadError::Io)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error(transparent)]
    Muse(#[from] MuseError),
    #[error("loi ghi file: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_cat_query_fragment_va_file_scheme() {
        assert_eq!(normalize_gateway_path("file:///a/b.mp4?x=1#y"), "a/b.mp4");
    }

    #[test]
    fn normalize_gop_double_slash_va_cat_dau() {
        assert_eq!(normalize_gateway_path("//a//b///c"), "a/b/c");
    }

    #[test]
    fn normalize_cat_truoc_doan_workspace() {
        assert_eq!(
            normalize_gateway_path("/home/user/workspace/imagine_media/x.mp4"),
            "workspace/imagine_media/x.mp4"
        );
    }

    #[test]
    fn normalize_cat_2_doan_dau_cho_home_users() {
        assert_eq!(
            normalize_gateway_path("/home/someuser/file.mp4"),
            "file.mp4"
        );
        assert_eq!(
            normalize_gateway_path("/Users/someuser/file.mp4"),
            "file.mp4"
        );
    }

    #[test]
    fn normalize_rong_tra_rong() {
        assert_eq!(normalize_gateway_path("   "), "");
        assert_eq!(normalize_gateway_path("///"), "");
    }

    #[test]
    fn percent_encode_giu_slash_khi_duoc_yeu_cau() {
        let encoded = percent_encode_path("a b/c#d", true);
        assert_eq!(encoded, "a%20b/c%23d");
    }

    #[test]
    fn percent_encode_khong_giu_slash_khi_khong_yeu_cau() {
        let encoded = percent_encode_path("a/b", false);
        assert_eq!(encoded, "a%2Fb");
    }
}
