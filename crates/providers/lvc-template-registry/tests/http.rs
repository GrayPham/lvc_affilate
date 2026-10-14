//! Test client HTTP với server giả (wiremock) — KHÔNG bao giờ gọi `raw.githubusercontent.com` thật.

use lvc_template_registry::{RegistryClient, RegistryError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_index() -> serde_json::Value {
    serde_json::json!([
        {
            "id": "ugc-unboxing-demo",
            "name": "UGC Unboxing (demo)",
            "description": "mo ta",
            "category": "Bán hàng/Sản phẩm",
            "thumbnail": "https://example.com/thumbs/ugc-unboxing-demo.svg",
            "updated_at": 1_791_713_964_000.0,
            "graph": { "version": 2, "nodes": [], "edges": [] }
        }
    ])
}

#[tokio::test]
async fn fetch_index_parse_dung_hinh_dang() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_index()))
        .mount(&server)
        .await;

    let client = RegistryClient::new(format!("{}/index.json", server.uri()));
    let items = client.fetch_index().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "ugc-unboxing-demo");
    assert_eq!(items[0].category, "Bán hàng/Sản phẩm");
}

#[tokio::test]
async fn fetch_index_loi_http_tra_ve_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server dang bao tri"))
        .mount(&server)
        .await;

    let client = RegistryClient::new(format!("{}/index.json", server.uri()));
    let err = client.fetch_index().await.unwrap_err();
    assert_eq!(
        err,
        RegistryError::Http {
            status: 500,
            detail: "server dang bao tri".to_string(),
        }
    );
}

#[tokio::test]
async fn fetch_index_json_sai_hinh_dang_tra_loi_decode() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("khong phai json hop le"))
        .mount(&server)
        .await;

    let client = RegistryClient::new(format!("{}/index.json", server.uri()));
    assert!(matches!(
        client.fetch_index().await,
        Err(RegistryError::Decode(_))
    ));
}

#[tokio::test]
async fn fetch_index_mang_rong_hop_le() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/index.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .mount(&server)
        .await;

    let client = RegistryClient::new(format!("{}/index.json", server.uri()));
    assert!(client.fetch_index().await.unwrap().is_empty());
}
