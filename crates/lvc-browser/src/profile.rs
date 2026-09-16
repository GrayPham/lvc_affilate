//! Sửa `Default/Preferences` của profile Chromium TRƯỚC khi mở (port `disable_session_restore`).

use std::path::Path;

use crate::BrowserError;

/// Ghi `session.restore_on_startup=5`, `profile.exit_type="Normal"`, `profile.exited_cleanly=true`
/// để Chromium/Opera không mở lại tab cũ và không hiện "khôi phục trang". Giữ khoá khác.
/// Trả `true` nếu file thay đổi.
///
/// # Errors
/// I/O hoặc JSON hỏng.
pub async fn disable_session_restore(profile_dir: &Path) -> Result<bool, BrowserError> {
    let path = profile_dir.join("Default").join("Preferences");
    let mut root: serde_json::Value = match tokio::fs::read_to_string(&path).await {
        Ok(s) if !s.trim().is_empty() => serde_json::from_str(&s)?,
        _ => serde_json::json!({}),
    };
    let before = root.clone();
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
