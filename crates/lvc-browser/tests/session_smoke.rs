//! Smoke test cho `lvc_browser::dola::session` tren fixture cuc bo `tests/fixtures/chat.html`
//! (KHONG goi dola.com; cookie `sessionid` gia chi nam trong profile tam va khong bao gio duoc gui
//! di vi trang khong dieu huong toi dola.com). Tat ca `#[ignore]` vi can Chromium:
//! `cargo test -p lvc-browser --test session_smoke -- --ignored`.

mod common;

use std::time::Duration;

use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use common::Fx;
use lvc_browser::dola::DolaError;
use lvc_browser::dola::labels::{DOLA_LANG_JS, MODE_BTN_JS, SESSION_JS};
use lvc_browser::dola::session::{
    self, LangState, SessionInfo, assert_logged_in, assert_logged_in_at, ensure_japanese,
    eval_retry, raise_if_logged_out, recent_conv_ids, wait_settled,
};
use lvc_browser::ops;

/// Dat cookie `sessionid` GIA cho origin dola.com trong profile tam (Network.setCookie voi url
/// tuong minh) — `assert_logged_in` doc lai qua `Network.getCookies(urls=[DOLA_URL])`.
async fn set_fake_dola_sessionid(page: &chromiumoxide::Page) {
    page.set_cookie(
        CookieParam::builder()
            .name("sessionid")
            .value("gia-cho-smoke")
            .url("https://www.dola.com/")
            .build()
            .unwrap(),
    )
    .await
    .unwrap();
}

async fn loads(fx: &Fx) -> i64 {
    ops::eval_json(
        fx.page(),
        "parseInt(document.getElementById('loads').textContent, 10)",
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn labels_js_chay_duoc_tren_trang() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    // MODE_BTN_JS: co nut 高速 -> true
    let has_mode: bool = ops::eval_fn(p, MODE_BTN_JS).await.unwrap();
    assert!(has_mode);
    // DOLA_LANG_JS: khong cookie/ls ngon ngu, co nut 高速
    let st: LangState = ops::eval_fn(p, DOLA_LANG_JS).await.unwrap();
    assert_eq!(
        st,
        LangState {
            ls: String::new(),
            ck: String::new(),
            ul: String::new(),
            jp: true
        }
    );
    // SESSION_JS: co o nhap, nhan la gom "Home"/"Settings"/"高速"/"動画を作成", khong marker
    let info: SessionInfo = ops::eval_fn(p, SESSION_JS).await.unwrap();
    assert!(info.composer);
    for t in ["Home", "Settings", "高速", "動画を作成"] {
        assert!(info.texts.iter().any(|x| x == t), "{:?}", info.texts);
    }
    // finder_* sinh JS hop le va tim dung phan tu
    let id: Option<String> = ops::eval_json(
        p,
        &format!(
            "({})?.id",
            lvc_browser::dola::labels::finder_button_text(lvc_browser::dola::labels::VIDEO_BTN_SRC)
        ),
    )
    .await
    .unwrap();
    assert_eq!(id.as_deref(), Some("create-btn"));
    let id: Option<String> = ops::eval_json(
        p,
        &format!(
            "({})?.id",
            lvc_browser::dola::labels::finder_button_text(&lvc_browser::dola::labels::MODE_BTN_SRC)
        ),
    )
    .await
    .unwrap();
    assert_eq!(id.as_deref(), Some("mode-btn"));
    let id: Option<String> = ops::eval_json(
        p,
        &format!(
            "({})?.id",
            lvc_browser::dola::labels::finder_text_exact("高速")
        ),
    )
    .await
    .unwrap();
    assert_eq!(id.as_deref(), Some("mode-btn"));
    let tag: String = ops::eval_json(
        p,
        &format!("({}).tagName", lvc_browser::dola::labels::finder_composer()),
    )
    .await
    .unwrap();
    assert_eq!(tag, "TEXTAREA");
    let none: Option<String> = ops::eval_json(
        p,
        &format!(
            "({})?.id",
            lvc_browser::dola::labels::finder_menu_item(r"^\s*10s(\b|\s|$)")
        ),
    )
    .await
    .unwrap();
    assert_eq!(none, None);
    let cookie_ok: Option<String> = ops::eval_json(
        p,
        &format!("({})?.id", lvc_browser::dola::labels::COOKIE_OK_FINDER),
    )
    .await
    .unwrap();
    assert_eq!(cookie_ok, None, "khong co nut OK/Đồng ý tren fixture");
    // DURATION_HOOK_JS chay duoc o MAIN world (dat data-lvc-hook)
    let _: serde_json::Value =
        ops::eval_json(p, &lvc_browser::dola::labels::duration_hook_script(30))
            .await
            .unwrap();
    let hook: String = ops::eval_json(p, "document.documentElement.getAttribute('data-lvc-hook')")
        .await
        .unwrap();
    assert_eq!(hook, "installed:30");
    let none: String = ops::eval_fn(p, lvc_browser::dola::labels::CLEAR_ABILITY_TAG_JS)
        .await
        .unwrap();
    assert_eq!(none, "none");
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn eval_retry_chiu_duoc_reload_giua_chung() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    let two: i64 = eval_retry(p, "() => 1 + 1", Duration::from_secs(5), 2)
        .await
        .unwrap();
    assert_eq!(two, 2);
    // Lan 1: trang tu tai lai sau 50 ms va Promise khong bao gio resolve -> context bi huy ->
    // thu lai sau khi tai xong -> lan 2 tra 'sau reload'.
    let js = "() => { if (!sessionStorage.__once) { sessionStorage.__once = '1'; \
              setTimeout(() => location.reload(), 50); return new Promise(() => {}); } \
              return 'sau reload'; }";
    let v: String = eval_retry(p, js, Duration::from_secs(10), 3).await.unwrap();
    assert_eq!(v, "sau reload");
    assert_eq!(loads(&fx).await, 2);
    // Loi JS that -> tra ngay, khong thu lai
    let err = eval_retry::<i64>(p, "() => null.x", Duration::from_secs(5), 3)
        .await
        .unwrap_err();
    assert!(matches!(err, lvc_browser::BrowserError::Js(_)), "{err:?}");
    // Promise treo -> Timeout (khong thu lai)
    let t0 = std::time::Instant::now();
    let err = eval_retry::<i64>(
        p,
        "() => new Promise(() => {})",
        Duration::from_millis(300),
        3,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, lvc_browser::BrowserError::Timeout(_)),
        "{err:?}"
    );
    assert!(t0.elapsed() < Duration::from_secs(2), "{:?}", t0.elapsed());
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn wait_settled_tra_true_khi_co_nut_mode() {
    let fx = Fx::open("chat.html").await;
    let t0 = std::time::Instant::now();
    assert!(
        wait_settled(fx.page(), Duration::from_secs(25))
            .await
            .unwrap()
    );
    // 3 nhip (stable dem tu nhip 2) ~ 2 s, khong doi het 25 s
    let dt = t0.elapsed();
    assert!(
        dt >= Duration::from_secs(2) && dt < Duration::from_secs(8),
        "{dt:?}"
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn wait_settled_tra_false_khi_khong_co_nut_mode() {
    let fx = Fx::open("chat.html?nomode=1").await;
    let t0 = std::time::Instant::now();
    assert!(
        !wait_settled(fx.page(), Duration::from_secs(3))
            .await
            .unwrap()
    );
    assert!(t0.elapsed() >= Duration::from_secs(3), "{:?}", t0.elapsed());
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn assert_logged_in_khong_sessionid_bao_logged_out() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    // Cookie sessionid cho origin 127.0.0.1 KHONG tinh (loc theo dola.com) -> duong "no sessionid".
    p.set_cookie(
        CookieParam::builder()
            .name("sessionid")
            .value("khong-phai-dola")
            .url("http://127.0.0.1/")
            .build()
            .unwrap(),
    )
    .await
    .unwrap();
    let err = assert_logged_in(p, "acc_test").await.unwrap_err();
    match &err {
        DolaError::LoggedOut {
            account,
            why,
            revoked,
        } => {
            assert_eq!(account, "acc_test");
            assert_eq!(why, "no sessionid cookie");
            assert!(!revoked);
        }
        other => panic!("sai bien the: {other:?}"),
    }
    assert_eq!(err.to_string(), "LOGGED_OUT: acc_test: no sessionid cookie");
    assert_eq!(loads(&fx).await, 1, "khong tai lai khi khong co sessionid");
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn assert_logged_in_co_sessionid_va_composer_la_ok() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    assert!(session::has_sessionid(
        &ops::cookies(p, lvc_core::DOLA_URL).await.unwrap()
    ));
    assert_logged_in(p, "acc_test").await.unwrap();
    assert_eq!(loads(&fx).await, 1, "logged_in -> khong tai lai");
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn assert_logged_in_nut_login_du_co_sessionid_tai_lai_1_lan_roi_bao() {
    let fx = Fx::open("chat.html?login=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let t0 = std::time::Instant::now();
    // chat_url = chinh fixture (KHONG dola.com) — van hien ログイン sau khi tai lai
    let err = assert_logged_in_at(p, "acc_test", &fx.url("chat.html?login=1"))
        .await
        .unwrap_err();
    match &err {
        DolaError::LoggedOut { why, revoked, .. } => {
            assert!(
                why.starts_with("login/guest button visible on page after reload: "),
                "{why}"
            );
            assert!(why.contains("ログイン"), "{why}");
            assert!(!revoked);
        }
        other => panic!("sai bien the: {other:?}"),
    }
    assert_eq!(loads(&fx).await, 2, "dung 1 lan tai lai (goto chat_url)");
    // wait_settled ~2 s + goto + 4 s settle
    assert!(t0.elapsed() >= Duration::from_secs(6), "{:?}", t0.elapsed());
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn raise_if_logged_out_theo_url_va_theo_nut_login() {
    // (a) URL chua from_logout=1 -> revoked, khong can SESSION_JS
    let fx = Fx::open("chat.html?from_logout=1").await;
    let err = raise_if_logged_out(fx.page(), "acc_test", "khi chọn tỉ lệ")
        .await
        .unwrap_err();
    match &err {
        DolaError::LoggedOut { why, revoked, .. } => {
            assert!(revoked);
            assert!(
                why.starts_with("Dola tự đăng xuất khi chọn tỉ lệ (from_logout=1). "),
                "{why}"
            );
            assert!(why.ends_with(lvc_core::REVOKED_HINT), "{why}");
        }
        other => panic!("sai bien the: {other:?}"),
    }
    fx.close().await;

    // (b) URL sach nhung trang hien nut ログイン -> revoked
    let fx = Fx::open("chat.html?login=1").await;
    let err = raise_if_logged_out(fx.page(), "acc_test", "khi mở composer Nhanh")
        .await
        .unwrap_err();
    match &err {
        DolaError::LoggedOut { why, revoked, .. } => {
            assert!(revoked);
            assert!(
                why.starts_with("trang hiện nút đăng nhập khi mở composer Nhanh. "),
                "{why}"
            );
        }
        other => panic!("sai bien the: {other:?}"),
    }
    fx.close().await;

    // (c) Trang binh thuong -> Ok
    let fx = Fx::open("chat.html").await;
    raise_if_logged_out(fx.page(), "acc_test", "x")
        .await
        .unwrap();
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn ensure_japanese_xoa_i18nextlng_va_tai_lai() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    // Da tieng Nhat (khong ls/cookie, co nut 高速) -> khong dung gi
    assert!(!ensure_japanese(p).await.unwrap());
    assert_eq!(loads(&fx).await, 1);

    // Gia lap trang dang tieng Anh: localStorage.i18nextLng = 'en'
    let _: serde_json::Value = ops::eval_json(p, "localStorage.setItem('i18nextLng', 'en'); null")
        .await
        .unwrap();
    let t0 = std::time::Instant::now();
    assert!(ensure_japanese(p).await.unwrap(), "phai doi");
    assert!(
        t0.elapsed() >= Duration::from_secs(4),
        "reload + 4 s: {:?}",
        t0.elapsed()
    );
    let ls: Option<String> = ops::eval_json(p, "localStorage.getItem('i18nextLng')")
        .await
        .unwrap();
    assert_eq!(ls, None, "DOLA_SET_JA_JS phai xoa i18nextLng");
    assert_eq!(loads(&fx).await, 2, "dung 1 lan tai lai");
    // Sau khi doi: khong con phai doi nua
    assert!(!ensure_japanese(p).await.unwrap());
    assert_eq!(loads(&fx).await, 2);
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn recent_conv_ids_loc_id_so_va_loi_thi_rong() {
    let fx = Fx::open("chat.html").await;
    let ids = recent_conv_ids(fx.page()).await.unwrap();
    assert_eq!(ids, vec!["3841796000000000689", "12345"]);
    fx.close().await;

    // fetch bi chan -> JS nem -> []
    let fx = Fx::open("chat.html?recent=fail").await;
    let ids = recent_conv_ids(fx.page()).await.unwrap();
    assert!(ids.is_empty(), "{ids:?}");
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test session_smoke -- --ignored"]
async fn ops_goto_cho_domcontentloaded() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    let t0 = std::time::Instant::now();
    ops::goto(p, &fx.url("chat.html?login=1"), Duration::from_millis(300))
        .await
        .unwrap();
    assert!(t0.elapsed() >= Duration::from_millis(300));
    assert!(ops::url(p).await.unwrap().ends_with("chat.html?login=1"));
    assert_eq!(loads(&fx).await, 2);
    // URL khong resolve duoc -> Navigation (errorText), khong treo 60 s
    let t0 = std::time::Instant::now();
    let err = ops::goto(p, "http://khong-ton-tai.invalid/", Duration::ZERO)
        .await
        .unwrap_err();
    assert!(
        matches!(err, lvc_browser::BrowserError::Navigation { .. }),
        "{err:?}"
    );
    assert!(t0.elapsed() < Duration::from_secs(30), "{:?}", t0.elapsed());
    fx.close().await;
}
