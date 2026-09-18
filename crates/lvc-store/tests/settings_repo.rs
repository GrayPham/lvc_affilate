use std::collections::HashMap;

use lvc_store::{Db, SettingsRepo};

// AppSettings (defaults + parse) o lvc-core (Task 5) — SettingsRepo chi luu
// key/value tho, khong biet gi ve y nghia tung setting.

// Review vong 1 M1: da can nhac them 1 test "set_many rollback khi cap thu hai
// loi" nhung KHONG THEM — ghi lai ly do (M1 cho phep "document why and skip"
// khi khong co that bai tu nhien).
//
// Bang `settings` chi co `key TEXT PRIMARY KEY, value TEXT NOT NULL` — khong
// CHECK, khong UNIQUE nao khac ngoai PK, va PK trung thi INSERT...ON CONFLICT
// DO UPDATE xu ly em, khong loi. Voi chu ky `set_many(&[(&str, &str)])`, MOI
// cap (key, value) hop le deu ghi thanh cong — khong co cach nao tu mot input
// (&str, &str) hop le lam mot cau UPDATE/INSERT giua vong lap that bai o tang
// SQLite. Cach duy nhat tao that bai that (vd giu khoa EXCLUSIVE tren connection
// thu hai de cau thu hai trong vong lap dinh SQLITE_BUSY, hoac mo Db o che do
// read-only) hoac (a) khong tat dinh — dua vao thoi diem SQLite bao ban (dua
// theo `busy_timeout` mac dinh cua sqlx, ~5s, khien test cham/de flaky), hoac
// (b) doi them API san xuat (vd Db::open ho tro read-only/tuy chinh busy_timeout)
// khong ai khac can — vuot pham vi M1. Tinh dung dan rollback (early return qua
// `?` truoc `tx.commit()` lam `tx` bi drop → sqlx tu ROLLBACK) da duoc kiem tra
// qua doc code va la mau hinh sqlx chuan (giong `Transaction::drop` — khong
// commit thi rollback); cac test da co (`set_many_ghi_nhieu_cap_trong_mot_giao_dich`,
// `set_many_ghi_de_cap_da_co`, `set_many_rong_khong_loi`) da phu duong thanh cong.

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
