//! P4d: nối kho tài khoản (`Vault`) vào dịch vụ sinh text của engine — engine resolve API key theo
//! TÊN (label) credential người dùng khai ở node `script_gen`.
//!
//! An toàn bí mật: API key CHỈ mượn qua `Vault::lease_secret` (giải mã nội bộ), trả cho engine để
//! gọi API rồi thôi. KHÔNG log key. `resolve` trả `Err(String)` là mô tả AN TOÀN (không chứa key).

use std::sync::Arc;

use async_trait::async_trait;
use lvc_engine::{ResolvedSecret, TextSecretResolver};
use lvc_vault::Vault;

/// Resolver backed by [`Vault`]: tra API key `kind = "api_key"` theo `provider` + `name` (label).
pub struct VaultTextResolver {
    vault: Arc<Vault>,
}

impl VaultTextResolver {
    #[must_use]
    pub fn new(vault: Arc<Vault>) -> Self {
        Self { vault }
    }
}

#[async_trait]
impl TextSecretResolver for VaultTextResolver {
    async fn resolve(&self, provider: &str, name: &str) -> Result<ResolvedSecret, String> {
        let list = self
            .vault
            .list(Some(provider))
            .await
            .map_err(|e| e.to_string())?;
        // Khớp theo label (tên người dùng khai); nếu để trống thì lấy API key BẬT đầu tiên.
        let wanted = name.trim();
        let cred = list
            .iter()
            .find(|c| c.enabled && c.kind == "api_key" && c.label == wanted)
            .or_else(|| {
                wanted
                    .is_empty()
                    .then(|| list.iter().find(|c| c.enabled && c.kind == "api_key"))
                    .flatten()
            })
            .ok_or_else(|| {
                if wanted.is_empty() {
                    format!("chua co API key '{provider}' nao duoc bat")
                } else {
                    format!("khong tim thay API key '{wanted}' ({provider})")
                }
            })?;

        let (api_key, meta) = self
            .vault
            .lease_secret(&cred.id)
            .await
            .map_err(|e| e.to_string())?;
        let base_url = meta
            .get("base_url")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        Ok(ResolvedSecret { api_key, base_url })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_store::Db;
    use lvc_vault::{AddCredential, PlainBox};

    async fn vault() -> Arc<Vault> {
        let db = Db::open_memory().await.unwrap();
        Arc::new(Vault::new(&db, Arc::new(PlainBox)))
    }

    fn add(label: &str, secret: &str) -> AddCredential {
        AddCredential {
            provider: "gemini".into(),
            kind: "api_key".into(),
            label: label.into(),
            secret: secret.into(),
            meta: serde_json::json!({ "base_url": "http://x" }),
            proxy: String::new(),
        }
    }

    #[tokio::test]
    async fn resolve_theo_label() {
        let v = vault().await;
        v.add(&add("khoa-chinh", "AIza-1234")).await.unwrap();
        let r = VaultTextResolver::new(Arc::clone(&v));
        let out = r.resolve("gemini", "khoa-chinh").await.unwrap();
        assert_eq!(out.api_key, "AIza-1234");
        assert_eq!(out.base_url.as_deref(), Some("http://x"));
    }

    #[tokio::test]
    async fn ten_rong_lay_key_dau_tien() {
        let v = vault().await;
        v.add(&add("k1", "AIza-1")).await.unwrap();
        let r = VaultTextResolver::new(v);
        assert_eq!(r.resolve("gemini", "").await.unwrap().api_key, "AIza-1");
    }

    #[tokio::test]
    async fn khong_co_thi_bao_loi_khong_lo_key() {
        let v = vault().await;
        v.add(&add("k1", "AIza-secret")).await.unwrap();
        let r = VaultTextResolver::new(v);
        let err = r.resolve("gemini", "khong-co").await.unwrap_err();
        assert!(err.contains("khong tim thay"));
        assert!(!err.contains("AIza-secret"), "loi lo key: {err}");
    }
}
