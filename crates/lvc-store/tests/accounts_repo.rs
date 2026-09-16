use lvc_core::Mode;
use lvc_store::{AccountRepo, Db, SettingsRepo};

#[tokio::test]
async fn upsert_hai_lan_khong_nhan_ban() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.upsert("acc2").await.unwrap();
    assert_eq!(repo.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn mac_dinh_bat_lich() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    let rows = repo.list().await.unwrap();
    assert!(rows[0].scheduling);
    assert_eq!(rows[0].proxy, "");
}

#[tokio::test]
async fn tat_lich_va_dat_proxy() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_scheduling("acc2", false).await.unwrap();
    repo.set_proxy("acc2", "http://u:p@1.2.3.4:8080")
        .await
        .unwrap();
    let rows = repo.list().await.unwrap();
    assert!(!rows[0].scheduling);
    assert_eq!(rows[0].proxy, "http://u:p@1.2.3.4:8080");
}

#[tokio::test]
async fn usage_dem_rieng_tung_che_do() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    assert_eq!(
        repo.bump_usage("acc2", "2026-09-16", Mode::Fast)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repo.bump_usage("acc2", "2026-09-16", Mode::Fast)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        repo.bump_usage("acc2", "2026-09-16", Mode::Pro)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repo.used_today("acc2", "2026-09-16", Mode::Fast)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        repo.used_today("acc2", "2026-09-17", Mode::Fast)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn bump_usage_dong_thoi_khong_mat_dem() {
    // File-backed (WAL, 8 connection) — in-memory chi co 1 connection nen khong
    // kiem duoc dong thoi that.
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("dong_thoi.db")).await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();

    let mut handles = Vec::new();
    for _ in 0..32 {
        let r = repo.clone();
        handles.push(tokio::spawn(async move {
            r.bump_usage("acc2", "2026-09-16", Mode::Fast).await
        }));
    }
    for h in handles {
        h.await.unwrap().expect("khong duoc 'database is locked'");
    }
    assert_eq!(
        repo.used_today("acc2", "2026-09-16", Mode::Fast)
            .await
            .unwrap(),
        32
    );
}

#[tokio::test]
async fn settings_get_set() {
    let db = Db::open_memory().await.unwrap();
    let s = SettingsRepo::new(&db);
    assert!(s.get("threads").await.unwrap().is_none());
    s.set("threads", "5").await.unwrap();
    assert_eq!(s.get("threads").await.unwrap().as_deref(), Some("5"));
    s.set("threads", "8").await.unwrap();
    assert_eq!(s.get_or("threads", "3").await.unwrap(), "8");
    assert_eq!(s.get_or("khong_co", "mac_dinh").await.unwrap(), "mac_dinh");
}
