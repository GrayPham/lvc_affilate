//! P5 Task 9b (phần `lvc-app`): nối kho tài khoản (`Vault`) vào `VeoAccountResolver` của engine —
//! engine resolve TÊN (label) credential người dùng khai ở node `veo3_image`/`veo3_video` (tham
//! số `account`) thành `profile_dir` (đường dẫn profile Chrome thật, `meta.profile_dir` của
//! credential `provider=google`). Khớp đúng khuôn `crate::textgen::VaultTextResolver`.
//!
//! An toàn bí mật: `profile_dir` KHÔNG phải bí mật (không mã hoá, có sẵn trong `CredentialView`
//! trả từ `Vault::list` — không cần `lease_secret`/giải mã). `resolve` trả `Err(String)` là mô tả
//! AN TOÀN (không chứa gì nhạy cảm).

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lvc_engine::VeoAccountResolver;
use lvc_vault::Vault;

/// Resolver backed by [`Vault`]: tra `profile_dir` credential `provider="google"`,
/// `kind` `login`/`session`, theo `label` (tên người dùng khai ở tham số `account` của node).
pub struct VaultVeo3AccountResolver {
    vault: Arc<Vault>,
}

impl VaultVeo3AccountResolver {
    #[must_use]
    pub fn new(vault: Arc<Vault>) -> Self {
        Self { vault }
    }
}

#[async_trait]
impl VeoAccountResolver for VaultVeo3AccountResolver {
    async fn profile_dir(&self, credential_id: &str) -> Result<PathBuf, String> {
        let wanted = credential_id.trim();
        if wanted.is_empty() {
            return Err("tham so 'account' rong — chua chon tai khoan Google".to_string());
        }
        let list = self
            .vault
            .list(Some("google"))
            .await
            .map_err(|e| e.to_string())?;
        let cred = list
            .iter()
            .find(|c| c.enabled && (c.kind == "login" || c.kind == "session") && c.label == wanted)
            .ok_or_else(|| {
                format!("khong tim thay tai khoan Google '{wanted}' (tab Tai khoan AI)")
            })?;
        let profile_dir = cred
            .meta
            .get("profile_dir")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                format!("tai khoan Google '{wanted}' thieu profile_dir — dang nhap lai qua tab Tai khoan AI")
            })?;
        Ok(PathBuf::from(profile_dir))
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

    fn add(label: &str, profile_dir: &str) -> AddCredential {
        AddCredential {
            provider: "google".into(),
            kind: "login".into(),
            label: label.into(),
            secret: "session-placeholder".into(),
            meta: serde_json::json!({ "profile_dir": profile_dir }),
            proxy: String::new(),
        }
    }

    #[tokio::test]
    async fn resolve_theo_label() {
        let v = vault().await;
        v.add(&add("tk-phu", "C:/accounts/.logins/google/tk-phu"))
            .await
            .unwrap();
        let r = VaultVeo3AccountResolver::new(Arc::clone(&v));
        let dir = r.profile_dir("tk-phu").await.unwrap();
        assert_eq!(dir, PathBuf::from("C:/accounts/.logins/google/tk-phu"));
    }

    #[tokio::test]
    async fn khong_tim_thay_bao_loi_ro_khong_lo_gi() {
        let v = vault().await;
        v.add(&add("tk-phu", "C:/x")).await.unwrap();
        let r = VaultVeo3AccountResolver::new(v);
        let err = r.profile_dir("khong-ton-tai").await.unwrap_err();
        assert!(err.contains("khong tim thay"));
    }

    #[tokio::test]
    async fn account_rong_bao_loi_ro_rang() {
        let v = vault().await;
        let r = VaultVeo3AccountResolver::new(v);
        let err = r.profile_dir("").await.unwrap_err();
        assert!(err.contains("rong"));
    }

    #[tokio::test]
    async fn thieu_profile_dir_trong_meta_bao_loi_ro() {
        let v = vault().await;
        let mut cred = add("tk-phu", "");
        cred.meta = serde_json::json!({});
        v.add(&cred).await.unwrap();
        let r = VaultVeo3AccountResolver::new(v);
        let err = r.profile_dir("tk-phu").await.unwrap_err();
        assert!(err.contains("profile_dir"));
    }
}
