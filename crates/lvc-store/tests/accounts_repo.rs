use lvc_core::Mode;
use lvc_store::{AccountRepo, Db, SettingsRepo};

/// So sanh moc thoi gian (cot REAL) — ghi roi doc lai nguyen van nen sai so < EPSILON.
fn assert_f64(got: f64, want: f64) {
    assert!((got - want).abs() < f64::EPSILON, "got {got}, want {want}");
}

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
async fn set_browser_ghi_cot_browser() {
    // Task 17: engine doc `browser` qua BrowserKind::parse; UI (P3) dat gia tri. Mac dinh ''.
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    assert_eq!(repo.get("acc2").await.unwrap().unwrap().browser, "");
    repo.set_browser("acc2", "chrome").await.unwrap();
    assert_eq!(repo.get("acc2").await.unwrap().unwrap().browser, "chrome");
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

// ===== Task 16: cac cot trang thai tai khoan cho AccountPool (browser_pool.py) =====

#[tokio::test]
async fn get_tra_none_khi_thieu_va_du_cot_khi_co() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    assert!(repo.get("khong_co").await.unwrap().is_none());
    repo.upsert("acc2").await.unwrap();
    let row = repo.get("acc2").await.unwrap().expect("phai co");
    assert_eq!(row.name, "acc2");
    assert!(row.login_ok);
    assert!(row.created_at > 0.0);
    assert_f64(row.login_checked_at, 0.0);
    assert_eq!(row.limit_reason, "");
    assert_eq!(row.quota_reason, "");
    assert_eq!(row.credit_balance, None);
    assert_f64(row.credit_checked_at, 0.0);
    assert_f64(row.logged_out_at, 0.0);
    assert_eq!(row.auto_verify_state, "");
    assert_eq!(row.auto_verify_tries, 0);
}

#[tokio::test]
async fn set_login_ok_false_ghi_logged_out_at_true_xoa() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    // browser_pool.py:651-654 mark_logged_out: login_ok=0, login_checked_at=now, logged_out_at=now
    // (KHONG dung den auto_verify_* — :663 ghi 'revoked' rieng sau do)
    repo.set_auto_verify("acc2", "manual", 2).await.unwrap();
    repo.set_login_ok("acc2", false, 1000.0).await.unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert!(!row.login_ok);
    assert_f64(row.login_checked_at, 1000.0);
    assert_f64(row.logged_out_at, 1000.0);
    assert_eq!(row.auto_verify_state, "manual");
    assert_eq!(row.auto_verify_tries, 2);
    // browser_pool.py:864-866 verify ok / :590-593 clear_logged_out: login_ok=1,
    // login_checked_at=now, logged_out_at=0, auto_verify_state='', auto_verify_tries=0
    repo.set_auto_verify("acc2", "revoked", 0).await.unwrap();
    repo.set_login_ok("acc2", true, 2000.0).await.unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert!(row.login_ok);
    assert_f64(row.login_checked_at, 2000.0);
    assert_f64(row.logged_out_at, 0.0);
    assert_eq!(
        row.auto_verify_state, "",
        "review T16 I1: verify ok phai xoa 'revoked'"
    );
    assert_eq!(row.auto_verify_tries, 0);
}

#[tokio::test]
async fn set_auto_verify_ghi_state_va_tries() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_auto_verify("acc2", "revoked", 0).await.unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_eq!(row.auto_verify_state, "revoked");
    assert_eq!(row.auto_verify_tries, 0);
    repo.set_auto_verify("acc2", "checking", 2).await.unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_eq!(row.auto_verify_state, "checking");
    assert_eq!(row.auto_verify_tries, 2);
}

#[tokio::test]
async fn set_cooldown_until_ghi_moc() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_cooldown_until("acc2", 5000.5).await.unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_f64(row.cooldown_until, 5000.5);
}

#[tokio::test]
async fn set_rate_limited_until_ghi_moc_va_ly_do() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_rate_limited_until("acc2", 7000.0, "daily limit")
        .await
        .unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_f64(row.rate_limited_until, 7000.0);
    assert_eq!(row.limit_reason, "daily limit");
}

#[tokio::test]
async fn set_quota_blocked_until_ghi_moc_va_ly_do() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_quota_blocked_until("acc2", 8000.0, "insufficient credits")
        .await
        .unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_f64(row.quota_blocked_until, 8000.0);
    assert_eq!(row.quota_reason, "insufficient credits");
}

#[tokio::test]
async fn touch_last_used_ghi_moc() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.touch_last_used("acc2", 123.25).await.unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_f64(row.last_used_at, 123.25);
}

#[tokio::test]
async fn set_credit_balance_ghi_so_du_va_moc_kiem() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_credit_balance("acc2", Some(1), 900.0)
        .await
        .unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_eq!(row.credit_balance, Some(1));
    assert_f64(row.credit_checked_at, 900.0);
    // browser_pool.py:893-897 _unblock_quota: credit_balance=NULL, credit_checked_at=0
    repo.set_credit_balance("acc2", None, 0.0).await.unwrap();
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_eq!(row.credit_balance, None);
    assert_f64(row.credit_checked_at, 0.0);
}

#[tokio::test]
async fn set_usage_nang_len_khong_ha_xuong() {
    // browser_pool.py:480-483 _mark_daily_limit: ON CONFLICT DO UPDATE SET used=MAX(used, excluded.used)
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    // chua co dong -> chen moi
    repo.set_usage("acc2", "2026-09-16", Mode::Pro, 10)
        .await
        .unwrap();
    assert_eq!(
        repo.used_today("acc2", "2026-09-16", Mode::Pro)
            .await
            .unwrap(),
        10
    );
    // da dung 12 (vuot han muc vi han muc bi ha sau) -> giu 12, khong ha ve 10
    repo.set_usage("acc2", "2026-09-16", Mode::Pro, 12)
        .await
        .unwrap();
    repo.set_usage("acc2", "2026-09-16", Mode::Pro, 10)
        .await
        .unwrap();
    assert_eq!(
        repo.used_today("acc2", "2026-09-16", Mode::Pro)
            .await
            .unwrap(),
        12
    );
    // mode khac khong bi anh huong
    assert_eq!(
        repo.used_today("acc2", "2026-09-16", Mode::Fast)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn clear_expired_limits_chi_xoa_moc_da_qua() {
    // browser_pool.py:459-467 _clear_expired_rate_limits (+ cooldown_until theo ruling Task 16)
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    for n in ["het_han", "con_han", "khong_dat"] {
        repo.upsert(n).await.unwrap();
    }
    repo.set_rate_limited_until("het_han", 100.0, "r1")
        .await
        .unwrap();
    repo.set_quota_blocked_until("het_han", 100.0, "q1")
        .await
        .unwrap();
    repo.set_cooldown_until("het_han", 100.0).await.unwrap();
    repo.set_rate_limited_until("con_han", 300.0, "r2")
        .await
        .unwrap();
    repo.set_quota_blocked_until("con_han", 300.0, "q2")
        .await
        .unwrap();
    repo.set_cooldown_until("con_han", 300.0).await.unwrap();

    // moc dung bang now cung coi la het han (<= now)
    let n = repo.clear_expired_limits(100.0).await.unwrap();
    assert_eq!(n, 1, "chi 1 dong bi cham");

    let row = repo.get("het_han").await.unwrap().unwrap();
    assert_f64(row.rate_limited_until, 0.0);
    assert_eq!(row.limit_reason, "");
    assert_f64(row.quota_blocked_until, 0.0);
    assert_eq!(row.quota_reason, "");
    assert_f64(row.cooldown_until, 0.0);

    let row = repo.get("con_han").await.unwrap().unwrap();
    assert_f64(row.rate_limited_until, 300.0);
    assert_eq!(row.limit_reason, "r2");
    assert_f64(row.quota_blocked_until, 300.0);
    assert_eq!(row.quota_reason, "q2");
    assert_f64(row.cooldown_until, 300.0);

    // goi lai khi khong con gi het han -> 0 dong
    assert_eq!(repo.clear_expired_limits(100.0).await.unwrap(), 0);
}

#[tokio::test]
async fn clear_expired_limits_xoa_tung_cot_doc_lap() {
    // rate_limited het han nhung quota con han -> chi xoa rate_limited
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.set_rate_limited_until("acc2", 100.0, "r")
        .await
        .unwrap();
    repo.set_quota_blocked_until("acc2", 300.0, "q")
        .await
        .unwrap();
    assert_eq!(repo.clear_expired_limits(200.0).await.unwrap(), 1);
    let row = repo.get("acc2").await.unwrap().unwrap();
    assert_f64(row.rate_limited_until, 0.0);
    assert_eq!(row.limit_reason, "");
    assert_f64(row.quota_blocked_until, 300.0);
    assert_eq!(row.quota_reason, "q");
}

// ===== Task 10: xoa tai khoan =====

#[tokio::test]
async fn delete_xoa_dong_tra_true_goi_lai_tra_false() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    assert!(repo.delete("acc2").await.unwrap());
    assert!(repo.get("acc2").await.unwrap().is_none());
    assert!(
        !repo.delete("acc2").await.unwrap(),
        "da xoa -> lan hai tra false, khong loi"
    );
}

#[tokio::test]
async fn delete_khong_dung_toi_bang_usage() {
    let db = Db::open_memory().await.unwrap();
    let repo = AccountRepo::new(&db);
    repo.upsert("acc2").await.unwrap();
    repo.bump_usage("acc2", "2026-09-16", Mode::Fast)
        .await
        .unwrap();
    repo.delete("acc2").await.unwrap();
    // usage khong bi dong theo (lich su dung con nguyen, chi accounts_meta mat dong)
    assert_eq!(
        repo.used_today("acc2", "2026-09-16", Mode::Fast)
            .await
            .unwrap(),
        1
    );
}
