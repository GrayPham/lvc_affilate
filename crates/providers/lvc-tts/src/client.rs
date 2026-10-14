//! Client HTTP + điều phối theo provider. Giữ `reqwest::Client` (pool kết nối); API key CHỈ đi qua
//! tham số hàm, KHÔNG lưu vào struct, KHÔNG log.

use crate::error::TtsError;
use crate::req::{Provider, SynthReq};
use crate::{elevenlabs, minimax};

/// Số ký tự tối đa của thân phản hồi lỗi giữ lại trong [`TtsError::Http`] (tránh log dài + lỡ dính
/// dữ liệu nhạy cảm).
const DETAIL_MAX: usize = 300;

/// Client sinh audio dùng chung. Rẻ để `clone` (bọc `reqwest::Client`).
#[derive(Debug, Clone)]
pub struct TtsClient {
    http: reqwest::Client,
}

impl Default for TtsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsClient {
    /// Client mặc định (reqwest với cấu hình chuẩn của workspace).
    #[must_use]
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// Dùng sẵn một `reqwest::Client` (vd caller muốn cắm proxy/timeout).
    #[must_use]
    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Sinh audio với `api_key` cho sẵn (engine đã resolve từ vault). `base_url` `None` = endpoint
    /// chính thức. Trả về bytes audio thô (mp3). KHÔNG log key/text.
    ///
    /// # Errors
    /// [`TtsError::BadRequest`] khi tham số sai; các biến thể khác theo phản hồi API.
    pub async fn generate_with_key(
        &self,
        api_key: &str,
        base_url: Option<&str>,
        req: &SynthReq,
    ) -> Result<Vec<u8>, TtsError> {
        if api_key.trim().is_empty() {
            return Err(TtsError::Credential("API key rong".to_string()));
        }
        if req.text.trim().is_empty() {
            return Err(TtsError::BadRequest("thieu text".to_string()));
        }
        if req.voice_id.trim().is_empty() {
            return Err(TtsError::BadRequest("thieu voice_id".to_string()));
        }
        match req.provider {
            Provider::ElevenLabs => elevenlabs::synth(&self.http, api_key, base_url, req).await,
            Provider::Minimax => minimax::synth(&self.http, api_key, base_url, req).await,
        }
    }
}

/// Ánh xạ mã HTTP loi (KHONG phai 2xx) -> [`TtsError`] (dùng chung `ElevenLabs`/`MiniMax`). `body` chỉ
/// dùng cho `detail` khi lỗi khác 401/403/429 và được CẮT NGẮN — không log nguyên văn. Tách riêng
/// `check_status` (dùng cho phản hồi JSON — xem `minimax.rs`) vì `ElevenLabs` trả BYTES thô lúc
/// thành công nên không thể gọi `resp.text()` trước khi biết status (`elevenlabs.rs` gọi thẳng hàm
/// này sau khi tự kiểm `200..=299`).
pub(crate) fn error_for_status(status: u16, body: &str) -> TtsError {
    match status {
        401 | 403 => TtsError::Auth(status),
        429 => TtsError::Quota,
        s => TtsError::Http {
            status: s,
            detail: clip(body, DETAIL_MAX),
        },
    }
}

/// Ánh xạ mã HTTP -> [`TtsError`] cho phản hồi JSON (dùng chung — xem `minimax.rs`).
///
/// # Errors
/// [`TtsError`] khi `status` không phải 2xx.
pub fn check_status(status: u16, body: &str) -> Result<(), TtsError> {
    if (200..=299).contains(&status) {
        Ok(())
    } else {
        Err(error_for_status(status, body))
    }
}

/// Cắt theo ký tự (không cắt giữa code point) + gỡ khoảng trắng thừa.
fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status_phan_loai_dung() {
        assert!(check_status(200, "").is_ok());
        assert_eq!(check_status(401, "x").unwrap_err(), TtsError::Auth(401));
        assert_eq!(check_status(403, "x").unwrap_err(), TtsError::Auth(403));
        assert_eq!(check_status(429, "x").unwrap_err(), TtsError::Quota);
        match check_status(500, "loi may chu").unwrap_err() {
            TtsError::Http { status, detail } => {
                assert_eq!(status, 500);
                assert_eq!(detail, "loi may chu");
            }
            other => panic!("mong Http, gap {other:?}"),
        }
    }
}
