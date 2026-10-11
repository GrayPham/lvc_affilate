//! Payload builders + response extractors cho tung rpcid cua batchexecute. MOT FILE MOT RPCID
//! (module-split-rule) — port thuan tu
//! `Lvc_veo3_automation/src/core/flow_batchexecute.py`. Tat ca THUAN (khong I/O); goi that qua
//! `client::BatchExecuteClient` (Task 6).

pub mod image;
pub mod image_to_video;
pub mod interpolation;
pub mod media;
pub mod reference_to_video;
pub mod status;
pub mod text_to_video;
pub mod upload;

use serde_json::{Value, json};

/// Block reCAPTCHA dung chung anh+video: `[null,22,...,PROJECT,...,[TOKEN,1]]`. Token nhung
/// TRONG day (khong con o clientContext nhu REST cu).
pub(crate) fn recaptcha_block(project_id: &str, recaptcha_token: &str) -> Value {
    json!([
        null,
        22,
        null,
        null,
        null,
        project_id,
        null,
        null,
        null,
        null,
        [recaptcha_token, 1]
    ])
}

/// UUID hoa chu HOA — dang `media_id`/`workflow_id` cURL thuc luon dung.
#[must_use]
pub fn new_uuid() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

/// Slot anh dau/cuoi trong spec i2v/interpolation: `[null, media_id, null,null,null, CROP]`.
/// `CROP` port NGUYEN tu `veo3.js` — vung crop mac dinh, **chua verify bang cURL thuc cua du an
/// nay** (xem `rpc::image_to_video`/`rpc::interpolation`).
pub(crate) fn image_ref_block(media_id: &str) -> Value {
    json!([
        null,
        media_id,
        null,
        null,
        null,
        [null, 0.341_796_875, 1, 0.658_203_125]
    ])
}

/// Duyet de-quy tim URL media (anh `flow-content.google` / video / `usercontent`).
pub(crate) fn find_media_url(node: &Value) -> Option<String> {
    match node {
        Value::String(s)
            if s.starts_with("http")
                && (s.contains("flow-content.google")
                    || s.contains("usercontent")
                    || s.contains(".mp4")) =>
        {
            Some(s.clone())
        }
        Value::Array(items) => items.iter().find_map(find_media_url),
        _ => None,
    }
}

/// Nhu [`find_media_url`] nhung CHI lay URL VIDEO thuc (bo qua `.../image/...` poster) — dung khi
/// doi video I2V/R2V con dang render (tranh tai nham poster).
pub(crate) fn find_video_url(node: &Value) -> Option<String> {
    match node {
        Value::String(s)
            if s.starts_with("http")
                && !s.contains("/image/")
                && (s.contains("/video/") || s.contains(".mp4")) =>
        {
            Some(s.clone())
        }
        Value::Array(items) => items.iter().find_map(find_video_url),
        _ => None,
    }
}

/// Tim `[w,h]` (2 so nguyen hop ly, `1 < n < 8192` de loai timestamp/seed) duyet de-quy.
pub(crate) fn find_dims(node: &Value) -> Option<(i64, i64)> {
    if let Value::Array(items) = node {
        if items.len() == 2 {
            if let (Some(w), Some(h)) = (items[0].as_i64(), items[1].as_i64()) {
                if (2..8192).contains(&w) && (2..8192).contains(&h) {
                    return Some((w, h));
                }
            }
        }
        return items.iter().find_map(find_dims);
    }
    None
}

/// Handle operation video (T2V/R2V/I2V/Interpolation deu tra CUNG HINH DANG — dung 1 extractor).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VideoOperation {
    pub scene_id: Option<String>,
    pub operation_id: Option<String>,
    pub workflow_id: Option<String>,
    pub project_id: Option<String>,
    pub status: Option<String>,
}

/// Rut operation handle tu response T2V/R2V/I2V/Interpolation (da decode qua `wire`). Vi tri mang
/// CO DINH: `payload[2][0]` = op, `op[3]` = detail, `detail[4]`=`operation_id`, `detail[5]`=
/// `workflow_id`, `op[0]`=`scene_id`, `op[-1]`=`project_id`. Nguon: `extract_video_operation`
/// (`flow_batchexecute.py` dong 489-505).
#[must_use]
pub fn extract_video_operation(payload: &Value) -> VideoOperation {
    let op = payload.get(2).and_then(|v| v.get(0));
    let detail = op.and_then(|o| o.get(3));
    let scene_id = op
        .and_then(|o| o.get(0))
        .and_then(Value::as_str)
        .map(str::to_string);
    let operation_id = detail
        .and_then(|d| d.get(4))
        .and_then(Value::as_str)
        .map(str::to_string);
    let workflow_id = detail
        .and_then(|d| d.get(5))
        .and_then(Value::as_str)
        .map(str::to_string);
    let project_id = op
        .and_then(Value::as_array)
        .and_then(|arr| arr.last())
        .and_then(Value::as_str)
        .map(str::to_string);
    let status = payload
        .get(3)
        .filter(|v| !matches!(v, Value::Null) && !matches!(v, Value::Array(a) if a.is_empty()))
        .and_then(|v| v.get(0))
        .and_then(|v| v.get(3))
        .and_then(Value::as_str)
        .map(str::to_string);
    VideoOperation {
        scene_id,
        operation_id,
        workflow_id,
        project_id,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_video_operation_doc_dung_vi_tri() {
        let payload = json!([
            null,
            null,
            [[
                "SCENE-1",
                null,
                null,
                [null, null, null, null, "OP-1", "WF-1"],
                "PROJECT-1"
            ]],
        ]);
        let op = extract_video_operation(&payload);
        assert_eq!(op.scene_id, Some("SCENE-1".to_string()));
        assert_eq!(op.operation_id, Some("OP-1".to_string()));
        assert_eq!(op.workflow_id, Some("WF-1".to_string()));
        assert_eq!(op.project_id, Some("PROJECT-1".to_string()));
        assert_eq!(op.status, None);
    }

    #[test]
    fn extract_video_operation_doc_status_khi_co() {
        let payload = json!([
            null,
            null,
            [[
                "SCENE-1",
                null,
                null,
                [null, null, null, null, "OP-1", "WF-1"],
                "PROJECT-1"
            ]],
            [[null, null, null, "CAE"]],
        ]);
        let op = extract_video_operation(&payload);
        assert_eq!(op.status, Some("CAE".to_string()));
    }

    #[test]
    fn new_uuid_dang_hoa_va_dai_36() {
        let id = new_uuid();
        assert_eq!(id.len(), 36);
        assert_eq!(id, id.to_uppercase());
    }
}
