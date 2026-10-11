//! Upload (`maseQ`) — upload 1 anh nguoi dung (base64) -> `media_id` dung lam ref. Port thuan tu
//! `flow_batchexecute.py::build_upload_payload`/`extract_upload_media_id` (xac nhan cURL thuc
//! 03/09/2026; hinh dang 12 phan tu port NGUYEN tu `veo3.js::uploadImage` — ban 3-phan-tu cu tra
//! rong, xem MEMORY.md).

use serde_json::{Value, json};

use super::{new_uuid, recaptcha_block};

pub const RPC: &str = "maseQ";

#[must_use]
pub fn build_payload(
    project_id: &str,
    recaptcha_token: &str,
    base64_image: &str,
    mime_type: &str,
    file_name: &str,
    media_ids: &[String],
) -> Value {
    let u1 = media_ids.first().cloned().unwrap_or_else(new_uuid);
    let u2 = media_ids.get(1).cloned().unwrap_or_else(new_uuid);
    json!([
        recaptcha_block(project_id, recaptcha_token),
        base64_image,
        mime_type,
        1,
        null,
        null,
        null,
        null,
        file_name,
        null,
        u1,
        u2,
    ])
}

/// `media_id` anh vua upload — `None` neu hinh dang la (upload loi).
#[must_use]
pub fn extract_media_id(payload: &Value) -> Option<String> {
    payload.get(0)?.get(0)?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_payload_dung_12_phan_tu() {
        let payload = build_payload(
            "P",
            "T",
            "base64data",
            "image/png",
            "anh.png",
            &["U1".into(), "U2".into()],
        );
        assert_eq!(payload.as_array().unwrap().len(), 12);
        assert_eq!(payload[1], "base64data");
        assert_eq!(payload[2], "image/png");
        assert_eq!(payload[8], "anh.png");
        assert_eq!(payload[10], "U1");
        assert_eq!(payload[11], "U2");
    }

    #[test]
    fn extract_media_id_lay_dung_phan_tu() {
        let payload = json!([["MEDIA-1"]]);
        assert_eq!(extract_media_id(&payload), Some("MEDIA-1".to_string()));
    }

    #[test]
    fn extract_media_id_hinh_dang_la_tra_none() {
        assert_eq!(extract_media_id(&Value::Null), None);
        assert_eq!(extract_media_id(&json!([])), None);
    }
}
