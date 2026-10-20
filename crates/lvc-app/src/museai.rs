//! Nối kho tài khoản (`Vault`) vào `MuseaiAccountResolver` của engine — engine resolve TÊN (label)
//! credential người dùng khai ở node `museai_video` (tham số `account`) thành `profile_dir`
//! (đường dẫn profile Chromium thật, `meta.profile_dir` của credential `provider=museai`). Khớp
//! đúng khuôn `crate::veo3::VaultVeo3AccountResolver`.
//!
//! An toàn bí mật: `profile_dir` KHÔNG phải bí mật (không mã hoá, có sẵn trong `CredentialView`
//! trả từ `Vault::list` — không cần `lease_secret`/giải mã). `resolve` trả `Err(String)` là mô tả
//! AN TOÀN (không chứa gì nhạy cảm).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use lvc_engine::MuseaiAccountResolver;
use lvc_vault::Vault;

/// Resolver backed by [`Vault`]: tra `profile_dir` credential `provider="museai"`,
/// `kind` `login`/`session`, theo `label` (tên người dùng khai ở tham số `account` của node).
/// `account` RỖNG — tự chọn tài khoản ĐẦU TIÊN còn dùng được trong kho (đủ `enabled` + có
/// `profile_dir`), giống hành vi "để trống = tự chọn" của Dola — người dùng không bị ép gõ tay tên
/// tài khoản khi không quan tâm dùng tài khoản nào (review người dùng 2026-10-13).
pub struct VaultMuseaiAccountResolver {
    vault: Arc<Vault>,
    /// Con trỏ XOAY VÒNG khi `account` rỗng — mỗi lần resolve lấy account kế tiếp để chia tải nhiều
    /// tài khoản (giảm 502/rate-limit, song song thật vì khác `profile_dir`).
    next: AtomicUsize,
}

impl VaultMuseaiAccountResolver {
    #[must_use]
    pub fn new(vault: Arc<Vault>) -> Self {
        Self {
            vault,
            next: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl MuseaiAccountResolver for VaultMuseaiAccountResolver {
    async fn profile_dir(&self, credential_id: &str) -> Result<PathBuf, String> {
        let wanted = credential_id.trim();
        let list = self
            .vault
            .list(Some("museai"))
            .await
            .map_err(|e| e.to_string())?;
        let logged_in = |c: &&lvc_vault::CredentialView| {
            c.enabled && (c.kind == "login" || c.kind == "session")
        };
        let has_profile = |c: &&lvc_vault::CredentialView| {
            c.meta
                .get("profile_dir")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|s| !s.is_empty())
        };
        let cred = if wanted.is_empty() {
            // XOAY VÒNG qua các tài khoản sẵn sàng (đủ `enabled` + có `profile_dir`). Nhiều cảnh/
            // nhiều account → chia đều thay vì dồn hết vào account đầu tiên.
            let candidates: Vec<&lvc_vault::CredentialView> = list
                .iter()
                .filter(|c| logged_in(c) && has_profile(c))
                .collect();
            if candidates.is_empty() {
                return Err("khong co tai khoan MuseAI nao san sang (tab Tai khoan AI)".to_string());
            }
            let idx = self.next.fetch_add(1, Ordering::Relaxed) % candidates.len();
            candidates[idx]
        } else {
            list.iter()
                .find(|c| logged_in(c) && c.label == wanted)
                .ok_or_else(|| {
                    format!("khong tim thay tai khoan MuseAI '{wanted}' (tab Tai khoan AI)")
                })?
        };
        let profile_dir = cred
            .meta
            .get("profile_dir")
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                format!(
                    "tai khoan MuseAI '{}' thieu profile_dir — dang nhap lai qua tab Tai khoan AI",
                    cred.label
                )
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
            provider: "museai".into(),
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
        v.add(&add("tk-1", "C:/accounts/.logins/museai/tk-1"))
            .await
            .unwrap();
        let r = VaultMuseaiAccountResolver::new(Arc::clone(&v));
        let dir = r.profile_dir("tk-1").await.unwrap();
        assert_eq!(dir, PathBuf::from("C:/accounts/.logins/museai/tk-1"));
    }

    #[tokio::test]
    async fn khong_tim_thay_bao_loi_ro_khong_lo_gi() {
        let v = vault().await;
        v.add(&add("tk-1", "C:/x")).await.unwrap();
        let r = VaultMuseaiAccountResolver::new(v);
        let err = r.profile_dir("khong-ton-tai").await.unwrap_err();
        assert!(err.contains("khong tim thay"));
    }

    #[tokio::test]
    async fn account_rong_khong_co_tai_khoan_nao_bao_loi_ro() {
        let v = vault().await;
        let r = VaultMuseaiAccountResolver::new(v);
        let err = r.profile_dir("").await.unwrap_err();
        assert!(err.contains("khong co tai khoan"));
    }

    #[tokio::test]
    async fn account_rong_tu_chon_tai_khoan_dau_tien_con_dung_duoc() {
        // `VaultRepo::list` sap theo `label` (ORDER BY label) — "tk-1" phai den TRUOC "tk-2".
        let v = vault().await;
        v.add(&add("tk-1", "C:/accounts/.logins/museai/tk-1"))
            .await
            .unwrap();
        v.add(&add("tk-2", "C:/accounts/.logins/museai/tk-2"))
            .await
            .unwrap();
        let r = VaultMuseaiAccountResolver::new(v);
        let dir = r.profile_dir("").await.unwrap();
        assert_eq!(dir, PathBuf::from("C:/accounts/.logins/museai/tk-1"));
    }

    /// `account` rỗng + NHIỀU tài khoản → XOAY VÒNG (round-robin): mỗi lần gọi lấy account kế tiếp,
    /// hết vòng quay lại. Để nhiều account chia tải (giảm 502/rate-limit + song song thật).
    #[tokio::test]
    async fn account_rong_xoay_vong_nhieu_tai_khoan() {
        let v = vault().await;
        v.add(&add("tk-1", "C:/p/tk-1")).await.unwrap();
        v.add(&add("tk-2", "C:/p/tk-2")).await.unwrap();
        v.add(&add("tk-3", "C:/p/tk-3")).await.unwrap();
        let r = VaultMuseaiAccountResolver::new(v);
        // `list` ORDER BY label -> tk-1, tk-2, tk-3; xoay vòng qua 4 lần.
        let mut got = Vec::new();
        for _ in 0..4 {
            got.push(r.profile_dir("").await.unwrap());
        }
        assert_eq!(
            got,
            vec![
                PathBuf::from("C:/p/tk-1"),
                PathBuf::from("C:/p/tk-2"),
                PathBuf::from("C:/p/tk-3"),
                PathBuf::from("C:/p/tk-1"),
            ]
        );
    }

    #[tokio::test]
    async fn account_rong_bo_qua_tai_khoan_thieu_profile_dir() {
        let v = vault().await;
        let mut no_profile = add("tk-1", "");
        no_profile.meta = serde_json::json!({});
        v.add(&no_profile).await.unwrap();
        v.add(&add("tk-2", "C:/accounts/.logins/museai/tk-2"))
            .await
            .unwrap();
        let r = VaultMuseaiAccountResolver::new(v);
        let dir = r.profile_dir("").await.unwrap();
        assert_eq!(dir, PathBuf::from("C:/accounts/.logins/museai/tk-2"));
    }

    #[tokio::test]
    async fn thieu_profile_dir_trong_meta_bao_loi_ro() {
        let v = vault().await;
        let mut cred = add("tk-1", "");
        cred.meta = serde_json::json!({});
        v.add(&cred).await.unwrap();
        let r = VaultMuseaiAccountResolver::new(v);
        let err = r.profile_dir("tk-1").await.unwrap_err();
        assert!(err.contains("profile_dir"));
    }
}
