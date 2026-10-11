//! R2V (`MZZa6b`) — 1-6 anh tham chieu -> video. Port thuan tu
//! `flow_batchexecute.py::build_video_ref_payload` (xac nhan cURL thuc 03/09, 07/09, 08/09/2026).
//! Response cung hinh dang T2V — dung [`super::extract_video_operation`].

use serde_json::{Value, json};

use super::recaptcha_block;

pub const RPC: &str = "MZZa6b";

/// Dung 1 payload R2V (LUON 1 spec, khac T2V khong nhan `seed_count` spec). `refs` = `media_id`
/// anh da upload (`rpc::upload`/`rpc::image`), toi da 6. `media_ids` (2 phan tu, m1/m2) KHONG tu
/// sinh UUID neu thieu (khac T2V) — giu `None` nhu Python goc.
///
/// ⚠️ `seed_count` PHAI = 1 cho model `*_lite`/Omni — dat 2 -> server tra `[]`/`INVALID_ARGUMENT`
/// (xac nhan 08/09, day la nguyen nhan goc loi "No operations" cua ban cu).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_payload(
    prompt: &str,
    model: &str,
    project_id: &str,
    recaptcha_token: &str,
    media_ids: &[String],
    workflow_id: &str,
    ref_media_ids: &[String],
    aspect: i64,
    seed_count: u32,
) -> Value {
    let m1 = media_ids.first().cloned();
    let m2 = media_ids.get(1).cloned();
    let refs: Vec<Value> = ref_media_ids.iter().map(|rid| json!([null, rid])).collect();
    let spec = json!([
        [null, null, [[[prompt]]]],
        refs,
        model,
        aspect,
        null,
        [null, null, null, null, m1, m2],
    ]);
    json!([
        [spec],
        recaptcha_block(project_id, recaptcha_token),
        [workflow_id, seed_count]
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_payload_dung_hinh_dang() {
        let payload = build_payload(
            "mot con cho",
            "veo_3_1_r2v_lite_low_priority",
            "PROJECT-1",
            "TOKEN",
            &["M1".into(), "M2".into()],
            "WF-1",
            &["REF-1".into(), "REF-2".into()],
            2,
            1,
        );
        assert_eq!(payload[0][0][2], "veo_3_1_r2v_lite_low_priority");
        assert_eq!(payload[0][0][3], 2);
        assert_eq!(payload[0][0][1][0][1], "REF-1");
        assert_eq!(payload[0][0][1][1][1], "REF-2");
        assert_eq!(payload[0][0][0][2][0][0][0], "mot con cho");
        assert_eq!(payload[2][0], "WF-1");
        assert_eq!(payload[2][1], 1);
    }

    #[test]
    fn build_payload_thieu_media_id_giu_null_khong_tu_sinh() {
        let payload = build_payload("x", "m", "P", "T", &[], "WF", &[], 2, 1);
        assert!(payload[0][0][5][4].is_null());
        assert!(payload[0][0][5][5].is_null());
    }
}
