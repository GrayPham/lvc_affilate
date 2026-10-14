//! Test client HTTP với server giả (wiremock) — KHÔNG bao giờ gọi API AI thật.

use lvc_tts::{Provider, SynthReq, TtsClient, TtsError};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn req(provider: Provider) -> SynthReq {
    SynthReq {
        provider,
        text: "xin chao".into(),
        voice_id: "voice-1".into(),
        model: String::new(),
        speed: 1.0,
    }
}

#[tokio::test]
async fn elevenlabs_gui_key_qua_header_va_tra_bytes_tho() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/text-to-speech/voice-1"))
        .and(header("xi-api-key", "el-test"))
        .and(query_param("output_format", "mp3_44100_128"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fake-mp3-bytes".to_vec()))
        .mount(&server)
        .await;

    let client = TtsClient::new();
    let out = client
        .generate_with_key("el-test", Some(&server.uri()), &req(Provider::ElevenLabs))
        .await
        .unwrap();
    assert_eq!(out, b"fake-mp3-bytes".to_vec());
}

#[tokio::test]
async fn minimax_gui_key_qua_bearer_va_giai_ma_hex() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/t2a_v2"))
        .and(header("authorization", "Bearer mm-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": { "audio": "68656c6c6f", "status": 2 },
            "base_resp": { "status_code": 0, "status_msg": "success" }
        })))
        .mount(&server)
        .await;

    let client = TtsClient::new();
    let out = client
        .generate_with_key("mm-test", Some(&server.uri()), &req(Provider::Minimax))
        .await
        .unwrap();
    assert_eq!(out, b"hello".to_vec());
}

#[tokio::test]
async fn minimax_loi_nghiep_vu_du_http_200_van_bao_loi() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/t2a_v2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "base_resp": { "status_code": 1004, "status_msg": "invalid api key" }
        })))
        .mount(&server)
        .await;

    let client = TtsClient::new();
    let err = client
        .generate_with_key("bad", Some(&server.uri()), &req(Provider::Minimax))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        TtsError::Http {
            status: 1004,
            detail: "invalid api key".to_string()
        }
    );
}

#[tokio::test]
async fn ma_401_thanh_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = TtsClient::new();
    let err = client
        .generate_with_key("bad", Some(&server.uri()), &req(Provider::ElevenLabs))
        .await
        .unwrap_err();
    assert_eq!(err, TtsError::Auth(401));
}

#[tokio::test]
async fn tham_so_sai_khong_goi_mang() {
    let client = TtsClient::new();
    let mut r = req(Provider::ElevenLabs);
    r.text = String::new();
    // base_url tro toi cong khong ai nghe — neu goi mang se loi Network, day mong BadRequest.
    let err = client
        .generate_with_key("k", Some("http://127.0.0.1:1"), &r)
        .await
        .unwrap_err();
    assert!(matches!(err, TtsError::BadRequest(_)));
}
