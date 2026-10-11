//! Test client HTTP với server giả (wiremock) — KHÔNG bao giờ gọi API Google thật.

use lvc_google::{Aspect, GoogleClient, GoogleError, ImageReq, VideoReq};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn image_req() -> ImageReq {
    ImageReq {
        model: String::new(),
        prompt: "a calico cat".into(),
        aspect: Aspect::Portrait,
        sample_count: 1,
    }
}

fn video_req() -> VideoReq {
    VideoReq {
        model: String::new(),
        prompt: "a cat on a roof".into(),
        aspect: Aspect::Landscape,
        sample_count: 1,
        image: None,
    }
}

#[tokio::test]
async fn image_gui_key_qua_header_va_giai_ma_base64() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/v1beta/models/gemini-3.1-flash-image:generateContent",
        ))
        .and(header("x-goog-api-key", "AIza-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "candidates": [ { "content": { "parts": [
                { "inlineData": { "mimeType": "image/png", "data": "aGk=" } }
            ] } } ]
        })))
        .mount(&server)
        .await;

    let client = GoogleClient::new();
    let imgs = client
        .generate_image_with_key("AIza-test", Some(&server.uri()), &image_req())
        .await
        .unwrap();
    assert_eq!(imgs.len(), 1);
    assert_eq!(imgs[0].bytes, b"hi");
}

#[tokio::test]
async fn video_submit_roi_poll_toi_khi_xong() {
    let server = MockServer::start().await;
    // submit -> tra ten operation
    Mock::given(method("POST"))
        .and(path(
            "/v1beta/models/veo-3.1-generate-preview:predictLongRunning",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "models/veo/operations/op1"
        })))
        .mount(&server)
        .await;
    // poll -> done + uri
    Mock::given(method("GET"))
        .and(path("/v1beta/models/veo/operations/op1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "done": true,
            "response": { "generateVideoResponse": { "generatedSamples": [
                { "video": { "uri": "https://x/v.mp4" } }
            ] } }
        })))
        .mount(&server)
        .await;

    let client = GoogleClient::new();
    let name = client
        .submit_video_with_key("AIza-test", Some(&server.uri()), &video_req())
        .await
        .unwrap();
    assert_eq!(name, "models/veo/operations/op1");
    let poll = client
        .poll_video_once_with_key("AIza-test", Some(&server.uri()), &name)
        .await
        .unwrap();
    assert!(poll.done);
    assert_eq!(poll.uri.as_deref(), Some("https://x/v.mp4"));
}

#[tokio::test]
async fn ma_401_thanh_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = GoogleClient::new();
    let err = client
        .generate_image_with_key("bad", Some(&server.uri()), &image_req())
        .await
        .unwrap_err();
    assert_eq!(err, GoogleError::Auth(401));
}

#[tokio::test]
async fn prompt_rong_khong_goi_mang() {
    let client = GoogleClient::new();
    let mut r = image_req();
    r.prompt = "  ".into();
    let err = client
        .generate_image_with_key("k", Some("http://127.0.0.1:1"), &r)
        .await
        .unwrap_err();
    assert!(matches!(err, GoogleError::BadRequest(_)));
}
