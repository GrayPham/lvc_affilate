//! Test tích hợp luồng publish — dựng GitHub giả bằng `wiremock`, KHÔNG BAO GIỜ gọi api.github.com
//! thật (cùng luật với `lvc-template-registry/tests/http.rs`).

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use lvc_github::{GithubClient, PublishRequest, RepoRef};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "ghp_token_gia_dinh";

fn req(id: &str) -> PublishRequest {
    PublishRequest {
        id: id.to_string(),
        name: "Livestream clip ban hang".to_string(),
        version: "1.3.0".to_string(),
        entry: json!({ "id": id, "name": "Livestream clip ban hang", "kind": "workflow" }),
    }
}

/// Dựng đủ 5 endpoint cho một lần publish thành công. `index` là nội dung `index.json` hiện có.
async fn mock_github(index: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;

    // 1. Thong tin repo -> nhanh mac dinh (co y dat "master" de chac chan KHONG doan "main").
    Mock::given(method("GET"))
        .and(path("/repos/o/r"))
        .and(header("User-Agent", "lvc-affiliate"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "default_branch": "master" })),
        )
        .mount(&server)
        .await;

    // 2. SHA dau nhanh goc.
    Mock::given(method("GET"))
        .and(path("/repos/o/r/git/ref/heads/master"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "object": { "sha": "base-sha-1" } })),
        )
        .mount(&server)
        .await;

    // 3. Tao nhanh moi.
    Mock::given(method("POST"))
        .and(path("/repos/o/r/git/refs"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "ref": "refs/heads/x" })))
        .mount(&server)
        .await;

    // 4a. Doc index.json o nhanh goc (base64 CO xuong dong, dung nhu GitHub tra ve).
    let encoded = B64.encode(serde_json::to_string_pretty(&index).unwrap().as_bytes());
    let wrapped = encoded
        .as_bytes()
        .chunks(60)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    Mock::given(method("GET"))
        .and(path("/repos/o/r/contents/index.json"))
        .and(query_param("ref", "master"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "sha": "file-sha-1", "content": wrapped })),
        )
        .mount(&server)
        .await;

    // 4b. Ghi index.json len nhanh moi.
    Mock::given(method("PUT"))
        .and(path("/repos/o/r/contents/index.json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "commit": { "sha": "c1" } })),
        )
        .mount(&server)
        .await;

    // 5. Mo PR.
    Mock::given(method("POST"))
        .and(path("/repos/o/r/pulls"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(json!({ "html_url": "https://github.com/o/r/pull/7", "number": 7 })),
        )
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn publish_muc_moi_mo_duoc_pull_request() {
    let server = mock_github(json!([{ "id": "da-co", "name": "Da co" }])).await;
    let client = GithubClient::new(server.uri());

    let out = client
        .publish(&RepoRef::new("o", "r"), TOKEN, &req("moi-tinh"))
        .await
        .unwrap();

    assert_eq!(out.pull_request_url, "https://github.com/o/r/pull/7");
    assert_eq!(out.branch, "publish/moi-tinh-1-3-0");
    assert!(!out.updated_existing, "id chua co -> them moi");
}

#[tokio::test]
async fn publish_trung_id_bao_la_cap_nhat() {
    let server = mock_github(json!([{ "id": "trung-id", "name": "Ban cu" }])).await;
    let client = GithubClient::new(server.uri());

    let out = client
        .publish(&RepoRef::new("o", "r"), TOKEN, &req("trung-id"))
        .await
        .unwrap();

    assert!(
        out.updated_existing,
        "id da co -> thay the, khong them trung"
    );
}

#[tokio::test]
async fn token_sai_bao_unauthorized_khong_lo_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Bad credentials"))
        .mount(&server)
        .await;

    let err = GithubClient::new(server.uri())
        .publish(&RepoRef::new("o", "r"), TOKEN, &req("x"))
        .await
        .unwrap_err();

    assert_eq!(err.code(), "GITHUB_UNAUTHORIZED");
    assert!(
        !err.to_string().contains(TOKEN),
        "thong diep loi KHONG duoc chua token"
    );
}

#[tokio::test]
async fn repo_khong_ton_tai_bao_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/o/r"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    let err = GithubClient::new(server.uri())
        .publish(&RepoRef::new("o", "r"), TOKEN, &req("x"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GITHUB_NOT_FOUND");
}

#[tokio::test]
async fn index_json_khong_phai_mang_bao_loi_ro() {
    let server = mock_github(json!({ "khong": "phai mang" })).await;
    let err = GithubClient::new(server.uri())
        .publish(&RepoRef::new("o", "r"), TOKEN, &req("x"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "GITHUB_BAD_INDEX");
}
