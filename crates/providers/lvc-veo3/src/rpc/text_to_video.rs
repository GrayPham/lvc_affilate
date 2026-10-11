//! T2V (`YhhmEf`) — text-to-video. Port thuan tu
//! `flow_batchexecute.py::build_video_text_payload` (xac nhan cURL thuc 05/09, 23/09/2026).
//! Response cung hinh dang R2V/I2V/Interpolation — dung [`super::extract_video_operation`].

use serde_json::{Value, json};

use super::{new_uuid, recaptcha_block};

pub const RPC: &str = "YhhmEf";

/// Phan tu [1] cua duoi inner (`[workflow_id, X]`). KHONG phai "so video" — X CO DINH = 2 (xac
/// nhan cURL web 23/09: 1 spec + duoi 2 -> Google tra dung 1 operation). Doi X chi khi co cURL
/// web THAT chung minh gia tri khac.
pub const WORKFLOW_TAIL: i64 = 2;

/// Dung 1 payload T2V. `seed_count` = SO SPEC (bien the, khong phai so video) — model
/// `*_lite`/Omni chi nhan **1 spec/lan** (dat 2 -> server tra `[]`). `media_ids` nen co
/// `2 * seed_count` phan tu (thieu -> tu sinh UUID). `aspect`: dung [`crate::model::Aspect::video_enum`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_payload(
    prompt: &str,
    model: &str,
    project_id: &str,
    recaptcha_token: &str,
    media_ids: &[String],
    workflow_id: &str,
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
                [null, null, null, null, m1, m2]
            ])
        })
        .collect();
    json!([
        specs,
        recaptcha_block(project_id, recaptcha_token),
        [workflow_id, WORKFLOW_TAIL]
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_payload_dung_hinh_dang() {
        let payload = build_payload(
            "mot con meo",
            "veo_3_1_t2v_lite_low_priority",
            "PROJECT-1",
            "TOKEN",
            &["M1".into(), "M2".into()],
            "WF-1",
            2,
            1,
        );
        assert_eq!(payload[0][0][1], "veo_3_1_t2v_lite_low_priority");
        assert_eq!(payload[0][0][2], 2);
        assert_eq!(payload[0][0][0][2][0][0][0], "mot con meo");
        assert_eq!(payload[0][0][4][4], "M1");
        assert_eq!(payload[0][0][4][5], "M2");
        assert_eq!(payload[2][0], "WF-1");
        assert_eq!(payload[2][1], WORKFLOW_TAIL);
        assert_eq!(payload[1][5], "PROJECT-1");
        assert_eq!(payload[1][10][0], "TOKEN");
    }

    #[test]
    fn build_payload_thieu_media_id_tu_sinh_uuid() {
        let payload = build_payload("x", "m", "P", "T", &[], "WF", 2, 1);
        let m1 = payload[0][0][4][4].as_str().unwrap();
        assert_eq!(m1.len(), 36);
    }

    #[test]
    fn build_payload_seed_count_2_ra_2_spec() {
        let payload = build_payload("x", "m", "P", "T", &[], "WF", 2, 2);
        assert_eq!(payload[0].as_array().unwrap().len(), 2);
    }
}
