//! P4d: dịch vụ sinh text ([`lvc_genai::TextGenHost`]) do engine cấp cho node `script_gen`.
//!
//! Ranh giới bí mật (BẮT BUỘC): node chỉ truyền TÊN credential trong `TextGenReq`. Engine resolve
//! tên đó thành API key qua [`TextSecretResolver`] (lvc-app impl bằng `Vault`), rồi gọi
//! [`lvc_genai::GenAiClient`]. API key CHỈ nằm trong RAM engine giữa lúc resolve và lúc gọi HTTP —
//! KHÔNG đi ra event/log/UI. Engine KHÔNG phụ thuộc `lvc-vault` (chỉ trait này) để giữ tầng gọn.

use std::sync::Arc;

use async_trait::async_trait;
use lvc_genai::{GenAiClient, TextGenError, TextGenHost, TextGenReq};

/// Bí mật đã resolve cho một lần sinh text. Chỉ dùng NỘI BỘ engine.
#[derive(Debug, Clone)]
pub struct ResolvedSecret {
    pub api_key: String,
    /// `base_url` tuỳ chọn (meta credential) — `None` = endpoint chính thức của provider.
    pub base_url: Option<String>,
}

/// Nguồn API key cho engine — lvc-app impl bằng `Vault` (tra theo `provider` + tên/label credential).
/// Trả `Err(String)` là mô tả AN TOÀN (KHÔNG chứa key). Tách trait để `lvc-engine` KHÔNG phụ thuộc
/// `lvc-vault`.
#[async_trait]
pub trait TextSecretResolver: Send + Sync {
    /// # Errors
    /// Chuỗi mô tả khi không tìm/không giải mã được credential.
    async fn resolve(&self, provider: &str, name: &str) -> Result<ResolvedSecret, String>;
}

/// Resolver mặc định: LUÔN lỗi (dùng khi chưa cắm kho tài khoản — vd CLI/test). Nhờ vậy engine vẫn
/// chèn dịch vụ `TextGenHost` VÔ ĐIỀU KIỆN; node báo lỗi rõ ràng thay vì thiếu service.
pub(crate) struct NoResolver;

#[async_trait]
impl TextSecretResolver for NoResolver {
    async fn resolve(&self, _provider: &str, _name: &str) -> Result<ResolvedSecret, String> {
        Err("chua cau hinh kho tai khoan (khong resolve duoc API key)".to_string())
    }
}

/// Dịch vụ `TextGenHost` do engine cấp: resolve key → gọi client. KHÔNG log key/prompt.
pub(crate) struct EngineTextGenHost {
    client: Arc<GenAiClient>,
    resolver: Arc<dyn TextSecretResolver>,
}

impl EngineTextGenHost {
    pub(crate) fn new(client: Arc<GenAiClient>, resolver: Arc<dyn TextSecretResolver>) -> Self {
        Self { client, resolver }
    }
}

#[async_trait]
impl TextGenHost for EngineTextGenHost {
    async fn generate(&self, req: TextGenReq) -> Result<String, TextGenError> {
        let secret = self
            .resolver
            .resolve(req.provider.as_tag(), &req.credential_name)
            .await
            .map_err(TextGenError::Credential)?;
        self.client
            .generate_with_key(&secret.api_key, secret.base_url.as_deref(), &req)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_genai::Provider;

    fn req() -> TextGenReq {
        TextGenReq {
            credential_name: "khoa".into(),
            provider: Provider::Gemini,
            system: String::new(),
            prompt: "x".into(),
            num_scenes: 2,
            language: "vi".into(),
            style: String::new(),
        }
    }

    #[tokio::test]
    async fn no_resolver_bao_loi_credential() {
        let host = EngineTextGenHost::new(Arc::new(GenAiClient::new()), Arc::new(NoResolver));
        let err = host.generate(req()).await.unwrap_err();
        assert!(matches!(err, TextGenError::Credential(_)));
    }

    struct OkResolver;
    #[async_trait]
    impl TextSecretResolver for OkResolver {
        async fn resolve(&self, provider: &str, _name: &str) -> Result<ResolvedSecret, String> {
            assert_eq!(provider, "gemini");
            Ok(ResolvedSecret {
                api_key: "k".into(),
                base_url: Some("http://127.0.0.1:1".into()),
            })
        }
    }

    #[tokio::test]
    async fn resolver_ok_thi_goi_client() {
        let host = EngineTextGenHost::new(Arc::new(GenAiClient::new()), Arc::new(OkResolver));
        // Client sẽ cố gọi mạng tới cổng chết -> lỗi Network (chứng minh đã qua resolve).
        let err = host.generate(req()).await.unwrap_err();
        assert!(matches!(err, TextGenError::Network(_)), "gap {err:?}");
    }
}
