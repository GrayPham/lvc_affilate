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
use lvc_engine::{AccountScheduler, Candidate, MuseaiAccountResolver, MuseaiLease};
use lvc_vault::Vault;
use tokio_util::sync::CancellationToken;

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
    /// Chia tải theo TẢI THỰC giữa các tài khoản khi node song song (xem `lvc_engine::museai_sched`):
    /// mỗi tài khoản một chỗ, chọn tài khoản rảnh nhất, cách ly tài khoản vừa lỗi.
    sched: Arc<AccountScheduler>,
}

impl VaultMuseaiAccountResolver {
    #[must_use]
    pub fn new(vault: Arc<Vault>) -> Self {
        Self {
            vault,
            next: AtomicUsize::new(0),
            sched: Arc::new(AccountScheduler::new()),
        }
    }

    /// Danh sách tài khoản dùng được cho yêu cầu `wanted` (đã `trim`). `wanted` RỖNG = tự chọn: mọi
    /// tài khoản sẵn sàng (`enabled` + có `profile_dir`), BỎ các tài khoản đã kiểm tra ra
    /// `expired` — không dại dột thử lại tài khoản đã biết hết phiên. `wanted` có tên = ĐÚNG tài khoản
    /// đó (người dùng đã ghim, không xoay sang tài khoản khác).
    async fn candidates(&self, wanted: &str) -> Result<Vec<Candidate>, String> {
        let list = self
            .vault
            .list(Some("museai"))
            .await
            .map_err(|e| e.to_string())?;
        let usable = |c: &&lvc_vault::CredentialView| {
            c.enabled
                && (c.kind == "login" || c.kind == "session")
                && c.meta
                    .get("profile_dir")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|s| !s.is_empty())
        };
        let to_candidate = |c: &lvc_vault::CredentialView| Candidate {
            key: c.id.clone(),
            profile_dir: PathBuf::from(
                c.meta
                    .get("profile_dir")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
            ),
        };
        if wanted.is_empty() {
            let cands: Vec<Candidate> = list
                .iter()
                .filter(|c| usable(c) && c.status != "expired")
                .map(to_candidate)
                .collect();
            if cands.is_empty() {
                return Err("khong co tai khoan MuseAI nao san sang (tab Tai khoan AI)".to_string());
            }
            Ok(cands)
        } else {
            list.iter()
                .find(|c| usable(c) && c.label == wanted)
                .map(|c| vec![to_candidate(c)])
                .ok_or_else(|| {
                    format!("khong tim thay tai khoan MuseAI '{wanted}' (tab Tai khoan AI)")
                })
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

    async fn lease(
        &self,
        credential_id: &str,
        avoid: &[String],
        cancel: &CancellationToken,
    ) -> Result<MuseaiLease, String> {
        let cands = self.candidates(credential_id.trim()).await?;
        self.sched.acquire(&cands, avoid, cancel).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_store::Db;
    use lvc_vault::{AddCredential, PlainBox};
    use std::path::Path;

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

    // ---- lease: chia tai theo tai thuc ----

    async fn two(v: &Arc<Vault>) -> (String, String) {
        let a = v.add(&add("tk-1", "C:/p/tk-1")).await.unwrap().id;
        let b = v.add(&add("tk-2", "C:/p/tk-2")).await.unwrap().id;
        (a, b)
    }

    #[tokio::test]
    async fn lease_hai_node_dong_thoi_roi_vao_hai_tai_khoan_khac_nhau() {
        let v = vault().await;
        two(&v).await;
        let r = VaultMuseaiAccountResolver::new(v);
        let cancel = CancellationToken::new();
        let l1 = r.lease("", &[], &cancel).await.unwrap();
        let l2 = r.lease("", &[], &cancel).await.unwrap();
        assert_ne!(
            l1.profile_dir(),
            l2.profile_dir(),
            "2 node chay cung luc phai dung 2 tai khoan khac nhau"
        );
    }

    #[tokio::test]
    async fn lease_node_thu_ba_cho_roi_nhan_lai_cho_vua_tra() {
        let v = vault().await;
        two(&v).await;
        let r = Arc::new(VaultMuseaiAccountResolver::new(v));
        let cancel = CancellationToken::new();
        let l1 = r.lease("", &[], &cancel).await.unwrap();
        let _l2 = r.lease("", &[], &cancel).await.unwrap();

        let r2 = Arc::clone(&r);
        let c2 = cancel.clone();
        let third = tokio::spawn(async move { r2.lease("", &[], &c2).await });
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        assert!(
            !third.is_finished(),
            "ca hai tai khoan dang ban -> phai cho"
        );

        let freed = l1.profile_dir().to_path_buf();
        drop(l1);
        let l3 = tokio::time::timeout(std::time::Duration::from_secs(3), third)
            .await
            .expect("duoc danh thuc khi co cho trong")
            .unwrap()
            .unwrap();
        assert_eq!(l3.profile_dir(), freed);
    }

    #[tokio::test]
    async fn lease_bo_tai_khoan_da_thu_hong_chuyen_sang_tai_khoan_con_lai() {
        let v = vault().await;
        let (a, _b) = two(&v).await;
        let r = VaultMuseaiAccountResolver::new(v);
        let l = r.lease("", &[a], &CancellationToken::new()).await.unwrap();
        assert_eq!(l.profile_dir(), Path::new("C:/p/tk-2"));
    }

    #[tokio::test]
    async fn lease_het_tai_khoan_du_phong_bao_loi_ro() {
        let v = vault().await;
        let (a, b) = two(&v).await;
        let r = VaultMuseaiAccountResolver::new(v);
        let err = r
            .lease("", &[a, b], &CancellationToken::new())
            .await
            .unwrap_err();
        assert!(err.contains("du phong"), "{err}");
    }

    #[tokio::test]
    async fn lease_tai_khoan_da_ghim_khong_bi_xoay_sang_tai_khoan_khac() {
        let v = vault().await;
        two(&v).await;
        let r = VaultMuseaiAccountResolver::new(v);
        let cancel = CancellationToken::new();
        // Ghim tk-2: luon tk-2 du tk-1 ranh va dung dau danh sach.
        for _ in 0..2 {
            let l = r.lease("tk-2", &[], &cancel).await.unwrap();
            assert_eq!(l.profile_dir(), Path::new("C:/p/tk-2"));
        }
    }

    #[tokio::test]
    async fn lease_tu_chon_bo_tai_khoan_da_kiem_tra_het_han() {
        let v = vault().await;
        let (a, _b) = two(&v).await;
        v.set_status(&a, "expired").await.unwrap();
        let r = VaultMuseaiAccountResolver::new(v);
        let cancel = CancellationToken::new();
        for _ in 0..3 {
            let l = r.lease("", &[], &cancel).await.unwrap();
            assert_eq!(
                l.profile_dir(),
                Path::new("C:/p/tk-2"),
                "khong thu lai tk het han"
            );
        }
    }

    #[tokio::test]
    async fn lease_ghim_van_dung_duoc_tai_khoan_het_han_vi_nguoi_dung_chu_dich_chon() {
        let v = vault().await;
        let (a, _b) = two(&v).await;
        v.set_status(&a, "expired").await.unwrap();
        let r = VaultMuseaiAccountResolver::new(v);
        let l = r
            .lease("tk-1", &[], &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(l.profile_dir(), Path::new("C:/p/tk-1"));
    }
}
