//! `Vault` — CRUD tài khoản AI có mã hoá. NƠI DUY NHẤT chạm plaintext bí mật: nhận plaintext từ
//! lệnh (secret đi VÀO), mã hoá bằng [`SecretBox`] rồi giao [`CredentialRepo`] lưu ciphertext; đọc
//! ra trả [`CredentialView`] ĐÃ CHE (secret KHÔNG đi ra). Chỉ `lease_secret` mới giải mã, dùng nội
//! bộ khi thực thi (node), không qua IPC.

use lvc_store::{CredentialRepo, CredentialRow, Db, NewCredential};
use serde::{Deserialize, Serialize};

use crate::{SecretBox, VaultError, mask};

/// Loại provider hỗ trợ (mở rộng dần). `kind` hợp lệ theo provider kiểm ở [`Vault::add`].
pub const PROVIDERS: [&str; 5] = ["dola", "google", "chatgpt_web", "openai", "gemini"];

/// Lỗi kho tài khoản (có `code()` ổn định để map sang IPC như `AccountError`).
#[derive(Debug, thiserror::Error)]
pub enum VaultRepoError {
    #[error("provider khong ho tro: {0}")]
    BadProvider(String),
    #[error("kind khong hop le cho provider: {0}")]
    BadKind(String),
    #[error("nhan (label) khong hop le")]
    BadLabel,
    #[error("bi mat rong")]
    EmptySecret,
    #[error("khong tim thay tai khoan")]
    NotFound,
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("store: {0}")]
    Store(String),
}

impl From<lvc_store::StoreError> for VaultRepoError {
    fn from(e: lvc_store::StoreError) -> Self {
        Self::Store(e.to_string())
    }
}

impl VaultRepoError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadProvider(_) => "CRED_BAD_PROVIDER",
            Self::BadKind(_) => "CRED_BAD_KIND",
            Self::BadLabel => "CRED_BAD_LABEL",
            Self::EmptySecret => "CRED_EMPTY_SECRET",
            Self::NotFound => "CRED_NOT_FOUND",
            Self::Vault(_) => "CRED_CRYPTO",
            Self::Store(_) => "STORE",
        }
    }
}

/// Bản ghi tài khoản hiển thị cho UI — ĐÃ CHE, KHÔNG có plaintext bí mật.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialView {
    pub id: String,
    pub provider: String,
    pub kind: String,
    pub label: String,
    /// Gợi ý bí mật đã che (`••••1234` cho api key; mô tả cookie cho session).
    pub secret_hint: String,
    /// `meta` JSON KHÔNG nhạy cảm (đã lọc bỏ trường bí mật nếu có).
    pub meta: serde_json::Value,
    pub proxy_masked: String,
    pub status: String,
    pub enabled: bool,
    pub last_used_at: f64,
    pub last_verified_at: f64,
}

/// Yêu cầu thêm tài khoản (secret đi VÀO).
#[derive(Debug, Clone)]
pub struct AddCredential {
    pub provider: String,
    pub kind: String,
    pub label: String,
    /// Plaintext (api key / cookie JSON / mật khẩu...) — sẽ mã hoá ngay.
    pub secret: String,
    pub meta: serde_json::Value,
    pub proxy: String,
}

fn valid_label(s: &str) -> bool {
    let n = s.chars().count();
    (1..=64).contains(&n) && s.chars().all(|c| !c.is_control())
}

/// `kind` cho phép theo provider.
fn kind_ok(provider: &str, kind: &str) -> bool {
    match provider {
        "dola" => kind == "session",
        "openai" | "gemini" => kind == "api_key",
        "google" | "chatgpt_web" => kind == "login" || kind == "session",
        _ => false,
    }
}

/// Che secret cho view theo `kind`.
fn hint(kind: &str, plain: &[u8]) -> String {
    if kind == "session" {
        // cookie JSON -> dem cookie + co sessionid khong (khong lo gia tri)
        let (count, has_sid) = cookie_summary(plain);
        mask::describe_cookies(count, has_sid)
    } else {
        mask::mask_key(&String::from_utf8_lossy(plain))
    }
}

/// Đếm cookie + có `sessionid` — chấp nhận cả `[{...}]` lẫn `{"cookies":[...]}`.
fn cookie_summary(plain: &[u8]) -> (usize, bool) {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(plain) else {
        return (0, false);
    };
    let arr = v
        .as_array()
        .or_else(|| v.get("cookies").and_then(serde_json::Value::as_array));
    let Some(list) = arr else {
        return (0, false);
    };
    let has_sid = list
        .iter()
        .any(|c| c.get("name").and_then(serde_json::Value::as_str) == Some("sessionid"));
    (list.len(), has_sid)
}

/// Kho tài khoản AI: repo + hộp mã hoá.
pub struct Vault {
    repo: CredentialRepo,
    secret_box: std::sync::Arc<dyn SecretBox>,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("secret_box", &self.secret_box.kind())
            .finish_non_exhaustive()
    }
}

impl Vault {
    #[must_use]
    pub fn new(db: &Db, secret_box: std::sync::Arc<dyn SecretBox>) -> Self {
        Self {
            repo: CredentialRepo::new(db),
            secret_box,
        }
    }

    fn view(&self, row: &CredentialRow) -> Result<CredentialView, VaultRepoError> {
        let plain = self.secret_box.decrypt(&row.secret)?;
        let meta: serde_json::Value =
            serde_json::from_str(&row.meta).unwrap_or(serde_json::Value::Null);
        Ok(CredentialView {
            id: row.id.clone(),
            provider: row.provider.clone(),
            kind: row.kind.clone(),
            label: row.label.clone(),
            secret_hint: hint(&row.kind, &plain),
            meta,
            proxy_masked: lvc_core::mask_proxy(&row.proxy),
            status: row.status.clone(),
            enabled: row.enabled,
            last_used_at: row.last_used_at,
            last_verified_at: row.last_verified_at,
        })
    }

    /// Thêm tài khoản (mã hoá secret trước khi lưu).
    ///
    /// # Errors
    /// [`VaultRepoError`] khi provider/kind/label không hợp lệ, secret rỗng, hoặc lỗi mã hoá/lưu.
    pub async fn add(&self, req: &AddCredential) -> Result<CredentialView, VaultRepoError> {
        if !PROVIDERS.contains(&req.provider.as_str()) {
            return Err(VaultRepoError::BadProvider(req.provider.clone()));
        }
        if !kind_ok(&req.provider, &req.kind) {
            return Err(VaultRepoError::BadKind(req.kind.clone()));
        }
        if !valid_label(&req.label) {
            return Err(VaultRepoError::BadLabel);
        }
        if req.secret.trim().is_empty() {
            return Err(VaultRepoError::EmptySecret);
        }
        let secret = self.secret_box.encrypt(req.secret.as_bytes())?;
        let id = uuid::Uuid::new_v4().to_string();
        let new = NewCredential {
            id: id.clone(),
            provider: req.provider.clone(),
            kind: req.kind.clone(),
            label: req.label.clone(),
            secret,
            meta: req.meta.to_string(),
            proxy: req.proxy.clone(),
        };
        self.repo.insert(&new).await?;
        let row = self.repo.get(&id).await?.ok_or(VaultRepoError::NotFound)?;
        self.view(&row)
    }

    /// Liệt kê view đã che.
    ///
    /// # Errors
    /// [`VaultRepoError`] khi truy vấn hoặc giải mã lỗi.
    pub async fn list(
        &self,
        provider: Option<&str>,
    ) -> Result<Vec<CredentialView>, VaultRepoError> {
        let rows = self.repo.list(provider).await?;
        rows.iter().map(|r| self.view(r)).collect()
    }

    /// Đổi bí mật mới (mã hoá lại).
    ///
    /// # Errors
    /// [`VaultRepoError`] khi rỗng/không tìm thấy/lỗi mã hoá.
    pub async fn update_secret(&self, id: &str, secret: &str) -> Result<(), VaultRepoError> {
        if secret.trim().is_empty() {
            return Err(VaultRepoError::EmptySecret);
        }
        let row = self.repo.get(id).await?.ok_or(VaultRepoError::NotFound)?;
        let enc = self.secret_box.encrypt(secret.as_bytes())?;
        self.repo.update_secret(id, &enc, &row.meta).await?;
        Ok(())
    }

    /// # Errors
    /// [`VaultRepoError`] khi ghi lỗi.
    pub async fn set_enabled(&self, id: &str, on: bool) -> Result<(), VaultRepoError> {
        self.repo.set_enabled(id, on).await?;
        Ok(())
    }

    /// # Errors
    /// [`VaultRepoError`] khi ghi lỗi.
    pub async fn set_label(&self, id: &str, label: &str) -> Result<(), VaultRepoError> {
        if !valid_label(label) {
            return Err(VaultRepoError::BadLabel);
        }
        self.repo.set_label(id, label).await?;
        Ok(())
    }

    /// # Errors
    /// [`VaultRepoError`] khi ghi lỗi.
    pub async fn set_proxy(&self, id: &str, proxy: &str) -> Result<(), VaultRepoError> {
        self.repo.set_proxy(id, proxy).await?;
        Ok(())
    }

    /// # Errors
    /// [`VaultRepoError`] khi ghi lỗi.
    pub async fn set_status(&self, id: &str, status: &str) -> Result<(), VaultRepoError> {
        self.repo.set_status(id, status).await?;
        Ok(())
    }

    /// # Errors
    /// [`VaultRepoError`] khi xoá lỗi.
    pub async fn delete(&self, id: &str) -> Result<bool, VaultRepoError> {
        Ok(self.repo.delete(id).await?)
    }

    /// Kiểm tra một credential và ghi `status`. `api_key` (openai/gemini) → gọi list-models bằng
    /// key; provider/kind khác → `unknown`. reqwest nằm TRONG crate này (lvc-app không cần).
    ///
    /// # Errors
    /// [`VaultRepoError`] khi không tìm thấy hoặc giải mã/ghi lỗi.
    pub async fn verify(&self, id: &str) -> Result<crate::VerifyOutcome, VaultRepoError> {
        let row = self.repo.get(id).await?.ok_or(VaultRepoError::NotFound)?;
        let outcome = if row.kind == "api_key" {
            let plain = self.secret_box.decrypt(&row.secret)?;
            let meta: serde_json::Value =
                serde_json::from_str(&row.meta).unwrap_or(serde_json::Value::Null);
            let base = meta
                .get("base_url")
                .and_then(serde_json::Value::as_str)
                .filter(|s| !s.is_empty());
            let client = reqwest::Client::new();
            crate::verify_api_key(
                &client,
                &row.provider,
                &String::from_utf8_lossy(&plain),
                base,
            )
            .await
        } else {
            crate::VerifyOutcome {
                status: "unknown".to_string(),
                detail: "kiem bang trinh duyet - chua ho tro o P4c".to_string(),
            }
        };
        self.repo.set_status(id, &outcome.status).await?;
        Ok(outcome)
    }

    /// Mượn plaintext bí mật để THỰC THI (node) — KHÔNG đi qua IPC. `meta` kèm theo.
    ///
    /// # Errors
    /// [`VaultRepoError`] khi không tìm thấy hoặc giải mã lỗi.
    pub async fn lease_secret(
        &self,
        id: &str,
    ) -> Result<(String, serde_json::Value), VaultRepoError> {
        let row = self.repo.get(id).await?.ok_or(VaultRepoError::NotFound)?;
        let plain = self.secret_box.decrypt(&row.secret)?;
        self.repo.touch_used(id).await?;
        let meta = serde_json::from_str(&row.meta).unwrap_or(serde_json::Value::Null);
        Ok((String::from_utf8_lossy(&plain).into_owned(), meta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlainBox;
    use std::sync::Arc;

    async fn vault() -> Vault {
        let db = Db::open_memory().await.unwrap();
        Vault::new(&db, Arc::new(PlainBox))
    }

    fn add(provider: &str, kind: &str, secret: &str) -> AddCredential {
        AddCredential {
            provider: provider.into(),
            kind: kind.into(),
            label: format!("{provider}-1"),
            secret: secret.into(),
            meta: serde_json::json!({"base_url": "x"}),
            proxy: String::new(),
        }
    }

    #[tokio::test]
    async fn them_api_key_view_da_che_khong_lo_plaintext() {
        let v = vault().await;
        let view = v
            .add(&add("openai", "api_key", "sk-secret-key-1234"))
            .await
            .unwrap();
        assert_eq!(view.secret_hint, "••••1234");
        assert_eq!(view.provider, "openai");
        assert!(view.enabled);
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("secret-key"), "view lo plaintext: {json}");
    }

    #[tokio::test]
    async fn session_dola_che_dang_mo_ta_cookie() {
        let v = vault().await;
        let cookie = r#"[{"name":"sessionid","value":"XXX"},{"name":"uid_tt","value":"9"}]"#;
        let view = v.add(&add("dola", "session", cookie)).await.unwrap();
        assert_eq!(view.secret_hint, "2 cookie · có sessionid");
        assert!(!serde_json::to_string(&view).unwrap().contains("XXX"));
    }

    #[tokio::test]
    async fn provider_kind_label_secret_kiem_dung() {
        let v = vault().await;
        assert_eq!(
            v.add(&add("khong-co", "api_key", "k"))
                .await
                .unwrap_err()
                .code(),
            "CRED_BAD_PROVIDER"
        );
        assert_eq!(
            v.add(&add("openai", "session", "k"))
                .await
                .unwrap_err()
                .code(),
            "CRED_BAD_KIND"
        );
        assert_eq!(
            v.add(&add("openai", "api_key", "   "))
                .await
                .unwrap_err()
                .code(),
            "CRED_EMPTY_SECRET"
        );
        let mut bad = add("openai", "api_key", "k");
        bad.label = String::new();
        assert_eq!(v.add(&bad).await.unwrap_err().code(), "CRED_BAD_LABEL");
    }

    #[tokio::test]
    async fn lease_secret_tra_plaintext_cho_thuc_thi() {
        let v = vault().await;
        let view = v.add(&add("gemini", "api_key", "AIzaKEY")).await.unwrap();
        let (plain, meta) = v.lease_secret(&view.id).await.unwrap();
        assert_eq!(plain, "AIzaKEY");
        assert_eq!(meta["base_url"], "x");
    }

    #[tokio::test]
    async fn update_secret_va_delete() {
        let v = vault().await;
        let view = v
            .add(&add("openai", "api_key", "old-key-1111"))
            .await
            .unwrap();
        v.update_secret(&view.id, "new-key-2222").await.unwrap();
        let (plain, _) = v.lease_secret(&view.id).await.unwrap();
        assert_eq!(plain, "new-key-2222");
        assert!(v.delete(&view.id).await.unwrap());
        assert!(matches!(
            v.lease_secret(&view.id).await.unwrap_err(),
            VaultRepoError::NotFound
        ));
    }
}
