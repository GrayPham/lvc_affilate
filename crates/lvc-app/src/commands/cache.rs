//! Lệnh cho **thư mục tạm** (nhóm "Lưu trữ" trong màn Cài đặt): xem đang chiếm bao nhiêu, dọn
//! ngay, và tự dọn file quá cũ lúc khởi động.
//!
//! Thư mục tạm là `app_data_dir/cache` (xem [`crate::paths::cache_dir`]) — chứa ảnh/cảnh lẻ tải về
//! trước khi ghép. Nội dung ở đây là RÁC có thể xoá bất kỳ lúc nào, khác `downloads` là kết quả
//! người dùng giữ lại; nên các hàm dưới đây CHỈ được gọi với đường dẫn do
//! [`crate::paths::cache_dir`] sinh ra, không nhận đường dẫn tự do từ UI (không có lệnh nào cho UI
//! chỉ định thư mục để xoá).
//!
//! Ba hàm thuật toán ([`dir_usage`], [`clear_dir`], [`prune_older_than`]) chỉ chạm hệ thống tệp,
//! không cần Tauri/DB nên test được trực tiếp với `tempfile`.

use std::path::Path;
use std::time::{Duration, SystemTime};

use lvc_browser::BrowserDriver;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::{Ctx, IpcError};
use crate::state::AppState;

/// File trong thư mục tạm quá số ngày này thì [`prune_older_than`] xoá — khớp dòng mô tả trên UI
/// ("Tự xoá sau 7 ngày").
pub const CACHE_MAX_AGE_DAYS: u64 = 7;

/// Mốc "đầy" của thanh đo trên UI: 10 GB. Không phải giới hạn cứng (không có hạn mức thật) — chỉ
/// để thanh đo có thang quy chiếu; vượt mốc thì thanh đầy 100%.
pub const CACHE_BAR_FULL_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// Thông tin thư mục tạm cho UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheInfo {
    /// Đường dẫn đầy đủ, để UI hiện và để nút "Mở thư mục" dùng.
    pub path: String,
    /// Tổng dung lượng đang chiếm (byte).
    pub bytes: u64,
    /// Số file (không tính thư mục).
    pub files: u64,
    /// Mốc quy chiếu của thanh đo — [`CACHE_BAR_FULL_BYTES`], gửi kèm để UI không tự đoán.
    pub bar_full_bytes: u64,
    /// Số ngày giữ file trước khi tự xoá — [`CACHE_MAX_AGE_DAYS`].
    pub max_age_days: u64,
}

/// Tổng `(byte, số file)` của một thư mục, đệ quy. Thư mục không tồn tại → `(0, 0)` (chưa chạy lần
/// nào là trạng thái BÌNH THƯỜNG, không phải lỗi).
///
/// File không đọc được metadata (đang bị tiến trình khác giữ) bị bỏ qua lặng lẽ — con số là để
/// người dùng ước lượng, không phải để đối soát.
#[must_use]
pub fn dir_usage(dir: &Path) -> (u64, u64) {
    let mut bytes = 0_u64;
    let mut files = 0_u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let (b, f) = dir_usage(&path);
            bytes += b;
            files += f;
        } else if let Ok(meta) = entry.metadata() {
            bytes += meta.len();
            files += 1;
        }
    }
    (bytes, files)
}

/// Xoá toàn bộ NỘI DUNG của thư mục nhưng GIỮ lại chính thư mục đó (tiến trình đang chạy có thể
/// vẫn đang trỏ vào nó). Trả số byte đã giải phóng.
///
/// Mục nào xoá không được (file đang mở) bị bỏ qua và ghi `warn` — không làm cả lượt dọn thất bại.
pub fn clear_dir(dir: &Path) -> u64 {
    let (before, _) = dir_usage(dir);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let result = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        if let Err(e) = result {
            warn!(error = %e, "thu muc tam: khong xoa duoc mot muc, bo qua");
        }
    }
    let (after, _) = dir_usage(dir);
    before.saturating_sub(after)
}

/// Xoá các file đã được sửa lần cuối cách đây hơn `max_age`. Thư mục con rỗng sau khi xoá cũng
/// được dọn. Trả số file đã xoá.
///
/// `now` truyền vào (không tự gọi `SystemTime::now`) để test ghim được thời điểm.
pub fn prune_older_than(dir: &Path, max_age: Duration, now: SystemTime) -> u64 {
    let mut removed = 0_u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            removed += prune_older_than(&path, max_age, now);
            // Thư mục rỗng sau khi dọn con thì bỏ luôn; còn file thì `remove_dir` tự thất bại.
            let _ = std::fs::remove_dir(&path);
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        // `duration_since` lỗi khi file có mốc thời gian ở TƯƠNG LAI (lệch đồng hồ) — coi như mới,
        // không xoá.
        let Ok(age) = now.duration_since(modified) else {
            continue;
        };
        if age > max_age {
            if let Err(e) = std::fs::remove_file(&path) {
                warn!(error = %e, "thu muc tam: khong xoa duoc file cu, bo qua");
            } else {
                removed += 1;
            }
        }
    }
    removed
}

/// Dọn file quá [`CACHE_MAX_AGE_DAYS`] ngày — gọi MỘT lần lúc khởi động (`bootstrap`).
pub fn prune_cache_at_startup(base: &Path) {
    let dir = crate::paths::cache_dir(base);
    let removed = prune_older_than(
        &dir,
        Duration::from_secs(CACHE_MAX_AGE_DAYS * 24 * 60 * 60),
        SystemTime::now(),
    );
    if removed > 0 {
        info!(removed, "thu muc tam: da don file qua cu luc khoi dong");
    }
}

/// Thân logic `cache_info`.
pub fn cache_info_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>) -> CacheInfo {
    let dir = crate::paths::cache_dir(&ctx.base_dir);
    let (bytes, files) = dir_usage(&dir);
    CacheInfo {
        path: dir.display().to_string(),
        bytes,
        files,
        bar_full_bytes: CACHE_BAR_FULL_BYTES,
        max_age_days: CACHE_MAX_AGE_DAYS,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "chu ky `#[tauri::command]` bat buoc nhan State theo gia tri - bien gioi IPC"
)]
#[tauri::command]
pub fn cache_info(state: tauri::State<'_, AppState>) -> CacheInfo {
    cache_info_impl(&state)
}

/// Thân logic `cache_clear` — trả số byte đã giải phóng.
pub fn cache_clear_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>) -> u64 {
    let dir = crate::paths::cache_dir(&ctx.base_dir);
    let freed = clear_dir(&dir);
    info!(freed, "thu muc tam: da don theo yeu cau nguoi dung");
    freed
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "chu ky `#[tauri::command]` bat buoc nhan State theo gia tri - bien gioi IPC"
)]
#[tauri::command]
pub fn cache_clear(state: tauri::State<'_, AppState>) -> u64 {
    cache_clear_impl(&state)
}

/// Mở MỘT thư mục đã biết bằng trình quản lý tệp của hệ điều hành. `which` chỉ nhận hai giá trị
/// kín (`"output"`/`"cache"`) — KHÔNG nhận đường dẫn tự do từ UI, để một lệnh IPC không trở thành
/// cách mở bất kỳ thứ gì trên máy.
///
/// Thư mục chưa tồn tại thì được tạo trước (lần chạy đầu chưa có `downloads`/`cache`).
///
/// # Errors
/// `IpcError`: `BAD_TARGET` khi `which` lạ; `IO` khi không tạo được thư mục;
/// `SHELL_OPEN_FAILED` khi hệ điều hành từ chối mở.
#[allow(
    deprecated,
    reason = "tauri-plugin-shell::Shell::open van dung tot - cung ly do nhu tasks::task_open_file"
)]
#[tauri::command]
pub async fn open_app_folder(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    which: String,
) -> Result<(), IpcError> {
    use tauri_plugin_shell::ShellExt;

    let dir = match which.as_str() {
        "cache" => crate::paths::cache_dir(&state.base_dir),
        // Dung CHINH ham engine dung de giai `output_dir` -> khong bao gio mo sai thu muc so voi
        // noi thuc su ghi video.
        "output" => lvc_engine::resolve_dir(
            &state.settings.read().await.output_dir,
            &state.base_dir,
            "downloads",
        ),
        other => {
            return Err(IpcError::new(
                "BAD_TARGET",
                format!("thu muc '{other}' khong mo duoc"),
            ));
        }
    };
    std::fs::create_dir_all(&dir).map_err(|e| IpcError::new("IO", e.to_string()))?;
    app.shell()
        .open(dir.display().to_string(), None)
        .map_err(|e| IpcError::new("SHELL_OPEN_FAILED", e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tạo file với nội dung `len` byte.
    fn write(path: &Path, len: usize) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, vec![b'x'; len]).unwrap();
    }

    #[test]
    fn dir_usage_cong_ca_thu_muc_con() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("a.png"), 100);
        write(&dir.path().join("sub/b.mp4"), 250);
        write(&dir.path().join("sub/sau/c.mp4"), 1);
        let (bytes, files) = dir_usage(dir.path());
        assert_eq!(bytes, 351);
        assert_eq!(files, 3, "thu muc KHONG duoc tinh la file");
    }

    #[test]
    fn dir_usage_thu_muc_khong_ton_tai_la_0_khong_phai_loi() {
        let dir = tempfile::tempdir().unwrap();
        let (bytes, files) = dir_usage(&dir.path().join("chua-co"));
        assert_eq!((bytes, files), (0, 0));
    }

    #[test]
    fn clear_dir_xoa_het_noi_dung_nhung_giu_thu_muc() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("a.png"), 100);
        write(&dir.path().join("sub/b.mp4"), 200);

        let freed = clear_dir(dir.path());

        assert_eq!(freed, 300);
        assert!(dir.path().is_dir(), "chinh thu muc tam phai con lai");
        assert_eq!(dir_usage(dir.path()), (0, 0));
    }

    #[test]
    fn prune_chi_xoa_file_qua_cu() {
        let dir = tempfile::tempdir().unwrap();
        let moi = dir.path().join("moi.png");
        let cu = dir.path().join("cu.png");
        write(&moi, 10);
        write(&cu, 10);
        // Đặt mốc sửa của file cũ về 10 ngày trước bằng cách lùi `now` thay vì đổi mtime (đổi
        // mtime cần API riêng theo hệ điều hành) — `now` là tham số nên test ghim được.
        let now = SystemTime::now() + Duration::from_secs(10 * 24 * 60 * 60);

        // Với `now` ở 10 ngày sau, CẢ HAI file đều "quá 7 ngày".
        let removed = prune_older_than(dir.path(), Duration::from_secs(7 * 24 * 60 * 60), now);
        assert_eq!(removed, 2);
        assert!(!moi.exists());
        assert!(!cu.exists());
    }

    #[test]
    fn prune_khong_xoa_file_moi() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("moi.png"), 10);
        let removed = prune_older_than(
            dir.path(),
            Duration::from_secs(7 * 24 * 60 * 60),
            SystemTime::now(),
        );
        assert_eq!(removed, 0, "file vua tao KHONG duoc xoa");
        assert_eq!(dir_usage(dir.path()).1, 1);
    }

    #[test]
    fn prune_bo_thu_muc_con_rong_sau_khi_don() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("sub/cu.png"), 10);
        let now = SystemTime::now() + Duration::from_secs(10 * 24 * 60 * 60);
        prune_older_than(dir.path(), Duration::from_secs(7 * 24 * 60 * 60), now);
        assert!(
            !dir.path().join("sub").exists(),
            "thu muc con rong phai bi don theo"
        );
    }

    #[tokio::test]
    async fn cache_info_tra_duong_dan_duoi_thu_muc_du_lieu() {
        let (dir, ctx, _d) = crate::commands::test_support::ctx_with(vec![], &[]).await;
        let info = cache_info_impl(&ctx);
        assert!(
            info.path.ends_with("cache"),
            "phai la thu muc cache: {}",
            info.path
        );
        assert!(info.path.starts_with(&dir.path().display().to_string()));
        assert_eq!(info.bytes, 0, "chua chay gi thi rong");
        assert_eq!(info.max_age_days, CACHE_MAX_AGE_DAYS);
        assert_eq!(info.bar_full_bytes, CACHE_BAR_FULL_BYTES);
    }

    #[tokio::test]
    async fn cache_clear_don_dung_thu_muc_cache_khong_dung_toi_downloads() {
        let (dir, ctx, _d) = crate::commands::test_support::ctx_with(vec![], &[]).await;
        write(&crate::paths::cache_dir(dir.path()).join("rac.png"), 500);
        let ket_qua = dir.path().join("downloads").join("video.mp4");
        write(&ket_qua, 999);

        let freed = cache_clear_impl(&ctx);

        assert_eq!(freed, 500);
        assert_eq!(cache_info_impl(&ctx).bytes, 0);
        assert!(
            ket_qua.exists(),
            "don thu muc tam KHONG duoc cham vao video nguoi dung giu"
        );
    }
}
