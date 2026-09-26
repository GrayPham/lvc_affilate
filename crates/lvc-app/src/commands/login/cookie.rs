//! (A) Nạp cookie/phiên NHIỀU FILE vào kho credentials theo NỀN TẢNG — người dùng mang phiên đã đăng
//! nhập của chính mình. Mỗi file = một entry `session` (bí mật = nội dung file, nhãn = tên file). Một
//! file lỗi KHÔNG dừng cả lô. Đường HỢP LỆ: không đăng nhập tự động, không né bot.

use std::path::Path;

use lvc_browser::BrowserDriver;
use lvc_vault::AddCredential;

use crate::commands::{Ctx, IpcError};
use crate::state::AppState;

/// Kết quả nạp MỘT file trong lô.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginImportItem {
    pub label: String,
    pub ok: bool,
    pub error: Option<String>,
}

/// Nhãn an toàn từ tên file (bỏ đuôi).
fn label_from_path(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if stem.trim().is_empty() {
        "cookie".to_string()
    } else {
        stem
    }
}

/// Đọc nhiều file cookie (server-side) và thêm vào kho theo `provider` (kind `session`). Trả kết quả
/// từng file.
///
/// # Errors
/// Không lỗi cả lô (mỗi file có `ok`/`error` riêng).
pub async fn login_import_cookie_files_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    provider: &str,
    paths: &[String],
    proxy: &str,
) -> Result<Vec<LoginImportItem>, IpcError> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let label = label_from_path(path);
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => {
                out.push(LoginImportItem {
                    label,
                    ok: false,
                    error: Some(format!("đọc file lỗi: {e}")),
                });
                continue;
            }
        };
        let req = AddCredential {
            provider: provider.to_string(),
            kind: "session".to_string(),
            label: label.clone(),
            secret: content,
            meta: serde_json::json!({ "mode": "cookie" }),
            proxy: proxy.to_string(),
        };
        match ctx.vault.add(&req).await {
            Ok(_) => out.push(LoginImportItem {
                label,
                ok: true,
                error: None,
            }),
            Err(e) => out.push(LoginImportItem {
                label,
                ok: false,
                error: Some(IpcError::from(e).message),
            }),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn login_import_cookie_files(
    state: tauri::State<'_, AppState>,
    provider: String,
    paths: Vec<String>,
    proxy: String,
) -> Result<Vec<LoginImportItem>, IpcError> {
    login_import_cookie_files_impl(&state, &provider, &paths, &proxy).await
}
