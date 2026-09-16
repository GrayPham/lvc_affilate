use std::path::Path;

use lvc_core::TaskStatus;
use lvc_store::{Db, import_legacy};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, Executor, Row};

/// Dung DB cu toi thieu dung schema Python (chi cot ta doc).
async fn make_legacy(dir: &Path) {
    let tasks_db = dir.join("tasks.db");
    let opts = SqliteConnectOptions::new()
        .filename(&tasks_db)
        .create_if_missing(true);
    let mut conn = opts.connect().await.unwrap();
    conn.execute(
        "CREATE TABLE tasks (
            id TEXT PRIMARY KEY, model TEXT, prompt TEXT, ratio TEXT, duration INTEGER,
            status TEXT, video_url TEXT, error TEXT, created_at REAL, updated_at REAL,
            conversation_id TEXT, failure_code TEXT, reference_images TEXT,
            mode TEXT DEFAULT 'fast', account TEXT, retry_count INTEGER DEFAULT 0,
            retry_at REAL DEFAULT 0, logo TEXT DEFAULT 'off', logo_applied INTEGER DEFAULT 0,
            batch_id TEXT, scene_index INTEGER, account_pref TEXT,
            project_name TEXT, project_dir TEXT, batch_name TEXT, local_path TEXT
        )",
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, model, prompt, ratio, duration, status, created_at, updated_at,
             reference_images, mode, logo)
         VALUES ('video_old1', 'seedance-2.0', 'canh cu', '9:16', 10, 'completed',
             1000.0, 1001.0, '[\"upload:a.jpg\"]', 'pro', 'crop')",
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, model, prompt, ratio, duration, status, created_at, updated_at,
             reference_images, mode, logo)
         VALUES ('video_old2', 'seedance-2.5', 'canh hai', '16:9', 30, 'failed',
             1002.0, 1003.0, '[]', 'fast', 'off')",
    )
    .await
    .unwrap();

    let pool_db = dir.join("pool_usage.db");
    let opts = SqliteConnectOptions::new()
        .filename(&pool_db)
        .create_if_missing(true);
    let mut conn = opts.connect().await.unwrap();
    conn.execute(
        "CREATE TABLE accounts_meta (
            name TEXT PRIMARY KEY, scheduling INTEGER DEFAULT 1, note TEXT DEFAULT '',
            email TEXT DEFAULT '', created_at REAL DEFAULT 0, last_used_at REAL DEFAULT 0,
            login_ok INTEGER DEFAULT 1, proxy TEXT DEFAULT '', browser TEXT DEFAULT ''
        )",
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO accounts_meta (name, email, proxy, browser, scheduling)
         VALUES ('acc2', 'a@b.com', 'http://1.2.3.4:8080', 'opera', 1)",
    )
    .await
    .unwrap();
    conn.execute(
        "CREATE TABLE usage (account TEXT, day TEXT, mode TEXT, used INTEGER,
            PRIMARY KEY(account, day, mode))",
    )
    .await
    .unwrap();
    conn.execute("INSERT INTO usage VALUES ('acc2', '2026-09-15', 'fast', 2)")
        .await
        .unwrap();
    conn.execute("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT)")
        .await
        .unwrap();
    conn.execute("INSERT INTO settings VALUES ('threads', '5')")
        .await
        .unwrap();
}

#[tokio::test]
async fn nhap_du_lieu_cu_day_du() {
    let dir = tempfile::tempdir().unwrap();
    make_legacy(dir.path()).await;

    let db = Db::open_memory().await.unwrap();
    let report = import_legacy(&db, dir.path()).await.unwrap();

    assert_eq!(report.tasks, 2);
    assert_eq!(report.accounts, 1);
    assert_eq!(report.usage, 1);
    assert_eq!(report.settings, 1);

    let repo = lvc_store::TaskRepo::new(&db);
    let t = repo.get("video_old1").await.unwrap().unwrap();
    assert_eq!(t.prompt, "canh cu");
    assert_eq!(t.mode, "pro");
    assert_eq!(t.logo, "crop");
    assert_eq!(t.reference_images, vec!["upload:a.jpg"]);

    let accounts = lvc_store::AccountRepo::new(&db).list().await.unwrap();
    assert_eq!(accounts[0].name, "acc2");
    assert_eq!(accounts[0].proxy, "http://1.2.3.4:8080");
}

#[tokio::test]
async fn chay_hai_lan_khong_nhan_ban() {
    let dir = tempfile::tempdir().unwrap();
    make_legacy(dir.path()).await;

    let db = Db::open_memory().await.unwrap();
    import_legacy(&db, dir.path()).await.unwrap();
    let second = import_legacy(&db, dir.path()).await.unwrap();

    assert_eq!(second.tasks, 0, "lan hai phai bo qua het");
    let rows = lvc_store::TaskRepo::new(&db)
        .list_recent(100)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn thieu_file_cu_thi_bao_skipped_chu_khong_loi() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_memory().await.unwrap();
    let report = import_legacy(&db, dir.path()).await.unwrap();
    assert_eq!(report.tasks, 0);
    assert_eq!(
        report.skipped.len(),
        2,
        "thieu ca tasks.db lan pool_usage.db"
    );
}

#[tokio::test]
async fn schema_cu_thieu_cot_van_nhap_duoc_voi_mac_dinh() {
    // Mo phong may cua nguoi dung chay ban Python RAT cu: tasks.db chi co 4 cot,
    // khong co ratio/duration/model/mode/logo/status/reference_images va khong co
    // bat ky cot nao migrate them sau nay. Khong co pool_usage.db.
    let dir = tempfile::tempdir().unwrap();
    let tasks_db = dir.path().join("tasks.db");
    let opts = SqliteConnectOptions::new()
        .filename(&tasks_db)
        .create_if_missing(true);
    let mut conn = opts.connect().await.unwrap();
    conn.execute(
        "CREATE TABLE tasks (id TEXT PRIMARY KEY, prompt TEXT, created_at REAL, updated_at REAL)",
    )
    .await
    .unwrap();
    conn.execute("INSERT INTO tasks VALUES ('video_ancient', 'prompt co', 1.0, 2.0)")
        .await
        .unwrap();

    let db = Db::open_memory().await.unwrap();
    let report = import_legacy(&db, dir.path()).await.unwrap();
    assert_eq!(report.tasks, 1);
    assert_eq!(report.skipped, vec!["pool_usage.db"]);

    let t = lvc_store::TaskRepo::new(&db)
        .get("video_ancient")
        .await
        .unwrap()
        .expect("task tu schema cu phai nhap duoc");
    assert_eq!(t.prompt, "prompt co");
    assert_eq!(t.ratio, "9:16");
    assert_eq!(t.duration, 10);
    assert_eq!(t.model, "seedance-2.0");
    assert_eq!(t.mode, "fast");
    assert_eq!(t.logo, "off");
    assert_eq!(
        t.status,
        TaskStatus::Failed,
        "task khong co status phai thanh failed de khong bi tu dong chay lai"
    );
    assert_eq!(t.reference_images, Vec::<String>::new());
}

#[tokio::test]
async fn status_cu_la_gia_tri_la_thi_chuan_hoa_thanh_failed() {
    // Ban Python cu co the co status khong nam trong 4 gia tri hop le (du lieu
    // hong, migrate dang do, v.v.). Bo nhap phai chuan hoa ngay khi ghi, khong
    // duoc cay nguyen chuoi la vao cot status.
    let dir = tempfile::tempdir().unwrap();
    let tasks_db = dir.path().join("tasks.db");
    let opts = SqliteConnectOptions::new()
        .filename(&tasks_db)
        .create_if_missing(true);
    let mut conn = opts.connect().await.unwrap();
    conn.execute(
        "CREATE TABLE tasks (id TEXT PRIMARY KEY, prompt TEXT, status TEXT,
             created_at REAL, updated_at REAL)",
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (id, prompt, status, created_at, updated_at)
         VALUES ('video_la', 'prompt la', 'weird', 1.0, 2.0)",
    )
    .await
    .unwrap();

    let db = Db::open_memory().await.unwrap();
    import_legacy(&db, dir.path()).await.unwrap();

    // Doc qua repo: TaskRow.status da duoc chuan hoa boi row_to_task.
    let t = lvc_store::TaskRepo::new(&db)
        .get("video_la")
        .await
        .unwrap()
        .expect("phai nhap duoc");
    assert_eq!(t.status, TaskStatus::Failed);

    // Doc thang cot status trong DB: phai la "failed" that su, khong phai
    // "weird" duoc "chua benh" boi tang doc — neu khong thi bat ky truy van SQL
    // truc tiep nao (dashboard, bao cao) van thay du lieu sai kieu.
    let raw: String = sqlx::query("SELECT status FROM tasks WHERE id = 'video_la'")
        .fetch_one(db.pool())
        .await
        .unwrap()
        .try_get("status")
        .unwrap();
    assert_eq!(
        raw, "failed",
        "cot status phai duoc chuan hoa luc nhap, khong chi luc doc"
    );
}
