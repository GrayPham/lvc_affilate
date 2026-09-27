//! Test orchestration upload + generate qua `wiremock` (KHÔNG gọi Dola thật). `AppSession` trỏ
//! `base_url` về server giả; ký thay bằng `NoSign` (chữ ký thật đã có test vector riêng).

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine as _;
use std::time::Duration;

use lvc_dola::generate::{VideoOpts, generate_video, generate_video_with};
use lvc_dola::session::{AppSession, DeviceIds, RequestSigner, SessionOpts};
use lvc_dola::upload::upload_image;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Ký giả: không tính chữ ký thật (test chỉ kiểm luồng orchestration).
struct NoSign;
impl RequestSigner for NoSign {
    fn sign(
        &self,
        _q: &str,
        _h: &[(String, String)],
        _b: Option<&[u8]>,
        _c: &str,
        _t: u64,
    ) -> Vec<(String, String)> {
        vec![("X-Gorgon".to_string(), "g".to_string())]
    }
}

fn make_session(base_url: &str) -> AppSession {
    let cookies: HashMap<String, String> = [("sessionid", "S1"), ("uid_tt", "U9")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let opts = SessionOpts {
        base_url: base_url.to_string(),
        language: "ja".to_string(),
        signer: Arc::new(NoSign),
    };
    AppSession::new("acc", &cookies, DeviceIds::reference(), None, &opts).unwrap()
}

#[tokio::test]
async fn upload_image_tra_uri_va_resource_id() {
    let server = MockServer::start().await;
    let host = server
        .uri()
        .strip_prefix("http://")
        .unwrap_or_default()
        .to_string();

    // session_token = 4 ký tự đầu (bỏ) + base64(JSON có AccessKeyId).
    let inner = base64::engine::general_purpose::STANDARD.encode(r#"{"AccessKeyId":"AK"}"#);
    let token = format!("sts.{inner}");

    Mock::given(method("POST"))
        .and(path("/alice/resource/prepare_upload"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {"service_id": "svc", "upload_auth_token": {"session_token": token, "secret_key": "sk"}}
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/top/v1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Result": {"UploadAddress": {
                "StoreInfos": [{"Auth": "cdnauth", "StoreUri": "abc123"}],
                "UploadHosts": [host],
            }}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/upload/v1/abc123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"code": 2000})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/alice/resource/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "code": 0, "data": {"abc123": {"resource_id": "RID"}}
        })))
        .mount(&server)
        .await;

    let sess = make_session(&server.uri());
    let out = upload_image(&sess, b"hello-bytes").await.unwrap();
    assert_eq!(out.uri, "abc123");
    assert_eq!(out.resource_id, "RID");
    // md5("hello-bytes") — kiểm đúng giá trị.
    assert_eq!(out.md5, "726457275bf4296d9e495cc6131788cf");
}

#[tokio::test]
async fn generate_video_lay_url_tu_sse() {
    let server = MockServer::start().await;
    let sse = concat!(
        "event: SSE_ACK\n",
        "data: {\"ack_client_meta\":{\"conversation_id\":\"conv1\",\"section_id\":\"s1\"}}\n\n",
        "event: FULL_MSG_NOTIFY\n",
        "data: {\"content\":{\"content_block\":[{\"content\":{\"creation_block\":{\"creations\":[{\"type\":2,\"video\":{\"vid\":\"v1\",\"status\":3,\"download_url\":\"https://cdn/x.mp4\"}}]}}}]}}\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/chat/completion"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse))
        .mount(&server)
        .await;

    let sess = make_session(&server.uri());
    let out = generate_video(
        &sess,
        "con meo nhay",
        &[],
        &VideoOpts::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(out.conversation_id, "conv1");
    assert_eq!(out.video_url, "https://cdn/x.mp4");
    assert!(out.watermarked, "chi co download_url => co watermark");
}

#[tokio::test]
async fn generate_video_lay_url_khong_watermark_tu_poll() {
    let server = MockServer::start().await;
    // SSE chỉ trả conversation_id, chưa có video.
    let sse = concat!(
        "event: SSE_ACK\n",
        "data: {\"ack_client_meta\":{\"conversation_id\":\"conv2\"}}\n\n",
    );
    Mock::given(method("POST"))
        .and(path("/chat/completion"))
        .respond_with(ResponseTemplate::new(200).set_body_string(sse))
        .mount(&server)
        .await;

    let clean = base64::engine::general_purpose::STANDARD.encode("https://cdn/clean.mp4");
    let vm = json!({"video_list": {"a": {"vheight": 720, "vwidth": 404, "main_url": clean}}})
        .to_string();
    let blocks = json!([{"content": {"creation_block": {"creations": [
        {"type": 2, "video": {"vid": "v2", "status": 3, "video_model": vm}}
    ]}}}])
    .to_string();
    let poll = json!({"downlink_body": {"pull_singe_chain_downlink_body": {"messages": [
        {"index_in_conv": "5", "user_type": 2, "content": blocks}
    ]}}});
    Mock::given(method("POST"))
        .and(path("/im/chain/single"))
        .respond_with(ResponseTemplate::new(200).set_body_json(poll))
        .mount(&server)
        .await;

    let sess = make_session(&server.uri());
    let out = generate_video_with(
        &sess,
        "con cho chay",
        &[],
        &VideoOpts::default(),
        &CancellationToken::new(),
        Duration::from_millis(20),
    )
    .await
    .unwrap();
    assert_eq!(out.conversation_id, "conv2");
    assert_eq!(out.video_url, "https://cdn/clean.mp4");
    assert!(!out.watermarked, "co video_model sach => khong watermark");
}
