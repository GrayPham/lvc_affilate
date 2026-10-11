//! Khung batchexecute cua `flow.google.com` (BOQ `AiSandboxAngularFrontend`). Port thuan tu
//! `Lvc_veo3_automation/src/core/flow_batchexecute.py` (dong 1-111) — CHI encode/decode, khong
//! I/O. Xem `crates/providers/lvc-veo3/MEMORY.md` cho boi canh day (vi sao khong con Bearer).

use std::collections::BTreeMap;

use serde_json::Value;

/// Endpoint batchexecute duy nhat cho moi rpc (anh + video).
pub const BASE_URL: &str = "https://flow.google.com/_/AiSandboxAngularFrontend/data/batchexecute";

/// Dung URL goi 1 rpc tren `endpoint` (san thuc dung [`BASE_URL`]; test co the doi sang mock
/// server — xem `client::BatchExecuteClient::with_base_url`). `bl`/`f_sid` doc tu trang
/// (`window.WIZ_global_data`), xem `harvest`.
#[must_use]
pub fn build_url(
    endpoint: &str,
    rpcid: &str,
    bl: &str,
    f_sid: &str,
    source_path: &str,
    reqid: u32,
    hl: &str,
) -> String {
    let mut ser = form_urlencoded::Serializer::new(String::new());
    ser.append_pair("rpcids", rpcid)
        .append_pair("source-path", source_path)
        .append_pair("bl", bl)
        .append_pair("f.sid", f_sid)
        .append_pair("hl", hl)
        .append_pair("_reqid", &reqid.to_string())
        .append_pair("rt", "c");
    format!("{endpoint}?{}", ser.finish())
}

/// Body `x-www-form-urlencoded` cua 1 request: `f.req=<outer>&at=<token>&`. `inner` la payload
/// thuc cua rpc (json-hoa thanh CHUOI roi dat vao outer[0][0][1] — long 2 lop, dung y Google).
#[must_use]
pub fn encode_freq(rpcid: &str, inner: &Value, at: &str) -> String {
    let inner_str = serde_json::to_string(inner).unwrap_or_default();
    let outer = serde_json::json!([[[rpcid, inner_str, null, "generic"]]]);
    let outer_str = serde_json::to_string(&outer).unwrap_or_default();
    let mut ser = form_urlencoded::Serializer::new(String::new());
    ser.append_pair("f.req", &outer_str).append_pair("at", at);
    format!("{}&", ser.finish())
}

/// Nguoc cua [`encode_freq`] — chu yeu de test round-trip.
#[must_use]
pub fn decode_freq(body: &str) -> Option<(String, Value, Option<String>)> {
    let mut freq = None;
    let mut at = None;
    for (key, value) in form_urlencoded::parse(body.trim_end_matches('&').as_bytes()) {
        match key.as_ref() {
            "f.req" => freq = Some(value.into_owned()),
            "at" => at = Some(value.into_owned()),
            _ => {}
        }
    }
    let outer: Value = serde_json::from_str(&freq?).ok()?;
    let rpcid = outer[0][0][0].as_str()?.to_string();
    let inner_str = outer[0][0][1].as_str()?;
    let inner: Value = serde_json::from_str(inner_str).ok()?;
    Some((rpcid, inner, at))
}

/// Wire response -> `{rpcid: payload_da_parse}`. Bo XSSI guard `)]}'` + doc cac chunk
/// length-prefixed, chi lay envelope `wrb.fr`. Chunk khong phai JSON hop le -> bo qua (khoan gan
/// nhu Python: moi envelope tu chiu loi rieng, khong huy ca dong vi 1 envelope xau).
#[must_use]
pub fn decode_batchexecute(text: &str) -> BTreeMap<String, Option<Value>> {
    let mut out = BTreeMap::new();
    let trimmed = text.trim_start();
    let body = trimmed
        .strip_prefix(")]}'")
        .map_or(trimmed, |rest| rest.trim_start_matches('\n'));
    let lines: Vec<&str> = body.split('\n').collect();
    let mut i = 0usize;
    while i < lines.len() {
        let marker = lines[i].trim();
        if !marker.is_empty() && marker.chars().all(|c| c.is_ascii_digit()) {
            i += 1;
            if let Some(&chunk) = lines.get(i) {
                if let Ok(Value::Array(envelopes)) = serde_json::from_str::<Value>(chunk) {
                    for env in envelopes {
                        insert_envelope(&mut out, &env);
                    }
                }
            }
        }
        i += 1;
    }
    out
}

fn insert_envelope(out: &mut BTreeMap<String, Option<Value>>, env: &Value) {
    let Some(fields) = env.as_array() else { return };
    if fields.first().and_then(Value::as_str) != Some("wrb.fr") {
        return;
    }
    let Some(rpcid) = fields.get(1).and_then(Value::as_str) else {
        return;
    };
    let payload = match fields.get(2) {
        Some(Value::String(s)) if !s.is_empty() => serde_json::from_str(s).ok(),
        _ => None,
    };
    out.insert(rpcid.to_string(), payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_url_gom_dung_tham_so() {
        let url = build_url(
            BASE_URL,
            "YhhmEf",
            "boq_build",
            "sess-1",
            "/project/abc",
            123_456,
            "vi",
        );
        assert!(url.starts_with(BASE_URL));
        assert!(url.contains("rpcids=YhhmEf"));
        assert!(url.contains("bl=boq_build"));
        assert!(url.contains("f.sid=sess-1"));
        assert!(url.contains("_reqid=123456"));
        assert!(url.contains("rt=c"));
    }

    #[test]
    fn encode_roi_decode_freq_khop_nguoc() {
        let inner = serde_json::json!([["hello"], 1, null]);
        let body = encode_freq("ogiZ0b", &inner, "xsrf-token");
        assert!(body.ends_with('&'));
        let (rpcid, decoded_inner, at) = decode_freq(&body).expect("decode phai thanh cong");
        assert_eq!(rpcid, "ogiZ0b");
        assert_eq!(decoded_inner, inner);
        assert_eq!(at, Some("xsrf-token".to_string()));
    }

    #[test]
    fn decode_batchexecute_doc_dung_1_envelope() {
        let inner = serde_json::json!(["scene-1", null, ["op-1"]]);
        let inner_str = serde_json::to_string(&inner).unwrap();
        let env = serde_json::json!([["wrb.fr", "YhhmEf", inner_str, null, null, null, "generic"]]);
        let chunk = serde_json::to_string(&env).unwrap();
        let wire = format!(")]}}'\n\n{}\n{chunk}\n", chunk.len());
        let out = decode_batchexecute(&wire);
        assert_eq!(out.get("YhhmEf"), Some(&Some(inner)));
    }

    #[test]
    fn decode_batchexecute_bo_qua_chunk_hong_khong_panic() {
        let wire = ")]}'\n\n5\nkhong-phai-json\n";
        let out = decode_batchexecute(wire);
        assert!(out.is_empty());
    }

    #[test]
    fn decode_batchexecute_null_payload_thanh_none() {
        let env = serde_json::json!([["wrb.fr", "as29s", null, null, null, null, "generic"]]);
        let chunk = serde_json::to_string(&env).unwrap();
        let wire = format!(")]}}'\n\n{}\n{chunk}\n", chunk.len());
        let out = decode_batchexecute(&wire);
        assert_eq!(out.get("as29s"), Some(&None));
    }
}
