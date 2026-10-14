//! Duong dan file/thu muc cua ung dung (P3 Task 8). Cac ham duoi day THUAN (nhan `base` = thu muc
//! du lieu ung dung, khong tu doi CWD/moi truong) nen test duoc khong can runtime Tauri; ham
//! [`app_data_dir`] la lop mong DUY NHAT goi Tauri (`AppHandle::path().app_data_dir()`) — khong
//! test duoc (can `App` that dang chay), moi logic con lai nam o cac ham thuan.

use std::path::{Path, PathBuf};

/// File `SQLite` chinh (`lvc_store::Db::open`).
#[must_use]
pub fn db_path(base: &Path) -> PathBuf {
    base.join("lvc.db")
}

/// Thu muc log, xoay theo ngay (`crate::log::init`).
#[must_use]
pub fn logs_dir(base: &Path) -> PathBuf {
    base.join("logs")
}

/// Thu muc tai video mac dinh khi cai dat `output_dir` de rong
/// (`lvc_engine::EngineConfig::from_settings`).
#[must_use]
pub fn downloads_dir(base: &Path) -> PathBuf {
    base.join("downloads")
}

/// Thu muc TAM: anh/canh le tai ve truoc khi ghep, xem `commands::cache`. Tach khoi
/// `downloads_dir` vi noi dung o day la rac co the xoa bat ky luc nao, con `downloads` la ket qua
/// nguoi dung giu lai.
#[must_use]
pub fn cache_dir(base: &Path) -> PathBuf {
    base.join("cache")
}

/// Thu muc du lieu ung dung theo dinh danh Tauri (`identifier` trong `tauri.conf.json`,
/// `vn.lvcmedia.seedance`) — vd Windows `%APPDATA%/vn.lvcmedia.seedance`.
///
/// # Errors
/// `tauri::Error` khi Tauri khong xac dinh duoc thu muc du lieu he dieu hanh (hiem).
pub fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, tauri::Error> {
    use tauri::Manager;
    app.path().app_data_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_path_noi_ten_file_lvc_db() {
        assert_eq!(
            db_path(Path::new("/app/data")),
            PathBuf::from("/app/data/lvc.db")
        );
    }

    #[test]
    fn logs_dir_noi_thu_muc_logs() {
        assert_eq!(
            logs_dir(Path::new("/app/data")),
            PathBuf::from("/app/data/logs")
        );
    }

    #[test]
    fn cache_dir_noi_thu_muc_cache() {
        assert_eq!(
            cache_dir(Path::new("/app/data")),
            PathBuf::from("/app/data/cache")
        );
    }

    #[test]
    fn downloads_dir_noi_thu_muc_downloads() {
        assert_eq!(
            downloads_dir(Path::new("/app/data")),
            PathBuf::from("/app/data/downloads")
        );
    }
}
