//! Body `chat/completion` Rust == Python (vector sinh bởi `tests/vectors/gen_payload_vectors.py`).
//! So sánh NGỮ NGHĨA: mọi chuỗi chứa JSON object/array (template lồng, `ability_param`...) được parse
//! rồi mới so — server parse JSON nên khoảng trắng/escape không quan trọng, cấu trúc + giá trị phải khớp.

use lvc_core::{Model, Ratio};
use lvc_dola::payload::{
    ChatIdentity, ChatRef, Ids, ImageModel, ImageRatio, UploadedImage, asset_attachment_block,
    attachment_block, image_payload, text_payload, video_payload,
};
use serde_json::Value;

struct SeqIds {
    n: u64,
    now_ms: u64,
}

impl Ids for SeqIds {
    fn uuid(&mut self) -> String {
        self.n += 1;
        format!("00000000-0000-4000-8000-{:012}", self.n)
    }

    fn now_ms(&mut self) -> u64 {
        self.now_ms
    }
}

fn normalize(v: &Value) -> Value {
    match v {
        Value::String(s) if s.starts_with('{') || s.starts_with('[') => {
            match serde_json::from_str::<Value>(s) {
                Ok(inner) => normalize(&inner),
                Err(_) => v.clone(),
            }
        }
        Value::Array(a) => Value::Array(a.iter().map(normalize).collect()),
        Value::Object(o) => {
            Value::Object(o.iter().map(|(k, x)| (k.clone(), normalize(x))).collect())
        }
        other => other.clone(),
    }
}

/// Đường dẫn khác biệt đầu tiên (thông điệp lỗi dễ đọc).
#[allow(
    clippy::many_single_char_names,
    reason = "a/b (rust/python) va x/y (cap object dang so) doc ro trong ham so sanh ngan"
)]
fn first_diff(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let p = format!("{path}.{k}");
                match (x.get(k), y.get(k)) {
                    (Some(va), Some(vb)) => {
                        if let Some(d) = first_diff(va, vb, &p) {
                            return Some(d);
                        }
                    }
                    (va, vb) => return Some(format!("{p}: rust={va:?} python={vb:?}")),
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: len rust={} python={}", x.len(), y.len()));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (va, vb))| first_diff(va, vb, &format!("{path}[{i}]")))
        }
        _ if a == b => None,
        _ => Some(format!("{path}: rust={a} python={b}")),
    }
}

fn model(s: &str) -> Model {
    match s {
        "seedance_v2.5" => Model::Seedance25,
        "seedance_v1.0" => Model::Seedance10,
        _ => Model::Seedance20,
    }
}

fn ratio(s: &str) -> Ratio {
    serde_json::from_value(Value::String(s.to_string())).unwrap()
}

#[test]
fn payload_khop_python() {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/payload_vectors.json"
    ))
    .unwrap();
    let doc: Value = serde_json::from_str(&raw).unwrap();
    let who = ChatIdentity {
        bot_id: doc["bot_id"].as_str().unwrap().to_string(),
        uid: doc["uid"].as_str().unwrap().to_string(),
        language: "ja".to_string(),
    };
    let cases = doc["cases"].as_array().unwrap();
    assert!(cases.len() >= 30);
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let a = &c["args"];
        let want = &c["body"];
        let now_ms = want
            .pointer("/option/create_time_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let mut ids = SeqIds { n: 0, now_ms };
        let got = match c["kind"].as_str().unwrap() {
            "video" => {
                let media: Vec<Value> = if a["media"].as_u64() == Some(1) {
                    let info = UploadedImage {
                        uri: "tos-x/abc".into(),
                        md5: "m".repeat(32),
                        width: 100,
                        height: 200,
                        resource_id: String::new(),
                    };
                    vec![attachment_block(&info, &mut ids)]
                } else {
                    vec![]
                };
                let chat = ChatRef {
                    conversation_id: a["conversation_id"].as_str().unwrap().to_string(),
                    ..ChatRef::default()
                };
                video_payload(
                    &who,
                    a["prompt"].as_str().unwrap(),
                    model(a["model"].as_str().unwrap()),
                    ratio(a["ratio"].as_str().unwrap()),
                    u16::try_from(a["duration"].as_u64().unwrap()).unwrap(),
                    &chat,
                    &media,
                    &mut ids,
                )
            }
            "image" => {
                let m: ImageModel = serde_json::from_value(a["model"].clone()).unwrap();
                let r: ImageRatio = serde_json::from_value(a["ratio"].clone()).unwrap();
                image_payload(
                    &who,
                    a["prompt"].as_str().unwrap(),
                    m,
                    r,
                    &ChatRef::default(),
                    &[],
                    &mut ids,
                )
            }
            "text" => {
                let chat = ChatRef {
                    conversation_id: a["conversation_id"].as_str().unwrap().to_string(),
                    last_message_index: a["last_message_index"].as_u64().unwrap(),
                    section_id: a["section_id"].as_str().unwrap().to_string(),
                    local_conv_id: String::new(),
                };
                text_payload(
                    &who,
                    a["text"].as_str().unwrap(),
                    &chat,
                    a["pro"].as_bool().unwrap(),
                    &[],
                    &mut ids,
                )
            }
            "asset" => asset_attachment_block(a["uri"].as_str().unwrap(), "", &mut ids),
            "attach" => attachment_block(
                &UploadedImage {
                    uri: a["uri"].as_str().unwrap().into(),
                    md5: a["md5"].as_str().unwrap().into(),
                    width: 100,
                    height: 200,
                    resource_id: String::new(),
                },
                &mut ids,
            ),
            k => panic!("kind la {k}"),
        };
        let (g, w) = (normalize(&got), normalize(want));
        if let Some(d) = first_diff(&g, &w, name) {
            panic!("lech Python: {d}");
        }
    }
}
