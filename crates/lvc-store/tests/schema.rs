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
    for expected in ["accounts_meta", "proxies", "settings", "tasks", "usage"] {
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
    for expected in ["idx_tasks_status", "idx_tasks_batch", "idx_tasks_retry"] {
        assert!(
            names.contains(&expected),
            "thieu index {expected}; co: {names:?}"
        );
    }
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
