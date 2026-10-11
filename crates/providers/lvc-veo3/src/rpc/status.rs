//! Status (`jwpduf`) — poll metadata operation. Port thuan tu
//! `flow_batchexecute.py::build_status_payload`/`extract_status` (xac nhan cURL thuc 03/09/2026).
//! ⚠️ `status` LUON `'CAE'` du video da xong — dung [`super::media`] (`as29s`) de quyet dinh
//! "xong", KHONG dung field `status` cua rpc nay.

use serde_json::{Value, json};

use super::find_media_url;

pub const RPC: &str = "jwpduf";

/// Ma trang thai "dang chay/queued" — KHONG doi khi xong (xem doc-comment module).
pub const STATUS_RUNNING: &str = "CAE";

#[must_use]
pub fn build_payload(operation_id: &str) -> Value {
    json!([null, null, [[operation_id]]])
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub operation_id: Option<String>,
    pub project_id: Option<String>,
    pub scene_id: Option<String>,
    pub status: Option<String>,
    pub done: bool,
    pub video_url: Option<String>,
}

#[must_use]
pub fn extract(payload: &Value) -> Status {
    let rec = payload.get(2).and_then(|v| v.get(0));
    let operation_id = rec
        .and_then(|r| r.get(0))
        .and_then(Value::as_str)
        .map(str::to_string);
    let project_id = rec
        .and_then(|r| r.get(1))
        .and_then(Value::as_str)
        .map(str::to_string);
    let scene_id = rec
        .and_then(|r| r.get(2))
        .and_then(Value::as_str)
        .map(str::to_string);
    let status = rec
        .and_then(|r| r.get(3))
        .and_then(Value::as_str)
        .map(str::to_string);
    let done = status.as_deref().is_some_and(|s| s != STATUS_RUNNING);
    let video_url = rec.and_then(find_media_url);
    Status {
        operation_id,
        project_id,
        scene_id,
        status,
        done,
        video_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_dang_chay_status_cae() {
        let payload = json!([null, null, [["OP-1", "PROJECT-1", "SCENE-1", "CAE"]]]);
        let status = extract(&payload);
        assert_eq!(status.status.as_deref(), Some("CAE"));
        assert!(!status.done);
    }

    #[test]
    fn extract_status_khac_cae_coi_la_xong() {
        let payload = json!([null, null, [["OP-1", "PROJECT-1", "SCENE-1", "DONE"]]]);
        let status = extract(&payload);
        assert!(status.done);
    }
}
