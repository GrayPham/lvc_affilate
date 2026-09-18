use std::collections::HashMap;

use lvc_store::{Db, SettingsRepo};

// AppSettings (defaults + parse) o lvc-core (Task 5) — SettingsRepo chi luu
// key/value tho, khong biet gi ve y nghia tung setting.

#[tokio::test]
async fn get_all_rong_khi_chua_co_gi() {
    let db = Db::open_memory().await.unwrap();
    let s = SettingsRepo::new(&db);
    assert_eq!(s.get_all().await.unwrap(), HashMap::new());
}

#[tokio::test]
async fn get_all_tra_dung_moi_cap_da_ghi() {
    let db = Db::open_memory().await.unwrap();
    let s = SettingsRepo::new(&db);
    s.set("threads", "5").await.unwrap();
    s.set("theme", "dark").await.unwrap();

    let all = s.get_all().await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all.get("threads").map(String::as_str), Some("5"));
    assert_eq!(all.get("theme").map(String::as_str), Some("dark"));
}

#[tokio::test]
async fn set_many_ghi_nhieu_cap_trong_mot_giao_dich() {
    let db = Db::open_memory().await.unwrap();
    let s = SettingsRepo::new(&db);
    s.set_many(&[("a", "1"), ("b", "2"), ("c", "3")])
        .await
        .unwrap();

    let all = s.get_all().await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all.get("a").map(String::as_str), Some("1"));
    assert_eq!(all.get("b").map(String::as_str), Some("2"));
    assert_eq!(all.get("c").map(String::as_str), Some("3"));
}

#[tokio::test]
async fn set_many_ghi_de_cap_da_co() {
    let db = Db::open_memory().await.unwrap();
    let s = SettingsRepo::new(&db);
    s.set("a", "cu").await.unwrap();
    s.set_many(&[("a", "moi"), ("b", "2")]).await.unwrap();

    assert_eq!(s.get("a").await.unwrap().as_deref(), Some("moi"));
    assert_eq!(s.get("b").await.unwrap().as_deref(), Some("2"));
}

#[tokio::test]
async fn set_many_rong_khong_loi() {
    let db = Db::open_memory().await.unwrap();
    let s = SettingsRepo::new(&db);
    s.set_many(&[]).await.unwrap();
    assert_eq!(s.get_all().await.unwrap(), HashMap::new());
}
