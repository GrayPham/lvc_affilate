//! Sửa `Default/Preferences` của profile Chromium TRƯỚC khi mở (port `disable_session_restore`).
//!
//! M-7: 4 điểm lệch có chủ ý so với Python `browser.py:20-64` (`_prepare_profile_preferences`):
//! 1. Tạo mới `Default/Preferences` khi chưa tồn tại; Python chỉ sửa file ĐÃ có, bỏ qua nếu
//!    thiếu (profile mới do `add_account.py` tạo luôn đã có file, nên khác biệt không ảnh
//!    hưởng luồng chính, chỉ khác khi gọi trên thư mục hoàn toàn rỗng).
//! 2. Không xử lý profile Opera (`<profile>/Preferences` ở gốc, không có thư mục `Default/`);
//!    Python có nhánh riêng cho Opera. P1 chỉ nhắm Chromium/patchright nên chưa cần.
//! 3. Không `purge_session_files` (xoá `Sessions/`, `Session Storage/…` cũ); Python xoá để
//!    tránh Chromium tự hỏi "khôi phục trang" bằng UI dù đã tắt cờ.
//! 4. JSON hỏng: BỎ QUA (log rồi trả `Ok(false)`, không sửa, không fail `launch()`) — khớp
//!    Python (`json.JSONDecodeError` bị bắt và bỏ qua ở `_prepare_profile_preferences`), thay
//!    vì lan lỗi `serde_json::Error` lên `launch()` như trước bản vá này.

use std::path::Path;

use crate::BrowserError;

/// Nếu `root[key]` đã tồn tại nhưng KHÔNG phải object, `serde_json` `IndexMut` sẽ panic khi
/// gán tiếp một cấp nữa (chỉ tự-object-hoá `Value::Null`, còn lại — string/number/bool/array —
/// panic "cannot access key ... in JSON ..."). Thay bằng object rỗng trước khi gán sâu hơn.
fn ensure_object_field(root: &mut serde_json::Value, key: &str) {
    if !root[key].is_object() {
        root[key] = serde_json::json!({});
    }
}

/// Ghi `session.restore_on_startup=5`, `profile.exit_type="Normal"`, `profile.exited_cleanly=true`
/// để Chromium/Opera không mở lại tab cũ và không hiện "khôi phục trang". Giữ khoá khác.
/// Trả `true` nếu file thay đổi; `false` nếu không đổi gì (kể cả khi JSON hỏng bị bỏ qua).
///
/// # Errors
/// I/O khi đọc/ghi file (JSON hỏng KHÔNG còn là lỗi — xem điểm lệch 4 ở doc module).
pub async fn disable_session_restore(profile_dir: &Path) -> Result<bool, BrowserError> {
    let path = profile_dir.join("Default").join("Preferences");
    let mut root: serde_json::Value = match tokio::fs::read_to_string(&path).await {
        Ok(s) if !s.trim().is_empty() => match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(e) => {
                // M-7: Preferences hỏng thường do Chrome bị kill giữa lúc ghi — Chromium tự
                // ghi lại file bình thường ở lần khởi động sau, không cần ta sửa liều.
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Preferences hong, bo qua khong sua (khop Python browser.py:20-64)"
                );
                return Ok(false);
            }
        },
        _ => serde_json::json!({}),
    };
    // Neu top-level khong phai object (file chi chua vd "\"foo\"" hay "42") thi coi nhu chua
    // co gi, tao object moi — tranh panic khi gan root["session"] ben duoi.
    if !root.is_object() {
        root = serde_json::json!({});
    }
    let before = root.clone();
    ensure_object_field(&mut root, "session");
    ensure_object_field(&mut root, "profile");
    root["session"]["restore_on_startup"] = serde_json::json!(5);
    root["profile"]["exit_type"] = serde_json::json!("Normal");
    root["profile"]["exited_cleanly"] = serde_json::json!(true);
    if root == before {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serde_json::to_string(&root)?).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tao_preferences_moi_khi_chua_co() {
        let dir = tempfile::tempdir().unwrap();
        let changed = disable_session_restore(dir.path()).await.unwrap();
        assert!(changed);
        let p = dir.path().join("Default").join("Preferences");
        let v: serde_json::Value =
            serde_json::from_str(&tokio::fs::read_to_string(&p).await.unwrap()).unwrap();
        assert_eq!(v["session"]["restore_on_startup"], 5);
        assert_eq!(v["profile"]["exit_type"], "Normal");
        assert_eq!(v["profile"]["exited_cleanly"], true);
    }

    /// M-7: Preferences hong (JSON khong parse duoc) truoc day lam `disable_session_restore`
    /// tra `Err` qua `?` (`serde_json::Error` -> `BrowserError::Json`), fail ca `launch()`.
    /// Python (browser.py:20-64) BO QUA file hong. Phai tra `Ok(false)` va KHONG sua file.
    #[tokio::test]
    async fn json_hong_thi_bo_qua_khong_loi() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Default").join("Preferences");
        tokio::fs::create_dir_all(p.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&p, "{khong phai json hop le")
            .await
            .unwrap();

        let changed = disable_session_restore(dir.path())
            .await
            .expect("JSON hong phai duoc bo qua, khong fail launch");
        assert!(!changed, "khong sua file hong");

        let raw = tokio::fs::read_to_string(&p).await.unwrap();
        assert_eq!(raw, "{khong phai json hop le", "file hong giu nguyen");
    }

    /// M-7: `root["session"][key] = ..` panic neu `root["session"]` da ton tai va KHONG phai
    /// object (`serde_json` `IndexMut` chi tu-object-hoa gia tri `Null`, con lai panic). Phai
    /// thay the bang object moi thay vi panic.
    #[tokio::test]
    async fn session_khong_phai_object_thi_thay_the_khong_panic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Default").join("Preferences");
        tokio::fs::create_dir_all(p.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&p, r#"{"session":"khong-phai-object","profile":42}"#)
            .await
            .unwrap();

        let changed = disable_session_restore(dir.path())
            .await
            .expect("khong duoc panic hay loi");
        assert!(changed);

        let v: serde_json::Value =
            serde_json::from_str(&tokio::fs::read_to_string(&p).await.unwrap()).unwrap();
        assert_eq!(v["session"]["restore_on_startup"], 5);
        assert_eq!(v["profile"]["exit_type"], "Normal");
        assert_eq!(v["profile"]["exited_cleanly"], true);
    }

    #[tokio::test]
    async fn giu_khoa_khac_va_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("Default").join("Preferences");
        tokio::fs::create_dir_all(p.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(
            &p,
            r#"{"other":{"keep":1},"session":{"restore_on_startup":1}}"#,
        )
        .await
        .unwrap();
        assert!(disable_session_restore(dir.path()).await.unwrap());
        assert!(
            !disable_session_restore(dir.path()).await.unwrap(),
            "lan 2 khong doi gi"
        );
        let v: serde_json::Value =
            serde_json::from_str(&tokio::fs::read_to_string(&p).await.unwrap()).unwrap();
        assert_eq!(v["other"]["keep"], 1);
        assert_eq!(v["session"]["restore_on_startup"], 5);
    }
}
