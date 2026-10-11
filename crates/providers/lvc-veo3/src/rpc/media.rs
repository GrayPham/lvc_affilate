//! Media (`as29s`) — lay 1 media (anh hoac video) theo id. Port thuan tu
//! `flow_batchexecute.py::build_media_payload`/`extract_media` (xac nhan cURL thuc 03/09/2026).
//! Nguon su that DUY NHAT cho "da xong" — trai voi `rpc::status` (`jwpduf`, `status` khong dang
//! tin, luon `'CAE'`).

use serde_json::{Value, json};

use super::{find_dims, find_media_url, find_video_url};

pub const RPC: &str = "as29s";

/// Input `as29s`: video dung `operation_id`, anh khong can (anh dong bo, khong polling).
#[must_use]
pub fn build_payload(media_id: &str) -> Value {
    json!([media_id])
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Media {
    pub media_id: Option<String>,
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub url: Option<String>,
    pub done: bool,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// `done` = co URL hay khong — **KHONG dua vao `status`** (van la `'CAE'` du da xong).
/// `video_only`: bo qua URL `.../image/...` (poster) khi doi video I2V/R2V con dang render —
/// tranh tai nham poster (~180KB) roi coi la "video xong".
#[must_use]
pub fn extract(payload: &Value, video_only: bool) -> Media {
    let media_id = payload.get(0).and_then(Value::as_str).map(str::to_string);
    let project_id = payload.get(1).and_then(Value::as_str).map(str::to_string);
    let status = payload.get(3).and_then(Value::as_str).map(str::to_string);
    let url = if video_only {
        find_video_url(payload)
    } else {
        find_media_url(payload)
    };
    let (width, height) = find_dims(payload).map_or((None, None), |(w, h)| (Some(w), Some(h)));
    Media {
        media_id,
        project_id,
        status,
        done: url.is_some(),
        url,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_done_khi_co_url() {
        let payload = json!([
            "MEDIA-1",
            "PROJECT-1",
            null,
            "CAE",
            "https://flow-content.google/video/abc123?sig=1"
        ]);
        let media = extract(&payload, false);
        assert!(media.done);
        assert_eq!(
            media.url.as_deref(),
            Some("https://flow-content.google/video/abc123?sig=1")
        );
    }

    #[test]
    fn extract_video_only_bo_qua_poster() {
        let payload = json!([
            "MEDIA-1",
            null,
            null,
            "CAE",
            ["https://flow-content.google/image/poster123?sig=1",]
        ]);
        let media = extract(&payload, true);
        assert!(!media.done);
        assert_eq!(media.url, None);
    }

    #[test]
    fn extract_video_only_lay_dung_url_video_khi_co_ca_poster() {
        let payload = json!([
            "MEDIA-1",
            null,
            null,
            "CAE",
            [
                "https://flow-content.google/image/poster123?sig=1",
                "https://flow-content.google/video/real456?sig=1",
            ]
        ]);
        let media = extract(&payload, true);
        assert!(media.done);
        assert_eq!(
            media.url.as_deref(),
            Some("https://flow-content.google/video/real456?sig=1")
        );
    }
}
