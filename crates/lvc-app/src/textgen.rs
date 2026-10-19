//! P4d: nối kho tài khoản (`Vault`) vào dịch vụ sinh text/ảnh/video API của engine — engine resolve
//! API key theo TÊN (label) credential người dùng khai ở node (`script_gen`, các host
//! seedance/grok/google/tts đều đi qua trait `TextSecretResolver` này).
//!
//! ## Tự xoay vòng key (setting `api_key_auto_rotate`)
//! Khi BẬT: nếu key được chọn đã chạm `daily_limit` trong ngày hoặc `status = "quota"`, resolver
//! tự chọn key KHÁC cùng nhà cung cấp còn hạn mức — đây là ĐIỂM DUY NHẤT chọn api_key (mọi host
//! media + text đều qua đây) nên chỉ cần cài ở một chỗ. Khi TẮT: giữ hành vi cũ (đúng label / key
//! bật đầu tiên), không quan tâm hạn mức.
//!
//! An toàn bí mật: API key CHỈ mượn qua `Vault::lease_secret` (giải mã nội bộ), trả cho engine để
//! gọi API rồi thôi. KHÔNG log key. `resolve` trả `Err(String)` là mô tả AN TOÀN (không chứa key).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use lvc_engine::{ResolvedSecret, TextSecretResolver};
use lvc_vault::{CredentialView, Vault};

/// Key còn hạn mức dùng được hôm nay: đang bật, là api_key, chưa bị quota, và chưa chạm ngưỡng
/// `daily_limit` (None/0 = không giới hạn). THUẦN.
fn has_quota(c: &CredentialView) -> bool {
    c.enabled
        && c.kind == "api_key"
        && c.status != "quota"
        && c.daily_limit
            .is_none_or(|lim| lim <= 0 || c.used_today < lim)
}

/// Chọn credential id để dùng — THUẦN (tách khỏi I/O để test). Trả về `&CredentialView`.
///
/// - `wanted` rỗng = tự chọn; có = ép đúng label.
/// - `auto_rotate` TẮT: đúng hành vi cũ (label khớp / key bật đầu tiên), bỏ qua hạn mức.
/// - `auto_rotate` BẬT: ưu tiên key còn hạn mức; nếu key được chỉ định đã hết hạn mức thì xoay sang
///   key khác cùng provider còn hạn mức; hết sạch thì báo lỗi rõ.
fn pick<'a>(
    list: &'a [CredentialView],
    provider: &str,
    wanted: &str,
    auto_rotate: bool,
) -> Result<&'a CredentialView, String> {
    let is_api = |c: &&CredentialView| c.enabled && c.kind == "api_key";
    if !auto_rotate {
        return list
            .iter()
            .find(|c| is_api(c) && c.label == wanted)
            .or_else(|| {
                wanted
                    .is_empty()
                    .then(|| list.iter().find(is_api))
                    .flatten()
            })
            .ok_or_else(|| not_found_msg(provider, wanted));
    }

    // Auto-rotate: nếu chỉ định label, thử key đó trước khi nó còn hạn mức.
    if !wanted.is_empty() {
        if let Some(c) = list.iter().find(|c| is_api(c) && c.label == wanted) {
            if has_quota(c) {
                return Ok(c);
            }
            // Key chỉ định hết hạn mức → xoay sang key khác cùng provider còn hạn mức.
            return list
                .iter()
                .find(|c| has_quota(c) && c.label != wanted)
                .ok_or_else(|| all_exhausted_msg(provider));
        }
        return Err(not_found_msg(provider, wanted));
    }

    // Tự chọn: key còn hạn mức đầu tiên.
    list.iter()
        .find(|c| has_quota(c))
        .ok_or_else(|| all_exhausted_msg(provider))
}

fn not_found_msg(provider: &str, wanted: &str) -> String {
    if wanted.is_empty() {
        format!("chua co API key '{provider}' nao duoc bat")
    } else {
        format!("khong tim thay API key '{wanted}' ({provider})")
    }
}

fn all_exhausted_msg(provider: &str) -> String {
    format!(
        "moi API key '{provider}' da het han muc hom nay (tat 'Tu xoay vong key' de chay bat chap, hoac them key)"
    )
}

/// Resolver backed by [`Vault`]: tra API key `kind = "api_key"` theo `provider` + `name` (label).
/// `auto_rotate` chia sẻ với cài đặt (cập nhật khi người dùng bật/tắt — xem `bootstrap`).
pub struct VaultTextResolver {
    vault: Arc<Vault>,
    auto_rotate: Arc<AtomicBool>,
}

impl VaultTextResolver {
    #[must_use]
    pub fn new(vault: Arc<Vault>, auto_rotate: Arc<AtomicBool>) -> Self {
        Self { vault, auto_rotate }
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
        let id = {
            let cred = pick(
                &list,
                provider,
                name.trim(),
                self.auto_rotate.load(Ordering::Relaxed),
            )?;
            cred.id.clone()
        };

        let (api_key, meta) = self
            .vault
            .lease_secret(&id)
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

    fn view(
        label: &str,
        used_today: i64,
        daily_limit: Option<i64>,
        status: &str,
    ) -> CredentialView {
        CredentialView {
            id: format!("id-{label}"),
            provider: "gemini".into(),
            kind: "api_key".into(),
            label: label.into(),
            secret_hint: String::new(),
            meta: serde_json::Value::Null,
            proxy_masked: String::new(),
            status: status.into(),
            enabled: true,
            daily_limit,
            used_today,
            last_used_at: 0.0,
            last_verified_at: 0.0,
        }
    }

    #[tokio::test]
    async fn resolve_theo_label() {
        let v = vault().await;
        v.add(&add("khoa-chinh", "AIza-1234")).await.unwrap();
        let r = VaultTextResolver::new(Arc::clone(&v), Arc::new(AtomicBool::new(false)));
        let out = r.resolve("gemini", "khoa-chinh").await.unwrap();
        assert_eq!(out.api_key, "AIza-1234");
        assert_eq!(out.base_url.as_deref(), Some("http://x"));
    }

    #[tokio::test]
    async fn ten_rong_lay_key_dau_tien() {
        let v = vault().await;
        v.add(&add("k1", "AIza-1")).await.unwrap();
        let r = VaultTextResolver::new(v, Arc::new(AtomicBool::new(false)));
        assert_eq!(r.resolve("gemini", "").await.unwrap().api_key, "AIza-1");
    }

    #[tokio::test]
    async fn khong_co_thi_bao_loi_khong_lo_key() {
        let v = vault().await;
        v.add(&add("k1", "AIza-secret")).await.unwrap();
        let r = VaultTextResolver::new(v, Arc::new(AtomicBool::new(false)));
        let err = r.resolve("gemini", "khong-co").await.unwrap_err();
        assert!(err.contains("khong tim thay"));
        assert!(!err.contains("AIza-secret"), "loi lo key: {err}");
    }

    // --- pick thuần ---

    #[test]
    fn tat_rotate_lay_dung_label_ke_ca_het_han_muc() {
        let list = vec![view("a", 100, Some(10), "ok"), view("b", 0, None, "ok")];
        let c = pick(&list, "gemini", "a", false).unwrap();
        assert_eq!(c.label, "a", "tat rotate: ton trong label du het han muc");
    }

    #[test]
    fn bat_rotate_label_het_han_muc_thi_xoay_sang_key_khac() {
        let list = vec![view("a", 100, Some(10), "ok"), view("b", 2, Some(10), "ok")];
        let c = pick(&list, "gemini", "a", true).unwrap();
        assert_eq!(c.label, "b", "xoay sang key con han muc");
    }

    #[test]
    fn bat_rotate_label_con_han_muc_thi_giu() {
        let list = vec![view("a", 3, Some(10), "ok"), view("b", 0, None, "ok")];
        let c = pick(&list, "gemini", "a", true).unwrap();
        assert_eq!(c.label, "a");
    }

    #[test]
    fn bat_rotate_tu_chon_bo_qua_key_quota_va_het_han() {
        let list = vec![
            view("a", 100, Some(10), "ok"), // het han muc
            view("b", 0, None, "quota"),    // bi quota
            view("c", 1, Some(10), "ok"),   // con dung duoc
        ];
        let c = pick(&list, "gemini", "", true).unwrap();
        assert_eq!(c.label, "c");
    }

    #[test]
    fn bat_rotate_het_sach_thi_bao_loi() {
        let list = vec![view("a", 100, Some(10), "ok"), view("b", 0, None, "quota")];
        let err = pick(&list, "gemini", "", true).unwrap_err();
        assert!(err.contains("het han muc"), "{err}");
    }
}
