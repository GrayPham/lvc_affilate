//! Kiểm tra API key bằng endpoint list-models CÔNG KHAI (OpenAI/Gemini) — GET thường, không ký gì,
//! không né phát hiện. Trả trạng thái để lưu vào `credentials.status`.

use serde::{Deserialize, Serialize};

/// Kết quả kiểm tra một credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyOutcome {
    /// `ok` | `expired` | `quota` | `unknown`.
    pub status: String,
    /// Mô tả ngắn (không chứa key).
    pub detail: String,
}

impl VerifyOutcome {
    fn of(status: &str, detail: impl Into<String>) -> Self {
        Self {
            status: status.to_string(),
            detail: detail.into(),
        }
    }
}

/// Endpoint list-models cho provider (base có thể override qua meta `base_url`).
fn models_url(provider: &str, base_url: Option<&str>) -> Option<(String, AuthStyle)> {
    match provider {
        "openai" => Some((
            format!(
                "{}/v1/models",
                base_url
                    .unwrap_or("https://api.openai.com")
                    .trim_end_matches('/')
            ),
            AuthStyle::Bearer,
        )),
        "gemini" => Some((
            format!(
                "{}/v1beta/models",
                base_url
                    .unwrap_or("https://generativelanguage.googleapis.com")
                    .trim_end_matches('/')
            ),
            AuthStyle::GoogleKeyQuery,
        )),
        _ => None,
    }
}

enum AuthStyle {
    Bearer,
    GoogleKeyQuery,
}

/// Kiểm tra `api_key` cho `provider`. `client` do caller cấp (proxy/timeout). Chỉ hỗ trợ openai/gemini;
/// provider khác trả `unknown` ("chưa hỗ trợ kiểm tự động").
///
/// # Errors
/// Không trả `Err` — lỗi mạng thành `status="unknown"` để UI không kẹt (giống Python nuốt lỗi mạng).
pub async fn verify_api_key(
    client: &reqwest::Client,
    provider: &str,
    api_key: &str,
    base_url: Option<&str>,
) -> VerifyOutcome {
    let Some((url, style)) = models_url(provider, base_url) else {
        return VerifyOutcome::of("unknown", "provider chua ho tro kiem tu dong");
    };
    let req = match style {
        AuthStyle::Bearer => client.get(&url).bearer_auth(api_key),
        AuthStyle::GoogleKeyQuery => client.get(&url).query(&[("key", api_key)]),
    };
    match req.send().await {
        Ok(resp) => classify(resp.status().as_u16()),
        Err(e) => VerifyOutcome::of("unknown", format!("loi mang: {}", e.without_url())),
    }
}

fn classify(status: u16) -> VerifyOutcome {
    match status {
        200 => VerifyOutcome::of("ok", "key hop le"),
        401 | 403 => VerifyOutcome::of("expired", "key sai hoac het han (HTTP 401/403)"),
        429 => VerifyOutcome::of("quota", "vuot han muc (HTTP 429)"),
        s => VerifyOutcome::of("unknown", format!("HTTP {s}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn run(provider: &str, code: u16) -> VerifyOutcome {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(code))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        verify_api_key(&client, provider, "test-key", Some(&server.uri())).await
    }

    #[tokio::test]
    async fn openai_200_ok() {
        assert_eq!(run("openai", 200).await.status, "ok");
    }

    #[tokio::test]
    async fn openai_401_expired() {
        assert_eq!(run("openai", 401).await.status, "expired");
    }

    #[tokio::test]
    async fn gemini_429_quota() {
        assert_eq!(run("gemini", 429).await.status, "quota");
    }

    #[tokio::test]
    async fn provider_la_unknown() {
        let c = reqwest::Client::new();
        assert_eq!(
            verify_api_key(&c, "dola", "k", None).await.status,
            "unknown"
        );
    }

    #[tokio::test]
    async fn gemini_gui_key_qua_query_khong_qua_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1beta/models"))
            .and(wiremock::matchers::query_param("key", "test-key"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let c = reqwest::Client::new();
        let out = verify_api_key(&c, "gemini", "test-key", Some(&server.uri())).await;
        assert_eq!(out.status, "ok");
    }
}
