//! Test client HTTP với server giả (wiremock) — KHÔNG bao giờ gọi API Volcengine thật.

use lvc_seedance::{Aspect, ImageReq, SeedanceClient, SeedanceError, VideoReq};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn image_req() -> ImageReq {
    ImageReq {
        model: String::new(),
        prompt: "a cat".into(),
        aspect: Aspect::Square,
    }
}

fn video_req() -> VideoReq {
    VideoReq {
        model: String::new(),
        prompt: "a cat".into(),
        aspect: Aspect::Landscape,
        image_url: None,
    }
}

#[tokio::test]
async fn image_bearer_va_giai_ma_base64() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/images/generations"))
        .and(header("authorization", "Bearer sk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [ { "b64_json": "aGk=" } ]
        })))
        .mount(&server)
        .await;

    let client = SeedanceClient::new();
    let bytes = client
        .generate_image_with_key("sk-test", Some(&server.uri()), &image_req())
        .await
        .unwrap();
    assert_eq!(bytes, b"hi");
}

#[tokio::test]
async fn video_submit_roi_poll_toi_khi_xong() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v3/contents/generations/tasks"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "cgt-1" })),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/contents/generations/tasks/cgt-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "status": "succeeded",
            "content": { "video_url": "https://x/v.mp4" }
        })))
        .mount(&server)
        .await;

    let client = SeedanceClient::new();
    let id = client
        .submit_video_with_key("sk-test", Some(&server.uri()), &video_req())
        .await
        .unwrap();
    assert_eq!(id, "cgt-1");
    let poll = client
        .poll_video_once_with_key("sk-test", Some(&server.uri()), &id)
        .await
        .unwrap();
    assert!(poll.done);
    assert_eq!(poll.url.as_deref(), Some("https://x/v.mp4"));
}

#[tokio::test]
async fn ma_401_thanh_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = SeedanceClient::new();
    let err = client
        .generate_image_with_key("bad", Some(&server.uri()), &image_req())
        .await
        .unwrap_err();
    assert_eq!(err, SeedanceError::Auth(401));
}

#[tokio::test]
async fn prompt_rong_khong_goi_mang() {
    let client = SeedanceClient::new();
    let mut r = image_req();
    r.prompt = "  ".into();
    let err = client
        .generate_image_with_key("k", Some("http://127.0.0.1:1"), &r)
        .await
        .unwrap_err();
    assert!(matches!(err, SeedanceError::BadRequest(_)));
}
