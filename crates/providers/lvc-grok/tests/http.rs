//! Test client HTTP với server giả (wiremock) — KHÔNG bao giờ gọi API xAI thật.

use lvc_grok::{GrokClient, GrokError, ImageReq};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req() -> ImageReq {
    ImageReq {
        model: String::new(),
        prompt: "a cat".into(),
    }
}

#[tokio::test]
async fn image_bearer_va_giai_ma_base64() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/images/generations"))
        .and(header("authorization", "Bearer xai-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [ { "b64_json": "aGk=" } ]
        })))
        .mount(&server)
        .await;

    let client = GrokClient::new();
    let bytes = client
        .generate_image_with_key("xai-test", Some(&server.uri()), &req())
        .await
        .unwrap();
    assert_eq!(bytes, b"hi");
}

#[tokio::test]
async fn ma_401_thanh_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = GrokClient::new();
    let err = client
        .generate_image_with_key("bad", Some(&server.uri()), &req())
        .await
        .unwrap_err();
    assert_eq!(err, GrokError::Auth(401));
}

#[tokio::test]
async fn prompt_rong_khong_goi_mang() {
    let client = GrokClient::new();
    let mut r = req();
    r.prompt = "  ".into();
    let err = client
        .generate_image_with_key("k", Some("http://127.0.0.1:1"), &r)
        .await
        .unwrap_err();
    assert!(matches!(err, GrokError::BadRequest(_)));
}
