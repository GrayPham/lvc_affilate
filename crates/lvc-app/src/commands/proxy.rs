//! Lenh "Kiem tra proxy" — nut o hop thoai "Proxy & trinh duyet"
//! (`references/lvc-ui-design/screens/AssignProxy.dc.html`).
//!
//! Khong can trinh duyet, khong ton credit Dola: chi noi thu qua proxy toi dich vu "IP cua ban"
//! roi tra IP ra + vi tri + do tre (xem `lvc_browser::proxy_probe`). Lenh khong dung `Ctx`/driver —
//! chi phu thuoc chuoi proxy nguoi dung go, nen khong generic tren `D`.

use lvc_browser::{ProxyProbe, proxy_probe::DEFAULT_TIMEOUT, test_proxy};
use lvc_core::parse_proxy;

use super::IpcError;

/// Parse chuoi proxy nguoi dung go roi noi thu. `""`/`"direct"` -> khong proxy -> do IP may (che
/// do "IP máy" o thiet ke). KHONG gop proxy chung: nut kiem tra DUNG chuoi dang go, de nguoi dung
/// thay chinh xac proxy minh nhap co chay khong.
///
/// # Errors
/// `INVALID_PROXY` khi chuoi sai dinh dang; `PROXY_TEST` khi khong noi duoc/qua thoi gian/dich vu
/// bao loi (thong diep da an mat khau proxy).
pub async fn proxy_test_impl(proxy: &str) -> Result<ProxyProbe, IpcError> {
    let spec =
        parse_proxy(proxy.trim()).map_err(|e| IpcError::new("INVALID_PROXY", e.to_string()))?;
    Ok(test_proxy(spec.as_ref(), DEFAULT_TIMEOUT).await?)
}

#[tauri::command]
pub async fn proxy_test(proxy: String) -> Result<ProxyProbe, IpcError> {
    proxy_test_impl(&proxy).await
}
