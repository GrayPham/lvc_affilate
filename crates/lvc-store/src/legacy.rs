use std::path::Path;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};

use lvc_core::TaskStatus;

use crate::{Db, StoreError};

/// Kết quả nhập. `skipped` liệt kê file cũ không tìm thấy — thiếu file không
/// phải lỗi, vì người dùng có thể cài mới hoàn toàn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub tasks: usize,
    pub accounts: usize,
    pub usage: usize,
    pub settings: usize,
    pub skipped: Vec<String>,
    /// P3c Task 1 (R5): so dong `scripts` da nhap tu `tasks.db` cua ban Python cu — CHI khi
    /// bang do ton tai (DB cu chua chay `scripts_api`/`ScriptStore` thi khong co bang nay,
    /// khong phai loi). `''` la trong `skipped` VAN dung cho `tasks.db`/`pool_usage.db` thieu file.
    pub scripts: usize,
    /// P3c Task 1 (R5): so dong `scenes` da nhap — xem [`ImportReport::scripts`].
    pub scenes: usize,
}

async fn open_readonly(path: &Path) -> Result<SqlitePool, StoreError> {
    // Minor 9 (review T10 fix-round 1): `?mode=ro&immutable=1` (context.md) — `read_only(true)`
    // khong du: `immutable(true)` cho phep mo file NGAY CA KHI tien trinh Python cu con giu no mo
    // (khong co -wal/-shm vi DB Python dung journal mode DELETE mac dinh), va tat SQLite kiem tra
    // thay doi file (mtime) moi lan truy van — an toan vi ta chi doc MOT LAN luc import.
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .immutable(true);
    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?)
}

/// Đọc cột có thể không tồn tại ở DB cũ (bản Python migrate bằng
/// `ALTER TABLE ... ADD COLUMN` nuốt lỗi, nên schema thực tế khác nhau tuỳ máy).
fn opt_str(row: &sqlx::sqlite::SqliteRow, col: &str) -> Option<String> {
    row.try_get::<Option<String>, _>(col).ok().flatten()
}

fn opt_i64(row: &sqlx::sqlite::SqliteRow, col: &str) -> Option<i64> {
    row.try_get::<Option<i64>, _>(col).ok().flatten()
}

fn opt_f64(row: &sqlx::sqlite::SqliteRow, col: &str) -> Option<f64> {
    row.try_get::<Option<f64>, _>(col).ok().flatten()
}

/// P3c Task 1 (R5): DB Python cu co the chua tung mo tab Kich ban -> bang `scripts`/`scenes`
/// khong ton tai. Tra `false` (khong loi) khi thieu, giong `opt_*` phong thu schema cu khac nhau.
async fn table_exists(pool: &SqlitePool, name: &str) -> Result<bool, StoreError> {
    let row = sqlx::query("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// Nhập `tasks.db` + `pool_usage.db` của bản Python vào DB mới.
///
/// Idempotent: chạy lại chỉ bỏ qua bản ghi đã có (`INSERT OR IGNORE`).
/// Chỉ ĐỌC DB cũ — không bao giờ sửa dữ liệu gốc.
///
/// # Errors
/// `StoreError::Sqlx` khi đọc/ghi thất bại trên file thực sự tồn tại.
#[allow(clippy::too_many_lines)]
// Hàm cần xử lý hai file DB với nhiều bảng và cột tùy chọn. Schema Python cũ
// thay đổi qua lịch sử nên phải đọc từng cột qua opt_* để chứa sơ các tính năng khác nhau.
pub async fn import_legacy(db: &Db, legacy_dir: &Path) -> Result<ImportReport, StoreError> {
    let mut report = ImportReport::default();

    let tasks_db = legacy_dir.join("tasks.db");
    if tasks_db.is_file() {
        let old = open_readonly(&tasks_db).await?;
        let rows = sqlx::query("SELECT * FROM tasks").fetch_all(&old).await?;
        for r in &rows {
            let Some(id) = opt_str(r, "id") else { continue };
            let res = sqlx::query(
                "INSERT OR IGNORE INTO tasks
                   (id, prompt, ratio, duration, model, mode, logo, logo_applied, status,
                    failure_code, error, account, account_pref, conversation_id, video_url,
                    local_path, reference_images, batch_id, batch_name, scene_index,
                    project_name, project_dir, retry_count, retry_at, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
                         ?20,?21,?22,?23,?24,?25,?26)",
            )
            .bind(&id)
            .bind(opt_str(r, "prompt").unwrap_or_default())
            .bind(opt_str(r, "ratio").unwrap_or_else(|| "9:16".into()))
            .bind(opt_i64(r, "duration").unwrap_or(10))
            .bind(opt_str(r, "model").unwrap_or_else(|| "seedance-2.0".into()))
            .bind(opt_str(r, "mode").unwrap_or_else(|| "fast".into()))
            .bind(opt_str(r, "logo").unwrap_or_else(|| "off".into()))
            .bind(opt_i64(r, "logo_applied").unwrap_or(0))
            .bind(
                // Chuan hoa ngay luc nhap: gia tri la (hoac thieu) tro thanh
                // "failed" that su trong cot, khong chi khi doc qua TaskRepo.
                opt_str(r, "status")
                    .and_then(|s| TaskStatus::parse(&s))
                    .unwrap_or(TaskStatus::Failed)
                    .as_str(),
            )
            .bind(opt_str(r, "failure_code"))
            .bind(opt_str(r, "error"))
            .bind(opt_str(r, "account"))
            .bind(opt_str(r, "account_pref"))
            .bind(opt_str(r, "conversation_id"))
            .bind(opt_str(r, "video_url"))
            .bind(opt_str(r, "local_path"))
            .bind(opt_str(r, "reference_images").unwrap_or_else(|| "[]".into()))
            .bind(opt_str(r, "batch_id"))
            .bind(opt_str(r, "batch_name"))
            .bind(opt_i64(r, "scene_index"))
            .bind(opt_str(r, "project_name"))
            .bind(opt_str(r, "project_dir"))
            .bind(opt_i64(r, "retry_count").unwrap_or(0))
            .bind(opt_f64(r, "retry_at").unwrap_or(0.0))
            .bind(opt_f64(r, "created_at").unwrap_or(0.0))
            .bind(opt_f64(r, "updated_at").unwrap_or(0.0))
            .execute(db.pool())
            .await?;
            report.tasks += usize::try_from(res.rows_affected()).unwrap_or(0);
        }

        // R5 (P3c Task 1): scripts/scenes nam CUNG file tasks.db (ScriptStore Python dung chung
        // connection voi TaskStore, scripts_store.py:21-25) — chi co khi ban Python da tung mo
        // tab Kich ban (ScriptStore._init tao bang). Ban khong co bang nay thi bo qua LANG LE
        // (khong phai loi, khong them vao `skipped` — do khac tasks.db/pool_usage.db la CA FILE
        // co the thieu, con day chi la MOT bang tuy chon trong file da co).
        let has_scripts = table_exists(&old, "scripts").await?;
        if has_scripts {
            let rows = sqlx::query("SELECT * FROM scripts").fetch_all(&old).await?;
            for r in &rows {
                let Some(id) = opt_str(r, "id") else { continue };
                let res = sqlx::query(
                    "INSERT OR IGNORE INTO scripts (id,name,dir,created_at,updated_at)
                     VALUES (?1,?2,?3,?4,?5)",
                )
                .bind(&id)
                .bind(opt_str(r, "name").unwrap_or_default())
                .bind(opt_str(r, "dir").unwrap_or_default())
                .bind(opt_f64(r, "created_at"))
                .bind(opt_f64(r, "updated_at"))
                .execute(db.pool())
                .await?;
                report.scripts += usize::try_from(res.rows_affected()).unwrap_or(0);
            }
        }
        let has_scenes = table_exists(&old, "scenes").await?;
        if has_scenes {
            let rows = sqlx::query("SELECT * FROM scenes").fetch_all(&old).await?;
            for r in &rows {
                let Some(id) = opt_str(r, "id") else { continue };
                let Some(script_id) = opt_str(r, "script_id") else {
                    continue;
                };
                // task_id GIU NGUYEN (R5) — task da nhap o P0 dung CHINH id nay.
                let res = sqlx::query(
                    "INSERT OR IGNORE INTO scenes
                       (id,script_id,idx,prompt,ratio,duration,model,mode,logo,account,refs,
                        task_id,updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                )
                .bind(&id)
                .bind(&script_id)
                .bind(opt_i64(r, "idx").unwrap_or(0))
                .bind(opt_str(r, "prompt").unwrap_or_default())
                .bind(opt_str(r, "ratio"))
                .bind(opt_i64(r, "duration"))
                .bind(opt_str(r, "model"))
                .bind(opt_str(r, "mode"))
                .bind(opt_str(r, "logo"))
                .bind(opt_str(r, "account").unwrap_or_default())
                .bind(opt_str(r, "refs").unwrap_or_else(|| "[]".into()))
                .bind(opt_str(r, "task_id"))
                .bind(opt_f64(r, "updated_at"))
                .execute(db.pool())
                .await?;
                report.scenes += usize::try_from(res.rows_affected()).unwrap_or(0);
            }
        }

        old.close().await;
    } else {
        report.skipped.push("tasks.db".to_string());
    }

    let pool_db = legacy_dir.join("pool_usage.db");
    if pool_db.is_file() {
        let old = open_readonly(&pool_db).await?;

        let rows = sqlx::query("SELECT * FROM accounts_meta")
            .fetch_all(&old)
            .await?;
        for r in &rows {
            let Some(name) = opt_str(r, "name") else {
                continue;
            };
            let res = sqlx::query(
                "INSERT OR IGNORE INTO accounts_meta
                   (name, scheduling, note, email, created_at, last_used_at, login_ok,
                    proxy, browser)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            )
            .bind(&name)
            .bind(opt_i64(r, "scheduling").unwrap_or(1))
            .bind(opt_str(r, "note").unwrap_or_default())
            .bind(opt_str(r, "email").unwrap_or_default())
            .bind(opt_f64(r, "created_at").unwrap_or(0.0))
            .bind(opt_f64(r, "last_used_at").unwrap_or(0.0))
            .bind(opt_i64(r, "login_ok").unwrap_or(1))
            .bind(opt_str(r, "proxy").unwrap_or_default())
            .bind(opt_str(r, "browser").unwrap_or_default())
            .execute(db.pool())
            .await?;
            report.accounts += usize::try_from(res.rows_affected()).unwrap_or(0);
        }

        let rows = sqlx::query("SELECT * FROM usage").fetch_all(&old).await?;
        for r in &rows {
            let res = sqlx::query(
                "INSERT OR IGNORE INTO usage (account, day, mode, used) VALUES (?1,?2,?3,?4)",
            )
            .bind(opt_str(r, "account").unwrap_or_default())
            .bind(opt_str(r, "day").unwrap_or_default())
            .bind(opt_str(r, "mode").unwrap_or_else(|| "fast".into()))
            .bind(opt_i64(r, "used").unwrap_or(0))
            .execute(db.pool())
            .await?;
            report.usage += usize::try_from(res.rows_affected()).unwrap_or(0);
        }

        if let Ok(rows) = sqlx::query("SELECT * FROM settings").fetch_all(&old).await {
            for r in &rows {
                let res = sqlx::query("INSERT OR IGNORE INTO settings (key, value) VALUES (?1,?2)")
                    .bind(opt_str(r, "key").unwrap_or_default())
                    .bind(opt_str(r, "value").unwrap_or_default())
                    .execute(db.pool())
                    .await?;
                report.settings += usize::try_from(res.rows_affected()).unwrap_or(0);
            }
        }

        old.close().await;
    } else {
        report.skipped.push("pool_usage.db".to_string());
    }

    Ok(report)
}
