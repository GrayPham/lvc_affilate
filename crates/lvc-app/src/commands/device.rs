//! Mã thiết bị — một chuỗi ổn định cho MỘT lần cài đặt trên MỘT máy.
//!
//! Dùng để người dùng gửi kèm khi báo lỗi ("máy tôi mã XXXX"), và là chỗ móc sẵn cho hệ thống bản
//! quyền về sau (chưa có — xem `CLAUDE.md`).
//!
//! ## Vì sao tự sinh chứ không đọc số máy của Windows
//! `MachineGuid` trong registry là định danh của CẢ MÁY, dùng được để theo dõi người dùng xuyên
//! ứng dụng — lấy nó là thu thập nhiều hơn mức cần. Một UUID tự sinh rồi ghi vào thư mục dữ liệu
//! của chính app đáp ứng đúng nhu cầu ("phân biệt các lần cài đặt") mà không chạm gì tới phần còn
//! lại của máy. Xoá thư mục dữ liệu = mã mới, đúng như kỳ vọng.

use std::path::{Path, PathBuf};

use lvc_browser::BrowserDriver;
use uuid::Uuid;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Tên file giữ mã thiết bị trong thư mục dữ liệu ứng dụng.
const DEVICE_FILE: &str = "device_id";

fn device_path(base: &Path) -> PathBuf {
    base.join(DEVICE_FILE)
}

/// Đọc mã thiết bị; chưa có thì SINH rồi ghi lại. Luôn trả cùng một giá trị cho các lần gọi sau.
///
/// File rỗng/hỏng (vd đĩa đầy giữa chừng lần ghi trước) được coi như chưa có → sinh lại, thay vì
/// trả về một chuỗi rác cho người dùng đọc.
///
/// # Errors
/// `IpcError` mã `IO` khi không ghi được file (thư mục chỉ đọc).
pub fn device_id_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>) -> Result<String, IpcError> {
    read_or_create(&ctx.base_dir)
}

/// Thân thuần (nhận thư mục qua tham số) — test được với `tempfile`, không cần runtime Tauri.
fn read_or_create(base: &Path) -> Result<String, IpcError> {
    let path = device_path(base);
    if let Ok(raw) = std::fs::read_to_string(&path) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    let id = Uuid::new_v4().simple().to_string().to_uppercase();
    std::fs::create_dir_all(base).map_err(|e| IpcError::new("IO", e.to_string()))?;
    std::fs::write(&path, &id).map_err(|e| IpcError::new("IO", e.to_string()))?;
    Ok(id)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "chu ky `#[tauri::command]` bat buoc nhan State theo gia tri - bien gioi IPC"
)]
#[tauri::command]
pub fn device_id(state: tauri::State<'_, AppState>) -> Result<String, IpcError> {
    device_id_impl(&state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sinh_lan_dau_roi_giu_nguyen_o_cac_lan_sau() {
        let dir = tempfile::tempdir().unwrap();
        let first = read_or_create(dir.path()).unwrap();
        assert!(!first.is_empty());
        for _ in 0..3 {
            assert_eq!(
                read_or_create(dir.path()).unwrap(),
                first,
                "ma phai ON DINH"
            );
        }
    }

    #[test]
    fn hai_thu_muc_khac_nhau_ra_ma_khac_nhau() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        assert_ne!(
            read_or_create(a.path()).unwrap(),
            read_or_create(b.path()).unwrap()
        );
    }

    #[test]
    fn file_rong_coi_nhu_chua_co_sinh_lai() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(device_path(dir.path()), "   \n").unwrap();
        let id = read_or_create(dir.path()).unwrap();
        assert!(
            !id.trim().is_empty(),
            "khong duoc tra chuoi rac cho nguoi dung"
        );
    }

    #[test]
    fn bo_khoang_trang_thua_khi_doc_lai() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(device_path(dir.path()), "  ABC123  \n").unwrap();
        assert_eq!(read_or_create(dir.path()).unwrap(), "ABC123");
    }

    #[test]
    fn ma_la_chu_hoa_khong_dau_gach_de_doc_qua_dien_thoai() {
        let dir = tempfile::tempdir().unwrap();
        let id = read_or_create(dir.path()).unwrap();
        assert_eq!(id.len(), 32, "uuid dang `simple` = 32 ky tu");
        assert!(
            id.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        );
    }

    #[tokio::test]
    async fn doc_qua_ctx_ra_dung_file_trong_thu_muc_du_lieu() {
        let (dir, ctx, _d) = crate::commands::test_support::ctx_with(vec![], &[]).await;
        let id = device_id_impl(&ctx).unwrap();
        let on_disk = std::fs::read_to_string(dir.path().join(DEVICE_FILE)).unwrap();
        assert_eq!(on_disk.trim(), id);
    }
}
