//! Test client HTTP với server giả (wiremock) — KHÔNG bao giờ gọi API AI thật.

use lvc_genai::{GenAiClient, Provider, TextGenError, TextGenReq};
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req(provider: Provider) -> TextGenReq {
    TextGenReq {
        credential_name: "k".into(),
        provider,
        system: String::new(),
        prompt: "giay chay bo".into(),
        num_scenes: 2,
        language: "vi".into(),
        style: String::new(),
    }
}

#[tokio::test]
async fn gemini_gui_key_qua_query_va_rut_text() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1beta/models/gemini-2.0-flash:generateContent"))
        .and(query_param("key", "AIza-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "candidates": [ { "content": { "parts": [ { "text": "canh 1\ncanh 2" } ] } } ]
        })))
        .mount(&server)
        .await;

    let client = GenAiClient::new();
    let out = client
        .generate_with_key("AIza-test", Some(&server.uri()), &req(Provider::Gemini))
        .await
        .unwrap();
    assert_eq!(out, "canh 1\ncanh 2");
}

#[tokio::test]
async fn openai_gui_key_qua_bearer_va_rut_content() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer sk-test"))
        .and(body_string_contains("gpt-4o-mini"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [ { "message": { "content": "canh 1\ncanh 2" } } ]
        })))
        .mount(&server)
        .await;

    let client = GenAiClient::new();
    let out = client
        .generate_with_key("sk-test", Some(&server.uri()), &req(Provider::OpenAi))
        .await
        .unwrap();
    assert_eq!(out, "canh 1\ncanh 2");
}

#[tokio::test]
async fn ma_401_thanh_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = GenAiClient::new();
    let err = client
        .generate_with_key("bad", Some(&server.uri()), &req(Provider::Gemini))
        .await
        .unwrap_err();
    assert_eq!(err, TextGenError::Auth(401));
}

#[tokio::test]
async fn tham_so_sai_khong_goi_mang() {
    let client = GenAiClient::new();
    let mut r = req(Provider::Gemini);
    r.num_scenes = 0;
    // base_url tro toi cong khong ai nghe — neu goi mang se loi Network, day mong BadRequest.
    let err = client
        .generate_with_key("k", Some("http://127.0.0.1:1"), &r)
        .await
        .unwrap_err();
    assert!(matches!(err, TextGenError::BadRequest(_)));
}
