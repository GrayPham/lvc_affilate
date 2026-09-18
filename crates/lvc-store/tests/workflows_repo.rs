use lvc_store::{Db, RunRepo, WorkflowRepo};

// workflows/runs la khai niem moi cua P3 (do thi React Flow + DAG executor Task 7) —
// khong co tuong duong trong ban Python (chi co task don, khong co workflow).

#[tokio::test]
async fn upsert_roi_get_giu_nguyen_json() {
    let db = Db::open_memory().await.unwrap();
    let repo = WorkflowRepo::new(&db);
    repo.upsert("wf1", "Kich ban A", r#"{"nodes":[]}"#, 100.0)
        .await
        .unwrap();

    let got = repo.get("wf1").await.unwrap().expect("phai co");
    assert_eq!(got.id, "wf1");
    assert_eq!(got.name, "Kich ban A");
    assert_eq!(got.json, r#"{"nodes":[]}"#);
    assert!((got.updated_at - 100.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn upsert_hai_lan_ghi_de_khong_nhan_ban() {
    let db = Db::open_memory().await.unwrap();
    let repo = WorkflowRepo::new(&db);
    repo.upsert("wf1", "Ten cu", "{}", 1.0).await.unwrap();
    repo.upsert("wf1", "Ten moi", r#"{"a":1}"#, 2.0)
        .await
        .unwrap();

    let got = repo.get("wf1").await.unwrap().unwrap();
    assert_eq!(got.name, "Ten moi");
    assert_eq!(got.json, r#"{"a":1}"#);
    assert!((got.updated_at - 2.0).abs() < f64::EPSILON);

    let metas = repo.list_meta().await.unwrap();
    assert_eq!(metas.len(), 1, "khong duoc nhan ban khi upsert cung id");
}

#[tokio::test]
async fn get_tra_none_khi_khong_co() {
    let db = Db::open_memory().await.unwrap();
    let repo = WorkflowRepo::new(&db);
    assert!(repo.get("khong_ton_tai").await.unwrap().is_none());
}

#[tokio::test]
async fn list_meta_khong_co_json_va_sap_theo_moi_nhat_truoc() {
    let db = Db::open_memory().await.unwrap();
    let repo = WorkflowRepo::new(&db);
    repo.upsert("wf1", "A", "{}", 1.0).await.unwrap();
    repo.upsert("wf2", "B", "{}", 2.0).await.unwrap();

    let metas = repo.list_meta().await.unwrap();
    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0].id, "wf2", "moi nhat (updated_at lon nhat) truoc");
    assert_eq!(metas[1].id, "wf1");
}

#[tokio::test]
async fn delete_xoa_dung_dong_tra_bool() {
    let db = Db::open_memory().await.unwrap();
    let repo = WorkflowRepo::new(&db);
    repo.upsert("wf1", "A", "{}", 1.0).await.unwrap();

    assert!(repo.delete("wf1").await.unwrap());
    assert!(repo.get("wf1").await.unwrap().is_none());
    assert!(
        !repo.delete("wf1").await.unwrap(),
        "xoa lai id da mat phai tra false"
    );
}

#[tokio::test]
async fn run_create_roi_get_mac_dinh_running() {
    let db = Db::open_memory().await.unwrap();
    let repo = RunRepo::new(&db);
    repo.create("run1", "wf1", 500.0).await.unwrap();

    let got = repo.get("run1").await.unwrap().expect("phai co");
    assert_eq!(got.id, "run1");
    assert_eq!(got.workflow_id, "wf1");
    assert_eq!(got.status, "running");
    assert!((got.started_at - 500.0).abs() < f64::EPSILON);
    assert!((got.finished_at - 0.0).abs() < f64::EPSILON);
    assert_eq!(got.error, "");
}

#[tokio::test]
async fn run_set_status_ghi_loi_va_moc_ket_thuc() {
    let db = Db::open_memory().await.unwrap();
    let repo = RunRepo::new(&db);
    repo.create("run1", "wf1", 500.0).await.unwrap();

    let matched = repo
        .set_status("run1", "failed", Some("mat mang"), 600.0)
        .await
        .unwrap();
    assert!(matched);

    let got = repo.get("run1").await.unwrap().unwrap();
    assert_eq!(got.status, "failed");
    assert_eq!(got.error, "mat mang");
    assert!((got.finished_at - 600.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn run_set_status_khong_loi_thi_error_rong() {
    let db = Db::open_memory().await.unwrap();
    let repo = RunRepo::new(&db);
    repo.create("run1", "wf1", 500.0).await.unwrap();

    repo.set_status("run1", "completed", None, 600.0)
        .await
        .unwrap();
    let got = repo.get("run1").await.unwrap().unwrap();
    assert_eq!(got.status, "completed");
    assert_eq!(got.error, "");
}

#[tokio::test]
async fn run_set_status_id_khong_ton_tai_tra_false() {
    let db = Db::open_memory().await.unwrap();
    let repo = RunRepo::new(&db);
    assert!(!repo.set_status("ma", "failed", None, 1.0).await.unwrap());
}

#[tokio::test]
async fn run_get_tra_none_khi_khong_co() {
    let db = Db::open_memory().await.unwrap();
    let repo = RunRepo::new(&db);
    assert!(repo.get("khong_co").await.unwrap().is_none());
}

#[tokio::test]
async fn run_list_recent_moi_nhat_truoc_va_kep_limit() {
    let db = Db::open_memory().await.unwrap();
    let repo = RunRepo::new(&db);
    repo.create("run1", "wf1", 1.0).await.unwrap();
    repo.create("run2", "wf1", 2.0).await.unwrap();
    repo.create("run3", "wf1", 3.0).await.unwrap();

    let rows = repo.list_recent(2).await.unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["run3", "run2"]);
}
