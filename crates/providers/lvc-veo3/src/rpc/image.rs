//! Anh (`ogiZ0b`) — tao anh `GEM_PIX_2`/Nano Banana. Port thuan tu
//! `flow_batchexecute.py::build_image_payload`/`extract_image_result` (xac nhan cURL thuc 03/09,
//! 09/09/2026). **Dong bo** — response tra URL ngay, khong polling.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use super::{find_dims, find_media_url, recaptcha_block};

pub const RPC: &str = "ogiZ0b";

/// Dung 1 payload tao anh. `ref_media_ids`: `media_id` anh tham chieu DA UPLOAD VAO DUNG PROJECT
/// hien tai (`rpc::upload`) — doi project phai upload lai. Block reCAPTCHA xuat hien 2 LAN (trong
/// spec + o ngoai) — dung y Google, khong phai loi lap.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_payload(
    prompt: &str,
    project_id: &str,
    recaptcha_token: &str,
    media_ids: &[String],
    workflow_id: &str,
    model: &str,
    seed: i64,
    aspect: i64,
    ref_media_ids: &[String],
) -> Value {
    let m1 = media_ids.first().cloned();
    let m2 = media_ids.get(1).cloned();
    let recap = recaptcha_block(project_id, recaptcha_token);
    let refs: Vec<Value> = ref_media_ids
        .iter()
        .filter(|rid| !rid.is_empty())
        .map(|rid| json!([rid, null, null, null, 1]))
        .collect();
    let refs_value = if refs.is_empty() {
        Value::Null
    } else {
        Value::Array(refs)
    };
    let spec = json!([
        null,
        null,
        refs_value,
        seed,
        aspect,
        model,
        null,
        recap,
        [[[prompt]]],
        null,
        null,
        null,
        m1,
        m2,
    ]);
    json!([null, [spec], 1, recap, [workflow_id]])
}

static MEDIA_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/image/([0-9a-fA-F-]{8,})").unwrap());

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageResult {
    pub url: Option<String>,
    pub media_id: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// Duyet de-quy (khong phu thuoc index mong manh cua response) — anh dong bo nen URL co ngay.
#[must_use]
pub fn extract(payload: &Value) -> ImageResult {
    let url = find_media_url(payload);
    let media_id = url
        .as_deref()
        .and_then(|u| MEDIA_ID_RE.captures(u))
        .map(|caps| caps[1].to_string());
    let (width, height) = find_dims(payload).map_or((None, None), |(w, h)| (Some(w), Some(h)));
    ImageResult {
        url,
        media_id,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_payload_khong_ref_thi_refs_la_null() {
        let payload = build_payload(
            "mot con meo",
            "P",
            "T",
            &["M1".into(), "M2".into()],
            "WF",
            "GEM_PIX_2",
            42,
            3,
            &[],
        );
        assert!(payload[1][0][2].is_null());
        assert_eq!(payload[1][0][5], "GEM_PIX_2");
        assert_eq!(payload[1][0][4], 3);
        assert_eq!(payload[1][0][3], 42);
    }

    #[test]
    fn build_payload_co_ref_thi_gan_dung_hinh_dang() {
        let payload = build_payload(
            "x",
            "P",
            "T",
            &[],
            "WF",
            "GEM_PIX_2",
            1,
            2,
            &["REF-1".into()],
        );
        assert_eq!(payload[1][0][2][0][0], "REF-1");
        assert_eq!(payload[1][0][2][0][4], 1);
    }

    #[test]
    fn extract_lay_url_va_media_id() {
        let payload = json!([[["https://flow-content.google/image/deadbeef-1234-5678?sig=1",]]]);
        let result = extract(&payload);
        assert_eq!(result.media_id.as_deref(), Some("deadbeef-1234-5678"));
        assert!(result.url.unwrap().contains("flow-content.google"));
    }
}
