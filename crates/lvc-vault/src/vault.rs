//! `Vault` — CRUD tài khoản AI có mã hoá. NƠI DUY NHẤT chạm plaintext bí mật: nhận plaintext từ
//! lệnh (secret đi VÀO), mã hoá bằng [`SecretBox`] rồi giao [`CredentialRepo`] lưu ciphertext; đọc
//! ra trả [`CredentialView`] ĐÃ CHE (secret KHÔNG đi ra). Chỉ `lease_secret` mới giải mã, dùng nội
//! bộ khi thực thi (node), không qua IPC.

use lvc_store::{CredentialRepo, CredentialRow, Db, NewCredential};
use serde::{Deserialize, Serialize};

use crate::{SecretBox, VaultError, mask};

/// Giây epoch hiện tại (cho `tokyo_day`).
fn now_unix() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Loại provider hỗ trợ (mở rộng dần). `kind` hợp lệ theo provider kiểm ở [`Vault::add`].
pub const PROVIDERS: [&str; 13] = [
    "dola",
    "google",
    "chatgpt_web",
    "openai",
    "gemini",
    "facebook",
    "apple",
    // P6: nha cung cap API chinh thuc moi (chi api_key) cho node seedance_*/grok_image.
    "seedance",
    "grok",
    // P-affiliate-multi-engine: kho tai khoan cho MuseAI (dang nhap/cookie, giong Google/Facebook —
    // CHUA co trang dang nhap thu cong (login_url) vi chua ro URL that, xem
    // `commands::login::platform`; chi dung duoc qua nap cookie hang loat/dan tay).
    "museai",
    // P-tts: API key cho node tts (text-to-speech) — chi api_key, giong seedance/grok.
    "elevenlabs",
    "minimax",
    // Publish workflow len kho cong dong: personal access token do CHINH nguoi dung tao, luu ma
    // hoa nhu API key (xem `crates/providers/lvc-github`).
    "github",
];

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
    /// Ngưỡng hạn mức/ngày; `None` = không giới hạn. UI cột "Hạn mức".
    pub daily_limit: Option<i64>,
    /// Số lần dùng trong ngày hiện tại (giờ Tokyo); 0 nếu đã sang ngày mới mà chưa dùng.
    pub used_today: i64,
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
        "openai" | "seedance" | "grok" | "elevenlabs" | "minimax" | "github" => kind == "api_key",
        // Gemini: API key (chính thức/endpoint) HOẶC đăng nhập/cookie (lấy profile Google).
        "gemini" => kind == "api_key" || kind == "login" || kind == "session",
        "google" | "chatgpt_web" | "facebook" | "apple" | "museai" => {
            kind == "login" || kind == "session"
        }
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
            daily_limit: row.daily_limit,
            // Bộ đếm chỉ có nghĩa cho NGÀY hiện tại — đã sang ngày khác thì coi như 0 (chưa dùng
            // hôm nay), khớp cách `touch_used` sẽ reset ở lần dùng kế.
            used_today: if row.used_day == lvc_core::tokyo_day(now_unix()) {
                row.used_today
            } else {
                0
            },
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

    /// Đặt ngưỡng hạn mức/ngày cho api_key (`None`/`Some(0)` = không giới hạn).
    ///
    /// # Errors
    /// [`VaultRepoError`] khi ghi lỗi.
    pub async fn set_daily_limit(&self, id: &str, limit: Option<i64>) -> Result<(), VaultRepoError> {
        // Chuẩn hoá 0 -> None (không giới hạn) để UI/engine chỉ cần kiểm `Some(>0)`.
        let limit = limit.filter(|&n| n > 0);
        self.repo.set_daily_limit(id, limit).await?;
        Ok(())
    }

    /// # Errors
    /// [`VaultRepoError`] khi ghi lỗi.
    pub async fn set_status(&self, id: &str, status: &str) -> Result<(), VaultRepoError> {
        self.repo.set_status(id, status).await?;
        Ok(())
    }

    /// Đặt TAG LOẠI NODE (`"video"` | `"image"` | `"script"`) cho credential — node AI chỉ lấy tài
    /// khoản/key có đúng tag (bản bàn giao UI, mục "Quy tắc nghiệp vụ"). Lưu vào `meta.node_types`
    /// (cột `meta` đã là JSON nên KHÔNG cần migration); giá trị lạ bị loại bỏ, trùng bị gộp.
    ///
    /// Ghi `meta` qua [`crate::repo::VaultRepo::update_secret`] với ciphertext CŨ — không đụng tới
    /// bí mật, không cần giải mã (đúng luật "nơi duy nhất chạm plaintext").
    ///
    /// # Errors
    /// [`VaultRepoError::NotFound`] khi `id` không có; lỗi ghi DB.
    pub async fn set_node_types(&self, id: &str, kinds: &[String]) -> Result<(), VaultRepoError> {
        let row = self.repo.get(id).await?.ok_or(VaultRepoError::NotFound)?;
        let mut meta: serde_json::Value =
            serde_json::from_str(&row.meta).unwrap_or(serde_json::Value::Null);
        if !meta.is_object() {
            meta = serde_json::json!({});
        }
        let mut clean: Vec<String> = Vec::new();
        for k in kinds {
            let k = k.trim();
            if matches!(k, "video" | "image" | "script") && !clean.iter().any(|c| c == k) {
                clean.push(k.to_string());
            }
        }
        // `as_object_mut` chắc chắn `Some` vì vừa ép thành object ở trên.
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("node_types".to_string(), serde_json::json!(clean));
        }
        self.repo
            .update_secret(id, &row.secret, &meta.to_string())
            .await?;
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
        // Tăng bộ đếm hạn mức theo ngày Tokyo (đồng bộ với tài khoản Dola). Mọi node dùng key đều
        // qua đây nên đếm đúng dù node nào gọi.
        let today = lvc_core::tokyo_day(now_unix());
        self.repo.touch_used(id, &today).await?;
        let meta = serde_json::from_str(&row.meta).unwrap_or(serde_json::Value::Null);
        Ok((String::from_utf8_lossy(&plain).into_owned(), meta))
    }

    /// `provider` + thư mục profile (từ `meta.profile_dir`, `None` nếu credential này chưa từng
    /// tiêm cookie vào profile thật — xem `lvc-engine::profiles`) + proxy THÔ (chưa che) của
    /// credential — dùng cho "Mở trình duyệt xem". KHÔNG chạm bí mật (không giải mã `secret`,
    /// không `touch_used` — xem profile không phải "dùng" bí mật theo nghĩa [`Self::lease_secret`]).
    ///
    /// # Errors
    /// [`VaultRepoError::NotFound`] nếu không có credential nào với `id` này.
    /// Trả `(provider, profile_dir, proxy, engine)` để "Mở trình duyệt" chọn đúng nhánh.
    /// `engine` = `meta.engine` (chuỗi thô; rỗng khi chưa gắn → caller coi là "chromium"); KHÔNG
    /// chạm secret.
    pub async fn profile_dir_and_proxy(
        &self,
        id: &str,
    ) -> Result<(String, Option<String>, String, String), VaultRepoError> {
        let row = self.repo.get(id).await?.ok_or(VaultRepoError::NotFound)?;
        let meta: serde_json::Value =
            serde_json::from_str(&row.meta).unwrap_or(serde_json::Value::Null);
        let profile_dir = meta
            .get("profile_dir")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let engine = meta
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        Ok((row.provider, profile_dir, row.proxy, engine))
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
    async fn profile_dir_and_proxy_doc_tu_meta_khong_cham_secret() {
        let v = vault().await;
        let mut req = add("google", "session", "cookie-that-su-nhay-cam");
        req.meta = serde_json::json!({"mode": "cookie", "profile_dir": "/tmp/profile-x"});
        req.proxy = "1.2.3.4:8080".to_string();
        let view = v.add(&req).await.unwrap();

        let (provider, profile_dir, proxy, engine) =
            v.profile_dir_and_proxy(&view.id).await.unwrap();
        assert_eq!(provider, "google");
        assert_eq!(profile_dir, Some("/tmp/profile-x".to_string()));
        assert_eq!(proxy, "1.2.3.4:8080");
        assert_eq!(
            engine, "",
            "meta khong co 'engine' -> rong (caller coi la chromium)"
        );
    }

    #[tokio::test]
    async fn profile_dir_and_proxy_doc_engine_firefox_tu_meta() {
        let v = vault().await;
        let mut req = add("museai", "session", "cookie-nhay-cam");
        req.meta = serde_json::json!({"mode": "cookie", "engine": "firefox_portable", "profile_dir": "/tmp/ff"});
        let view = v.add(&req).await.unwrap();
        let (_, _, _, engine) = v.profile_dir_and_proxy(&view.id).await.unwrap();
        assert_eq!(engine, "firefox_portable");
    }

    #[tokio::test]
    async fn set_node_types_loc_gia_tri_la_va_gop_trung() {
        let v = vault().await;
        let view = v.add(&add("gemini", "api_key", "AIzaKEY")).await.unwrap();

        v.set_node_types(
            &view.id,
            &[
                "image".to_string(),
                "khong-ton-tai".to_string(),
                "image".to_string(),
                "script".to_string(),
            ],
        )
        .await
        .unwrap();

        let rows = v.list(Some("gemini")).await.unwrap();
        assert_eq!(
            rows[0].meta["node_types"],
            serde_json::json!(["image", "script"])
        );
    }

    #[tokio::test]
    async fn set_node_types_giu_nguyen_bi_mat_va_meta_cu() {
        let v = vault().await;
        let mut req = add("gemini", "api_key", "AIzaKEY-rat-nhay-cam");
        req.meta = serde_json::json!({"base_url": "https://vi.du"});
        let view = v.add(&req).await.unwrap();

        v.set_node_types(&view.id, &["video".to_string()])
            .await
            .unwrap();

        // Bi mat van giai ma lai dung (khong bi ghi de boi buoc cap nhat meta).
        let (plain, meta) = v.lease_secret(&view.id).await.unwrap();
        assert_eq!(plain, "AIzaKEY-rat-nhay-cam");
        // Truong meta cu con nguyen, truong moi duoc them.
        assert_eq!(meta["base_url"], "https://vi.du");
        assert_eq!(meta["node_types"], serde_json::json!(["video"]));
    }

    #[tokio::test]
    async fn set_node_types_id_khong_co_bao_not_found() {
        let v = vault().await;
        let err = v
            .set_node_types("khong-co", &["video".to_string()])
            .await
            .unwrap_err();
        assert!(matches!(err, VaultRepoError::NotFound));
    }

    #[tokio::test]
    async fn profile_dir_and_proxy_chua_co_profile_tra_none() {
        let v = vault().await;
        let view = v.add(&add("gemini", "api_key", "AIzaKEY")).await.unwrap();
        let (_, profile_dir, _, _) = v.profile_dir_and_proxy(&view.id).await.unwrap();
        assert_eq!(profile_dir, None);
    }

    #[tokio::test]
    async fn profile_dir_and_proxy_khong_ton_tai_bao_not_found() {
        let v = vault().await;
        assert!(matches!(
            v.profile_dir_and_proxy("ghost").await.unwrap_err(),
            VaultRepoError::NotFound
        ));
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
