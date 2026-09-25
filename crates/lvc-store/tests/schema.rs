use sqlx::Row;
use sqlx::sqlite::SqlitePoolOptions;

use lvc_store::Db;

#[tokio::test]
async fn migration_tao_du_bang() {
    let db = Db::open_memory().await.unwrap();
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    for expected in [
        "accounts_meta",
        "proxies",
        "settings",
        "tasks",
        "usage",
        "workflows",
        "runs",
        "credentials",
    ] {
        assert!(
            names.contains(&expected),
            "thieu bang {expected}; co: {names:?}"
        );
    }
}

#[tokio::test]
async fn co_du_index_bat_buoc() {
    let db = Db::open_memory().await.unwrap();
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    for expected in [
        "idx_tasks_status",
        "idx_tasks_batch",
        "idx_tasks_retry",
        "idx_tasks_run",
        "idx_tasks_status_retry",
    ] {
        assert!(
            names.contains(&expected),
            "thieu index {expected}; co: {names:?}"
        );
    }
}

#[tokio::test]
async fn migration_0002_them_cot_tasks_run_node_stage() {
    // 0002_p3.sql: tasks da co started_at/finished_at/retry_at... tu 0001, chi
    // them 3 cot moi (run_id, node_id, stage) — kiem qua PRAGMA table_info.
    let db = Db::open_memory().await.unwrap();
    let cols: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('tasks')")
        .fetch_all(db.pool())
        .await
        .unwrap();
    let names: Vec<&str> = cols.iter().map(|r| r.0.as_str()).collect();
    for expected in ["run_id", "node_id", "stage"] {
        assert!(
            names.contains(&expected),
            "thieu cot tasks.{expected}; co: {names:?}"
        );
    }
}

#[tokio::test]
async fn migration_0002_workflows_va_runs_dung_cot() {
    let db = Db::open_memory().await.unwrap();

    let wf_cols: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('workflows')")
        .fetch_all(db.pool())
        .await
        .unwrap();
    let wf_names: Vec<&str> = wf_cols.iter().map(|r| r.0.as_str()).collect();
    for expected in ["id", "name", "json", "updated_at"] {
        assert!(wf_names.contains(&expected), "thieu workflows.{expected}");
    }

    let run_cols: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('runs')")
        .fetch_all(db.pool())
        .await
        .unwrap();
    let run_names: Vec<&str> = run_cols.iter().map(|r| r.0.as_str()).collect();
    for expected in [
        "id",
        "workflow_id",
        "status",
        "started_at",
        "finished_at",
        "error",
    ] {
        assert!(run_names.contains(&expected), "thieu runs.{expected}");
    }
}

#[tokio::test]
async fn nang_cap_db_p2_chi_co_0001_len_0002() {
    // Mo phong mot lvc.db that su tu ban P2 (chi chay 0001_init.sql, chua co
    // _sqlx_migrations). Ghi truc tiep bang connection tho — KHONG qua Db::open,
    // vi Db::open se chay het ca migrator (0001+0002) ngay lap tuc.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p2_era.db");
    {
        let opts = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let raw = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        let sql_0001 = include_str!("../migrations/0001_init.sql");
        sqlx::query(sql_0001).execute(&raw).await.unwrap();
        raw.close().await;
    }

    // Bay gio mo bang Db::open that (migrator chua tung chay tren file nay) —
    // phai tu ap 0001 (khong loi vi toan IF NOT EXISTS) roi 0002 (cot/bang moi).
    let db = Db::open(&path).await.unwrap();

    let cols: Vec<(String,)> = sqlx::query_as("SELECT name FROM pragma_table_info('tasks')")
        .fetch_all(db.pool())
        .await
        .unwrap();
    let names: Vec<&str> = cols.iter().map(|r| r.0.as_str()).collect();
    assert!(names.contains(&"run_id"), "phai nang cap len co run_id");
    assert!(names.contains(&"node_id"));
    assert!(names.contains(&"stage"));

    let tables: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(db.pool())
            .await
            .unwrap();
    let table_names: Vec<&str> = tables.iter().map(|r| r.0.as_str()).collect();
    assert!(table_names.contains(&"workflows"));
    assert!(table_names.contains(&"runs"));

    // Du lieu 0001 (vd bang settings) van con nguyen sau nang cap.
    sqlx::query("INSERT INTO settings (key, value) VALUES ('a', 'b')")
        .execute(db.pool())
        .await
        .unwrap();
    let row = sqlx::query("SELECT value FROM settings WHERE key = 'a'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let v: String = row.try_get("value").unwrap();
    assert_eq!(v, "b");
}

#[tokio::test]
async fn mo_lai_file_khong_loi_migration_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lvc.db");
    {
        let db = Db::open(&path).await.unwrap();
        drop(db);
    }
    let db = Db::open(&path).await.unwrap();
    let (mode,): (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal");
}
