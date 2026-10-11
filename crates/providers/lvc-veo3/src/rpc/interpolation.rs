//! Interpolation (`nprQif`) — anh dau + anh cuoi -> video noi. Port tu `veo3.js` (docs/ref) —
//! **CHUA CO cURL THUC xac nhan**, xem `MEMORY.md`. Response cung hinh dang T2V — dung
//! [`super::extract_video_operation`].

use serde_json::{Value, json};

use super::{image_ref_block, new_uuid, recaptcha_block};

pub const RPC: &str = "nprQif";

/// **Chua verify bang cURL thuc.** spec = `[promptblock, model, aspect, null, START, END, [ids]]`
/// — START/END o `[4]`/`[5]`, cap `media_id` day xuong `[6]`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_payload(
    prompt: &str,
    model: &str,
    project_id: &str,
    recaptcha_token: &str,
    media_ids: &[String],
    workflow_id: &str,
    start_media_id: &str,
    end_media_id: &str,
    aspect: i64,
    seed_count: u32,
) -> Value {
    let n = seed_count.max(1);
    let specs: Vec<Value> = (0..n)
        .map(|i| {
            let m1 = media_ids
                .get(2 * i as usize)
                .cloned()
                .unwrap_or_else(new_uuid);
            let m2 = media_ids
                .get(2 * i as usize + 1)
                .cloned()
                .unwrap_or_else(new_uuid);
            json!([
                [null, null, [[[prompt]]]],
                model,
                aspect,
                null,
                image_ref_block(start_media_id),
                image_ref_block(end_media_id),
                [null, null, null, null, m1, m2],
            ])
        })
        .collect();
    json!([
        specs,
        recaptcha_block(project_id, recaptcha_token),
        [workflow_id, seed_count]
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_payload_chen_start_va_end_image() {
        let payload = build_payload(
            "x",
            "m",
            "P",
            "T",
            &["M1".into(), "M2".into()],
            "WF",
            "START-1",
            "END-1",
            2,
            1,
        );
        assert_eq!(payload[0][0][4][1], "START-1");
        assert_eq!(payload[0][0][5][1], "END-1");
        assert_eq!(payload[0][0][6][4], "M1");
    }
}
