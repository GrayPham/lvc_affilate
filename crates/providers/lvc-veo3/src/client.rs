//! Goi batchexecute THAT bang `reqwest`. Nhan [`crate::FlowSession`] (da harvest) +
//! `recaptcha_token` (da mint, nhung san trong `inner` boi `rpc::*`) lam THAM SO — KHONG tu
//! resolve secret, KHONG log cookie/token day du (giong `lvc-genai` nhan `api_key` lam tham so).
//! 2 buoc can trinh duyet thuc (mint captcha, harvest phien) nam o `lvc_browser::veo3`, KHONG o
//! day — xem ADR-R018 (spec 2026-09-27 v2).

use serde_json::Value;

use crate::FlowSession;
use crate::error::VeoError;
use crate::wire;

/// Client HTTP thuan cho batchexecute — khong giu bi mat nao (session/token la tham so cua tung
/// lan goi). `endpoint` co the doi trong test (wiremock); ban thuc luon dung [`wire::BASE_URL`].
#[derive(Debug, Clone)]
pub struct BatchExecuteClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for BatchExecuteClient {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchExecuteClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            endpoint: wire::BASE_URL.to_string(),
        }
    }

    /// Doi endpoint batchexecute (chi dung trong test — wiremock).
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.endpoint = base_url.into();
        self
    }

    /// Goi 1 rpc: build URL+body tu `session`, POST, giai ma wire, tra payload cua DUNG `rpcid`.
    /// `source_path` thuong la `/project/<id>` (header `source-path` — Google dung de chan doan
    /// client, khong anh huong ket qua nhung nen dung dung cho giong trang thuc).
    ///
    /// # Errors
    /// Tra [`VeoError`] khi goi mang loi, HTTP khong phai 2xx, hoac response khong co payload cho
    /// `rpcid` nay (rpcid vang hoac payload `null` deu coi la loi — body loi duoc dua vao
    /// [`VeoError::from_text`] de phan loai).
    pub async fn call(
        &self,
        session: &FlowSession,
        rpcid: &str,
        inner: &Value,
        source_path: &str,
    ) -> Result<Value, VeoError> {
        let reqid = 100_000 + rand::random::<u32>() % 900_000;
        let url = wire::build_url(
            &self.endpoint,
            rpcid,
            &session.bl,
            &session.f_sid,
            source_path,
            reqid,
            "vi",
        );
        let body = wire::encode_freq(rpcid, inner, &session.at);
        let response = self
            .http
            .post(&url)
            .header("cookie", &session.cookie_header)
            .header(
                "content-type",
                "application/x-www-form-urlencoded;charset=UTF-8",
            )
            .header("x-same-domain", "1")
            .body(body)
            .send()
            .await
            .map_err(|e| VeoError::from_text(&format!("loi mang goi batchexecute: {e}")))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(VeoError::from_text(&format!(
                "HTTP {}: {text}",
                status.as_u16()
            )));
        }
        match wire::decode_batchexecute(&text).remove(rpcid) {
            Some(Some(payload)) => Ok(payload),
            _ => Err(VeoError::from_text(&text)),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn fake_session() -> FlowSession {
        FlowSession {
            cookie_header: "SID=abc; __Secure-1PSID=xyz".to_string(),
            at: "xsrf-token".to_string(),
            bl: "boq_build".to_string(),
            f_sid: "sess-1".to_string(),
            project_id: "PROJECT-1".to_string(),
        }
    }

    fn wire_body(rpcid: &str, payload: &Value) -> String {
        let inner_str = serde_json::to_string(payload).unwrap();
        let env = json!([["wrb.fr", rpcid, inner_str, null, null, null, "generic"]]);
        let chunk = serde_json::to_string(&env).unwrap();
        format!(")]}}'\n\n{}\n{chunk}\n", chunk.len())
    }

    #[tokio::test]
    async fn call_thanh_cong_tra_dung_payload() {
        let server = MockServer::start().await;
        let payload = json!(["OP-1", "SCENE-1"]);
        Mock::given(method("POST"))
            .and(path("/_/AiSandboxAngularFrontend/data/batchexecute"))
            .respond_with(ResponseTemplate::new(200).set_body_string(wire_body("YhhmEf", &payload)))
            .mount(&server)
            .await;

        let endpoint = format!(
            "{}/_/AiSandboxAngularFrontend/data/batchexecute",
            server.uri()
        );
        let client = BatchExecuteClient::new().with_base_url(endpoint);
        let session = fake_session();
        let result = client
            .call(&session, "YhhmEf", &json!([1, 2]), "/project/abc")
            .await;

        assert_eq!(result, Ok(payload));
    }

    #[tokio::test]
    async fn call_http_loi_tra_veo_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/_/AiSandboxAngularFrontend/data/batchexecute"))
            .respond_with(ResponseTemplate::new(403).set_body_string(
                r#"{"error":{"details":[{"reason":"PUBLIC_ERROR_UNUSUAL_ACTIVITY"}]}}"#,
            ))
            .mount(&server)
            .await;

        let endpoint = format!(
            "{}/_/AiSandboxAngularFrontend/data/batchexecute",
            server.uri()
        );
        let client = BatchExecuteClient::new().with_base_url(endpoint);
        let err = client
            .call(&fake_session(), "ogiZ0b", &json!([]), "/project/abc")
            .await
            .expect_err("phai loi khi HTTP 403");

        assert!(!err.is_retry_pointless());
        assert_eq!(err.code.as_deref(), Some("PUBLIC_ERROR_UNUSUAL_ACTIVITY"));
    }
}
