use lvc_core::{FailureCode, Limits, RawScene, Scene, TaskStatus};
use lvc_store::{BatchRef, Db, TaskRepo};

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
