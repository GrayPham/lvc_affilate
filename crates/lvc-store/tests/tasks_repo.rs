use lvc_core::{FailureCode, Limits, RawScene, Scene, TaskStatus};
use lvc_store::{BatchRef, Db, TaskRepo};
use sqlx::Row;

fn scene(prompt: &str) -> Scene {
    let raw = RawScene {
        prompt: Some(prompt.into()),
        ..RawScene::default()
    };
    Scene::normalize(&raw, &Limits::default(), None).unwrap()
}

#[tokio::test]
async fn tao_roi_doc_lai_giu_nguyen_truong() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let s = scene("con meo di bo");
    repo.create("video_a1", &s, None).await.unwrap();

    let got = repo.get("video_a1").await.unwrap().expect("phai co");
    assert_eq!(got.prompt, "con meo di bo");
    assert_eq!(got.status, TaskStatus::Queued);
    assert_eq!(got.ratio, "9:16");
    assert_eq!(got.duration, 10);
    assert_eq!(got.reference_images, Vec::<String>::new());
    assert!(got.created_at > 0.0);
}

#[tokio::test]
async fn luu_reference_images_dang_json() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let mut s = scene("x");
    s.reference_images = vec!["upload:a.jpg".into(), "https://x/y.png".into()];
    repo.create("video_b", &s, None).await.unwrap();

    let got = repo.get("video_b").await.unwrap().unwrap();
    assert_eq!(
        got.reference_images,
        vec!["upload:a.jpg", "https://x/y.png"]
    );
}

#[tokio::test]
async fn huy_la_failed_cong_failure_code_cancelled() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_c", &scene("x"), None).await.unwrap();
    let matched = repo
        .set_status(
            "video_c",
            TaskStatus::Failed,
            Some(FailureCode::Cancelled),
            None,
        )
        .await
        .unwrap();
    assert!(matched, "phai khop dung 1 dong khi id ton tai");

    let got = repo.get("video_c").await.unwrap().unwrap();
    assert_eq!(got.status, TaskStatus::Failed);
    assert_eq!(got.failure_code, Some(FailureCode::Cancelled));
}

#[tokio::test]
async fn set_status_tren_id_khong_ton_tai_tra_false() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let matched = repo
        .set_status("khong_ton_tai", TaskStatus::Failed, None, None)
        .await
        .unwrap();
    assert!(!matched, "id khong ton tai thi khong duoc bao thanh cong");
}

#[tokio::test]
async fn batch_luu_index_va_ten() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let b = BatchRef {
        batch_id: "batch_1".into(),
        batch_name: "Du an A".into(),
        scene_index: 2,
    };
    repo.create("video_d", &scene("x"), Some(&b)).await.unwrap();

    let got = repo.get("video_d").await.unwrap().unwrap();
    assert_eq!(got.batch_id.as_deref(), Some("batch_1"));
    assert_eq!(got.scene_index, Some(2));
}

#[tokio::test]
async fn khong_co_thi_tra_none() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    assert!(repo.get("khong_ton_tai").await.unwrap().is_none());
}

#[tokio::test]
async fn list_recent_moi_nhat_truoc_va_kep_limit() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    for i in 0..5 {
        repo.create(&format!("video_{i}"), &scene(&format!("p{i}")), None)
            .await
            .unwrap();
        // created_at = now() co the trung nhau giua hai lan tao lien tiep (do phan
        // giai dong ho khong du min); cho tam nghi de thu tu DESC tat dinh.
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }

    let rows = repo.list_recent(3).await.unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["video_4", "video_3", "video_2"],
        "phai la 3 ban ghi moi nhat, moi nhat truoc"
    );

    assert_eq!(
        repo.list_recent(0).await.unwrap().len(),
        1,
        "limit <= 0 phai kep len 1"
    );
    assert_eq!(
        repo.list_recent(999).await.unwrap().len(),
        5,
        "limit vuot so ban ghi thi tra het, khong loi"
    );
}

// ---- Task 17: cac cot engine ghi trong luc chay (server.py:610-627 on_account/on_conversation_id,
// :684-685 doi mode khi het luot, _finish_task :442-480 video_url/local_path) ----

#[tokio::test]
async fn set_account_va_conversation_ghi_dung_cot() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_e", &scene("x"), None).await.unwrap();

    assert!(repo.set_account("video_e", "acc1").await.unwrap());
    assert!(repo.set_conversation("video_e", "7451234").await.unwrap());

    let got = repo.get("video_e").await.unwrap().unwrap();
    assert_eq!(got.account.as_deref(), Some("acc1"));
    assert_eq!(got.conversation_id.as_deref(), Some("7451234"));
    // cac cot khac khong bi dung
    assert_eq!(got.status, TaskStatus::Queued);
    assert!(got.video_url.is_none());

    // id khong ton tai -> false, khong loi
    assert!(!repo.set_account("ghost", "acc1").await.unwrap());
    assert!(!repo.set_conversation("ghost", "1").await.unwrap());
}

#[tokio::test]
async fn set_result_ghi_video_url_va_local_path() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_f", &scene("x"), None).await.unwrap();

    let local = std::path::Path::new(r"D:\downloads\acc1_20260918_010203.mp4");
    assert!(
        repo.set_result("video_f", "https://v.example/x.mp4", local)
            .await
            .unwrap()
    );
    let got = repo.get("video_f").await.unwrap().unwrap();
    assert_eq!(got.video_url.as_deref(), Some("https://v.example/x.mp4"));
    assert_eq!(got.local_path.as_deref(), Some(local.to_str().unwrap()));
    // set_result KHONG doi status (engine goi set_status(Completed) rieng)
    assert_eq!(got.status, TaskStatus::Queued);
    assert!(!repo.set_result("ghost", "u", local).await.unwrap());
}

#[tokio::test]
async fn set_mode_duration_khi_doi_mode_het_luot() {
    // server.py:684-685 `store.update(task_id, mode=try_mode, duration=try_duration)`
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_g", &scene("x"), None).await.unwrap();
    assert!(
        repo.set_mode_duration("video_g", lvc_core::Mode::Pro, 15)
            .await
            .unwrap()
    );
    let got = repo.get("video_g").await.unwrap().unwrap();
    assert_eq!(got.mode, "pro");
    assert_eq!(got.duration, 15);
    assert!(
        !repo
            .set_mode_duration("ghost", lvc_core::Mode::Fast, 10)
            .await
            .unwrap()
    );
}

// ---- Task 3 (P3): tasks.run_id/node_id/stage + so bo cho DAG executor (Task 7),
// resume (Task 6) va Tauri commands (Task 9). ----

async fn col_f64(db: &Db, id: &str, col: &str) -> f64 {
    let row = sqlx::query(&format!("SELECT {col} FROM tasks WHERE id = ?1"))
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    row.try_get(col).unwrap()
}

#[tokio::test]
async fn set_run_ghi_run_id_va_node_id() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_h", &scene("x"), None).await.unwrap();

    assert!(repo.set_run("video_h", "run1", "node1").await.unwrap());
    let got = repo.get("video_h").await.unwrap().unwrap();
    assert_eq!(got.run_id, "run1");
    assert_eq!(got.node_id, "node1");

    assert!(!repo.set_run("ghost", "run1", "node1").await.unwrap());
}

#[tokio::test]
async fn set_stage_ghi_dung_cot() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_i", &scene("x"), None).await.unwrap();
    assert_eq!(repo.get("video_i").await.unwrap().unwrap().stage, "");

    assert!(repo.set_stage("video_i", "uploading").await.unwrap());
    assert_eq!(
        repo.get("video_i").await.unwrap().unwrap().stage,
        "uploading"
    );
    assert!(!repo.set_stage("ghost", "uploading").await.unwrap());
}

#[tokio::test]
async fn set_started_va_finished_ghi_moc() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("video_j", &scene("x"), None).await.unwrap();
    // Task 6 (P3): TaskRow gio phoi lai 2 cot nay — chua tung `set_started`/`set_finished` -> None
    // (0001_init.sql khong `NOT NULL`, `create` khong ghi cot nay).
    let got = repo.get("video_j").await.unwrap().unwrap();
    assert_eq!(got.started_at, None);
    assert_eq!(got.finished_at, None);

    assert!(repo.set_started("video_j", 111.0).await.unwrap());
    assert!((col_f64(&db, "video_j", "started_at").await - 111.0).abs() < f64::EPSILON);
    assert_eq!(
        repo.get("video_j").await.unwrap().unwrap().started_at,
        Some(111.0)
    );

    assert!(repo.set_finished("video_j", 222.0).await.unwrap());
    assert!((col_f64(&db, "video_j", "finished_at").await - 222.0).abs() < f64::EPSILON);
    assert_eq!(
        repo.get("video_j").await.unwrap().unwrap().finished_at,
        Some(222.0)
    );

    assert!(!repo.set_started("ghost", 1.0).await.unwrap());
    assert!(!repo.set_finished("ghost", 1.0).await.unwrap());
}

#[tokio::test]
async fn list_by_run_sap_theo_scene_index_roi_created_at() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    let b2 = BatchRef {
        batch_id: "b1".into(),
        batch_name: "Du an".into(),
        scene_index: 2,
    };
    let b1 = BatchRef {
        batch_id: "b1".into(),
        batch_name: "Du an".into(),
        scene_index: 1,
    };
    repo.create("video_k2", &scene("canh 2"), Some(&b2))
        .await
        .unwrap();
    repo.create("video_k1", &scene("canh 1"), Some(&b1))
        .await
        .unwrap();
    repo.create("video_khac_run", &scene("khong lien quan"), None)
        .await
        .unwrap();

    repo.set_run("video_k2", "run1", "n2").await.unwrap();
    repo.set_run("video_k1", "run1", "n1").await.unwrap();
    // video_khac_run KHONG duoc gan run_id -> mac dinh '' -> khong xuat hien trong run1

    let rows = repo.list_by_run("run1").await.unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["video_k1", "video_k2"], "scene_index 1 truoc 2");

    assert_eq!(
        repo.list_by_run("run_khong_ton_tai").await.unwrap().len(),
        0
    );
}

#[tokio::test]
async fn list_by_status_chi_loc_dung_trang_thai_va_kep_limit() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    for i in 0..3 {
        repo.create(&format!("video_ls{i}"), &scene("x"), None)
            .await
            .unwrap();
    }
    repo.set_status("video_ls0", TaskStatus::Failed, None, None)
        .await
        .unwrap();

    let queued = repo.list_by_status(TaskStatus::Queued, 10).await.unwrap();
    assert_eq!(queued.len(), 2);
    assert!(queued.iter().all(|r| r.status == TaskStatus::Queued));

    let failed = repo.list_by_status(TaskStatus::Failed, 10).await.unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].id, "video_ls0");

    let limited = repo.list_by_status(TaskStatus::Queued, 1).await.unwrap();
    assert_eq!(limited.len(), 1, "limit phai duoc ap dung");
}

#[tokio::test]
async fn requeue_orphans_chi_dua_processing_khong_conv_ve_queued() {
    // store.py:385-397 requeue_orphans + server.py:903-904 goi khi khoi dong.
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);

    // Case 1: processing, KHONG co conversation_id (con NULL) -> phai duoc requeue.
    // Gan san account + retry_at (nhu the da tung dispatch/bi requeue-thuong truoc do)
    // de kiem ca hai cot nay bi xoa ve NULL/0 dung nhu Python (store.py:394).
    repo.create("orphan_null", &scene("x"), None).await.unwrap();
    repo.set_status("orphan_null", TaskStatus::Processing, None, None)
        .await
        .unwrap();
    repo.set_account("orphan_null", "acc_cu").await.unwrap();
    sqlx::query("UPDATE tasks SET retry_at = 555.0 WHERE id = 'orphan_null'")
        .execute(db.pool())
        .await
        .unwrap();

    // Case 2: processing, CO conversation_id -> KHONG duoc dung vao (account/retry_at
    // cua no cung phai giu nguyen, khong bi requeue_orphans dung toi).
    repo.create("has_conv", &scene("x"), None).await.unwrap();
    repo.set_status("has_conv", TaskStatus::Processing, None, None)
        .await
        .unwrap();
    repo.set_conversation("has_conv", "conv123").await.unwrap();
    repo.set_account("has_conv", "acc_giu_nguyen")
        .await
        .unwrap();

    // Case 3: queued/completed -> khong bi dung toi.
    repo.create("still_queued", &scene("x"), None)
        .await
        .unwrap();
    repo.create("done", &scene("x"), None).await.unwrap();
    repo.set_status("done", TaskStatus::Completed, None, None)
        .await
        .unwrap();

    let ids = repo.requeue_orphans(9000.0).await.unwrap();
    assert_eq!(ids, vec!["orphan_null".to_string()]);

    let got = repo.get("orphan_null").await.unwrap().unwrap();
    assert_eq!(got.status, TaskStatus::Queued);
    assert_eq!(got.failure_code, Some(FailureCode::Restart));
    assert_eq!(got.error.as_deref(), Some("RESTART: app khoi dong lai"));
    assert!((got.updated_at - 9000.0).abs() < f64::EPSILON);
    assert_eq!(got.account, None, "review I1: account phai ve NULL");
    assert!(
        (got.retry_at - 0.0).abs() < f64::EPSILON,
        "review I1: retry_at phai ve 0"
    );

    let untouched = repo.get("has_conv").await.unwrap().unwrap();
    assert_eq!(untouched.status, TaskStatus::Processing);
    assert_eq!(
        untouched.account.as_deref(),
        Some("acc_giu_nguyen"),
        "co conv thi khong bi requeue_orphans dung toi"
    );

    let queued = repo.get("still_queued").await.unwrap().unwrap();
    assert_eq!(queued.status, TaskStatus::Queued);
    assert_eq!(queued.failure_code, None);

    let done = repo.get("done").await.unwrap().unwrap();
    assert_eq!(done.status, TaskStatus::Completed);

    // Goi lai lan 2 -> khong con orphan nao nua.
    assert_eq!(
        repo.requeue_orphans(9001.0).await.unwrap(),
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn requeue_orphans_processing_voi_conversation_rong_cung_duoc_dua_ve() {
    // Cot conversation_id la '' (khong phai NULL) van phai duoc coi la "chua co conv".
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);
    repo.create("orphan_empty", &scene("x"), None)
        .await
        .unwrap();
    repo.set_status("orphan_empty", TaskStatus::Processing, None, None)
        .await
        .unwrap();
    repo.set_conversation("orphan_empty", "").await.unwrap();

    let ids = repo.requeue_orphans(1.0).await.unwrap();
    assert_eq!(ids, vec!["orphan_empty".to_string()]);
}

#[tokio::test]
async fn orphans_with_conversation_chi_lay_processing_co_conv() {
    let db = Db::open_memory().await.unwrap();
    let repo = TaskRepo::new(&db);

    repo.create("proc_conv", &scene("x"), None).await.unwrap();
    repo.set_status("proc_conv", TaskStatus::Processing, None, None)
        .await
        .unwrap();
    repo.set_conversation("proc_conv", "conv1").await.unwrap();

    repo.create("proc_no_conv", &scene("x"), None)
        .await
        .unwrap();
    repo.set_status("proc_no_conv", TaskStatus::Processing, None, None)
        .await
        .unwrap();

    repo.create("queued_conv", &scene("x"), None).await.unwrap();
    repo.set_conversation("queued_conv", "conv2").await.unwrap();

    let rows = repo.orphans_with_conversation().await.unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["proc_conv"]);
}
