//! Smoke test cho `lvc_browser::dola::compose` tren fixture cuc bo `tests/fixtures/compose.html`
//! (KHONG goi dola.com; profile tam). Tat ca `#[ignore]` vi can Chromium:
//! `cargo test -p lvc-browser --test compose_smoke -- --ignored`.
//!
//! Fixture gia lap composer Dola: nut che do 高速/プロ mo `[role=menu]` (tao/xoa dong, dong bang
//! Escape), 動画を作成 hien モデル/10s/比率, menu model (Dreamina Seedance 2.5), menu ti le
//! (`[role=menuitem]` 16:9), menu thoi luong (15s), the dinh kem `[class*="image-container-"]
//! [class*="container-"]` co ✕ hien sau hover, the ky nang SVG, stub `fetch` ghi body roi goi
//! fetch that (hook ADR-044 doi `duration` -> `data-lvc-hook=patched:N`), Enter trong o soan
//! gui `/chat/completion`. Query string: xem dau file fixture.

mod common;

use std::time::Duration;

use common::Fx;
use lvc_browser::dola::DolaError;
use lvc_browser::dola::compose::{
    clear_ability_tag, clear_composer_attachments, close_open_menus, dismiss_cookie_banner,
    hook_state, install_duration_hook, open_video_composer_fast, select_fast_duration, select_mode,
    select_model, select_ratio, type_and_send, verify_hook_patched,
};
use lvc_browser::ops;
use lvc_core::{Mode, Model, Ratio};

async fn js<T: serde::de::DeserializeOwned>(fx: &Fx, expr: &str) -> T {
    ops::eval_json(fx.page(), expr).await.expect("eval")
}

/// `textContent` cua `#id` (None khi khong co).
async fn text_of(fx: &Fx, id: &str) -> Option<String> {
    js(
        fx,
        &format!("document.getElementById('{id}')?.textContent ?? null"),
    )
    .await
}

async fn has_menu(fx: &Fx) -> bool {
    js(fx, "!!document.querySelector('[role=\"menu\"]')").await
}

async fn loads(fx: &Fx) -> i64 {
    js(
        fx,
        "parseInt(document.getElementById('loads').textContent, 10)",
    )
    .await
}

fn count(log: &str, needle: &str) -> usize {
    log.lines().filter(|l| l.contains(needle)).count()
}

fn pos(log: &str, needle: &str) -> usize {
    log.find(needle)
        .unwrap_or_else(|| panic!("khong thay {needle:?} trong log:\n{log}"))
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn close_open_menus_escape_dong_menu() {
    let fx = Fx::open("compose.html").await;
    let p = fx.page();
    // Chua co menu -> khong bam Escape
    close_open_menus(p).await.unwrap();
    assert_eq!(count(&fx.log().await, "keydown Escape"), 0);
    // Mo menu che do -> [role=menu] ton tai -> Escape 1 lan la dong
    assert!(
        ops::click_js(p, "document.getElementById('mode-btn')")
            .await
            .unwrap()
    );
    assert!(has_menu(&fx).await);
    close_open_menus(p).await.unwrap();
    assert!(!has_menu(&fx).await);
    let log = fx.log().await;
    assert_eq!(count(&log, "keydown Escape"), 1, "{log}");
    assert!(log.contains("escape"), "{log}");
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn dismiss_cookie_banner_bam_ok() {
    let fx = Fx::open("compose.html?cookie=1").await;
    let p = fx.page();
    let t0 = std::time::Instant::now();
    dismiss_cookie_banner(p).await.unwrap();
    assert!(
        t0.elapsed() >= Duration::from_millis(500),
        "ngu 500 ms sau khi bam"
    );
    let log = fx.log().await;
    assert_eq!(count(&log, "cookie ok"), 1, "{log}");
    // Banner da mat -> lan 2 khong bam gi
    dismiss_cookie_banner(p).await.unwrap();
    assert_eq!(count(&fx.log().await, "cookie ok"), 1);
    fx.close().await;

    // Khong co banner -> khong lam gi, khong loi
    let fx = Fx::open("compose.html").await;
    dismiss_cookie_banner(fx.page()).await.unwrap();
    assert!(!fx.log().await.contains("cookie ok"));
    fx.close().await;
}

/// :2030-2045 — dang プロ, muon Nhanh: doc nhan -> bam nut -> 700 ms -> bam muc 高速 -> 1500 ms ->
/// dong menu -> doc lai "高速".
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_mode_fast_khi_dang_pro() {
    let fx = Fx::open("compose.html?mode=pro&cookie=1").await;
    let p = fx.page();
    assert_eq!(text_of(&fx, "mode-btn").await.as_deref(), Some("プロ"));
    let t0 = std::time::Instant::now();
    let label = select_mode(p, Mode::Fast).await.unwrap();
    assert_eq!(label, "高速");
    let dt = t0.elapsed();
    assert!(
        dt >= Duration::from_millis(500 + 700 + 1500) && dt < Duration::from_secs(8),
        "{dt:?}"
    );
    assert_eq!(text_of(&fx, "mode-btn").await.as_deref(), Some("高速"));
    assert!(!has_menu(&fx).await, "menu phai dong");
    let log = fx.log().await;
    // cookie banner truoc, roi bam nut che do, roi muc menu
    assert!(
        pos(&log, "cookie ok") < pos(&log, "click mode-btn"),
        "{log}"
    );
    assert!(
        pos(&log, "click mode-btn") < pos(&log, "mode 高速"),
        "{log}"
    );
    assert_eq!(count(&log, "click mode-btn"), 1, "{log}");

    // Da dung che do -> khong bam nua (doc nhan thoi)
    assert_eq!(select_mode(p, Mode::Fast).await.unwrap(), "高速");
    assert_eq!(count(&fx.log().await, "click mode-btn"), 1);

    // Agent/ProMax -> nut プロ (:2475)
    assert_eq!(select_mode(p, Mode::Agent).await.unwrap(), "プロ");
    assert_eq!(text_of(&fx, "mode-btn").await.as_deref(), Some("プロ"));
    assert_eq!(select_mode(p, Mode::ProMax).await.unwrap(), "プロ");
    let log = fx.log().await;
    assert_eq!(count(&log, "click mode-btn"), 2, "{log}");
    assert_eq!(count(&log, "mode プロ"), 1, "{log}");
    fx.close().await;
}

/// :2047-2049 — bam 2 vong ma nhan khong doi -> `ModeSwitch { want, shows }` (khong ma).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_mode_bi_ket_bao_mode_switch() {
    let fx = Fx::open("compose.html?mode=pro&stuck=1").await;
    let t0 = std::time::Instant::now();
    let err = select_mode(fx.page(), Mode::Fast).await.unwrap_err();
    match &err {
        DolaError::ModeSwitch { want, shows } => {
            assert_eq!(want, "fast (高速/Nhanh/Fast)");
            assert_eq!(shows, "プロ");
        }
        other => panic!("sai bien the: {other:?}"),
    }
    assert_eq!(
        err.to_string(),
        "Cannot switch Dola mode to fast (高速/Nhanh/Fast); button shows \"プロ\""
    );
    // 2 vong x (700 + 1500) ms
    assert!(
        t0.elapsed() >= Duration::from_millis(4400),
        "{:?}",
        t0.elapsed()
    );
    let log = fx.log().await;
    assert_eq!(count(&log, "click mode-btn"), 2, "{log}");
    assert!(!has_menu(&fx).await);
    fx.close().await;
}

/// :1995-2028 — khong co nut: cho 8 s (3 ung vien x <=5 s), go the (none), Escape + reload + 5 s,
/// cho 15 s, roi `MODE_UNAVAILABLE` voi url + buttons seen + anh chup `logs/mode_fail_<ts>.png`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_mode_khong_co_nut_bao_mode_unavailable() {
    let fx = Fx::open("compose.html?nomode=1").await;
    let t0 = std::time::Instant::now();
    let err = select_mode(fx.page(), Mode::Fast).await.unwrap_err();
    let DolaError::ModeUnavailable(msg) = &err else {
        panic!("sai bien the: {err:?}");
    };
    let expect_prefix = format!(
        "Dola page shows no 高速/Nhanh/Fast/プロ/Pro mode button even after reload (url {}; buttons seen: ['動画を作成']; screenshot logs",
        fx.url("compose.html?nomode=1")
    );
    assert!(msg.starts_with(&expect_prefix), "{msg}");
    assert!(
        err.to_string()
            .starts_with("MODE_UNAVAILABLE: Dola page shows no")
    );
    let shot = msg
        .rsplit("screenshot ")
        .next()
        .unwrap()
        .trim_end_matches(')');
    assert!(
        shot.starts_with("logs") && std::path::Path::new(shot).extension() == Some("png".as_ref()),
        "{shot}"
    );
    let meta = std::fs::metadata(shot).unwrap_or_else(|e| panic!("anh {shot}: {e}"));
    assert!(meta.len() > 0);
    let _ = std::fs::remove_file(shot);
    assert_eq!(loads(&fx).await, 2, "dung 1 lan tai lai");
    // 15 s + 5 s settle + 15 s (+ reload/screenshot)
    assert!(
        t0.elapsed() >= Duration::from_secs(35),
        "{:?}",
        t0.elapsed()
    );
    // Sau reload log chi con dong "load 2" (khong Escape vi khong co menu; khong bam gi)
    let log = fx.log().await;
    assert!(log.starts_with("load 2 ?nomode=1"), "{log}");
    assert!(!log.contains("click"), "{log}");
    fx.close().await;
}

/// :1997-2012 — composer con mo + the ky nang che thanh nut che do: go the (SVG), xoa nhap
/// (Ctrl+A, Delete), 800 ms, tim lai nut -> khong can reload.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_mode_go_the_ky_nang_roi_thay_nut() {
    let fx = Fx::open("compose.html?tag=1").await;
    let p = fx.page();
    assert_eq!(
        text_of(&fx, "mode-btn").await,
        None,
        "fixture: chua co nut che do"
    );
    assert_eq!(fx.textarea().await, "nhap cu");
    let t0 = std::time::Instant::now();
    let label = select_mode(p, Mode::Fast).await.unwrap();
    assert_eq!(label, "高速");
    // 3 ung vien x 5 s attached + 800 ms
    assert!(
        t0.elapsed() >= Duration::from_millis(15_800),
        "{:?}",
        t0.elapsed()
    );
    assert!(
        t0.elapsed() < Duration::from_secs(30),
        "khong reload: {:?}",
        t0.elapsed()
    );
    assert_eq!(loads(&fx).await, 1, "khong tai lai");
    assert_eq!(fx.textarea().await, "", "nhap cu phai bi xoa");
    let log = fx.log().await;
    assert!(log.contains("tag cleared"), "{log}");
    assert!(
        pos(&log, "tag cleared") < pos(&log, "keydown Delete"),
        "{log}"
    );
    assert!(log.contains("keydown A code=KeyA ctrl=true"), "{log}");
    assert_eq!(count(&log, "click mode-btn"), 0, "da la 高速 -> khong bam");
    fx.close().await;
}

/// :2053-2080 — bam 動画を作成 1 lan, 1500 ms, thay nut 10s/比率; goi lai -> "already active".
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn open_composer_fast_hien_nut_10s() {
    let fx = Fx::open("compose.html").await;
    let p = fx.page();
    assert_eq!(text_of(&fx, "dur-btn").await, None);
    let t0 = std::time::Instant::now();
    open_video_composer_fast(p).await.unwrap();
    assert!(t0.elapsed() >= Duration::from_millis(1500));
    assert_eq!(text_of(&fx, "dur-btn").await.as_deref(), Some("10s"));
    assert_eq!(
        text_of(&fx, "ratio-btn").await.as_deref(),
        Some("比率\n9:16")
    );
    assert_eq!(count(&fx.log().await, "click create-btn"), 1);
    // Da mo -> khong bam lai (bam lai se dong)
    open_video_composer_fast(p).await.unwrap();
    assert_eq!(count(&fx.log().await, "click create-btn"), 1);
    assert_eq!(text_of(&fx, "dur-btn").await.as_deref(), Some("10s"));
    fx.close().await;
}

/// :2081-2089 — nut 動画を作成 an: fallback `text=動画を作成` cho 5/10/15 s deu truot -> 3 lan ->
/// `MODE_UNAVAILABLE: không mở được composer Nhanh ...`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn open_composer_fast_khong_co_nut_bao_mode_unavailable() {
    let fx = Fx::open("compose.html?create=hidden").await;
    let t0 = std::time::Instant::now();
    let err = open_video_composer_fast(fx.page()).await.unwrap_err();
    let DolaError::ModeUnavailable(msg) = &err else {
        panic!("sai bien the: {err:?}");
    };
    assert!(
        msg.starts_with("không mở được composer Nhanh (nút 動画を作成) sau 3 lần ("),
        "{msg}"
    );
    // click timeout 5+10+15 s + nghi 1.5+3+4.5 s
    assert!(
        t0.elapsed() >= Duration::from_secs(39),
        "{:?}",
        t0.elapsed()
    );
    assert_eq!(count(&fx.log().await, "click create-btn"), 0);
    fx.close().await;
}

/// :794-833 — 2 the dinh kem cu: hover -> 250 ms -> bam goc phai tren (4,4) -> the mat.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn clear_composer_attachments_xoa_the_cu() {
    let fx = Fx::open("compose.html?attach=2").await;
    let p = fx.page();
    let n0: i64 = js(
        &fx,
        "document.querySelectorAll('[class*=\"image-container-\"] [class*=\"container-\"]').length",
    )
    .await;
    assert_eq!(n0, 2);
    let removed = clear_composer_attachments(p).await.unwrap();
    assert_eq!(removed, 2);
    let n1: i64 = js(
        &fx,
        "document.querySelectorAll('[class*=\"image-container-\"] [class*=\"container-\"]').length",
    )
    .await;
    assert_eq!(n1, 0);
    let log = fx.log().await;
    assert_eq!(count(&log, "attach removed"), 2, "{log}");
    assert!(count(&log, "mouseenter attach") >= 2, "{log}");
    // hover truoc, bam sau (moi the)
    assert!(
        pos(&log, "mouseenter attach") < pos(&log, "attach removed"),
        "{log}"
    );
    // Khong con the -> 0, khong loi
    assert_eq!(clear_composer_attachments(p).await.unwrap(), 0);
    fx.close().await;
}

/// :2136-2169 — nut model tim qua `finder_text_exact("モデル 2.0高速")` (`MODEL_BTN_RE` JS `\b` khong
/// khop), chua co "2.5" -> bam -> option "Dreamina Seedance 2.5" -> nut doi.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_model_2_5() {
    let fx = Fx::open("compose.html?open=1").await;
    let p = fx.page();
    assert_eq!(
        text_of(&fx, "model-btn").await.as_deref(),
        Some("モデル\n2.0高速")
    );
    // Da la 2.0 -> khong bam
    select_model(p, Model::Seedance20).await.unwrap();
    assert_eq!(count(&fx.log().await, "click model-btn"), 0);

    select_model(p, Model::Seedance25).await.unwrap();
    assert_eq!(
        text_of(&fx, "model-btn").await.as_deref(),
        Some("モデル\n2.5")
    );
    assert!(!has_menu(&fx).await);
    let log = fx.log().await;
    assert_eq!(count(&log, "click model-btn"), 1, "{log}");
    assert!(log.contains("model Dreamina Seedance 2.5"), "{log}");
    // Da 2.5 -> khong bam nua
    select_model(p, Model::Seedance25).await.unwrap();
    assert_eq!(count(&fx.log().await, "click model-btn"), 1);
    // Ve 2.0: option dau tien "Dreamina Seedance 2.0高速"
    select_model(p, Model::Seedance20).await.unwrap();
    assert_eq!(
        text_of(&fx, "model-btn").await.as_deref(),
        Some("モデル\n2.0高速")
    );
    assert!(fx.log().await.contains("model Dreamina Seedance 2.0高速"));
    fx.close().await;
}

/// :2176-2207 — nut 比率 9:16 -> bam -> 600 ms -> menuitem 16:9 -> 400 ms -> dong menu -> nut
/// "比率\n16:9".
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_ratio_16_9() {
    let fx = Fx::open("compose.html?open=1").await;
    let p = fx.page();
    // Da chua 9:16 -> khong bam
    select_ratio(p, Ratio::R9x16).await.unwrap();
    assert_eq!(count(&fx.log().await, "click ratio-btn"), 0);

    let t0 = std::time::Instant::now();
    select_ratio(p, Ratio::R16x9).await.unwrap();
    assert!(t0.elapsed() >= Duration::from_secs(1), "{:?}", t0.elapsed());
    assert_eq!(
        text_of(&fx, "ratio-btn").await.as_deref(),
        Some("比率\n16:9")
    );
    assert!(!has_menu(&fx).await);
    let log = fx.log().await;
    assert_eq!(count(&log, "click ratio-btn"), 1, "{log}");
    assert!(log.contains("ratio 16:9"), "{log}");
    // Da 16:9 -> khong bam
    select_ratio(p, Ratio::R16x9).await.unwrap();
    assert_eq!(count(&fx.log().await, "click ratio-btn"), 1);
    fx.close().await;
}

/// :2202-2207 — menu khong co muc: 3 lan (click timeout 3 s) -> `RATIO_UNAVAILABLE`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_ratio_khong_co_menuitem_bao_ratio_unavailable() {
    let fx = Fx::open("compose.html?open=1&ratiomenu=empty").await;
    let t0 = std::time::Instant::now();
    let err = select_ratio(fx.page(), Ratio::R16x9).await.unwrap_err();
    let DolaError::RatioUnavailable(msg) = &err else {
        panic!("sai bien the: {err:?}");
    };
    assert!(
        msg.starts_with("không chọn được tỉ lệ 16:9 trên giao diện Nhanh sau 3 lần ("),
        "{msg}"
    );
    assert!(
        err.to_string()
            .starts_with("RATIO_UNAVAILABLE: không chọn được")
    );
    // 3 x (600 + 3000 + 700) ms
    assert!(
        t0.elapsed() >= Duration::from_millis(12_900),
        "{:?}",
        t0.elapsed()
    );
    let log = fx.log().await;
    assert_eq!(count(&log, "click ratio-btn"), 3, "{log}");
    assert!(!has_menu(&fx).await, "menu phai dong sau loi");
    fx.close().await;
}

/// :1879-1922 — nut 10s, muon 15: cho 10 x 500 ms -> bam -> 600 ms -> muc 15s -> 400 ms -> "15s".
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_fast_duration_15() {
    let fx = Fx::open("compose.html?open=1").await;
    let p = fx.page();
    let t0 = std::time::Instant::now();
    select_fast_duration(p, 15).await.unwrap();
    assert!(t0.elapsed() >= Duration::from_secs(6), "{:?}", t0.elapsed());
    assert_eq!(text_of(&fx, "dur-btn").await.as_deref(), Some("15s"));
    assert!(!has_menu(&fx).await);
    let log = fx.log().await;
    assert_eq!(count(&log, "click dur-btn"), 1, "{log}");
    assert!(log.contains("duration 15s"), "{log}");
    // Da 15s -> khop ngay o vong dau, khong bam
    let t0 = std::time::Instant::now();
    select_fast_duration(p, 15).await.unwrap();
    assert!(t0.elapsed() < Duration::from_secs(2), "{:?}", t0.elapsed());
    assert_eq!(count(&fx.log().await, "click dur-btn"), 1);

    // Muon 30 ma menu khong co -> doc lai van "15s" -> DURATION_UNAVAILABLE, menu da dong
    let err = select_fast_duration(p, 30).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "DURATION_UNAVAILABLE: requested 30s, selector shows '15s'"
    );
    assert!(!has_menu(&fx).await);
    assert_eq!(count(&fx.log().await, "click dur-btn"), 2);
    fx.close().await;
}

/// Ruling NOTES §3.7: khong co nut thoi luong ma yeu cau 10 s -> Ok; 15 s -> `DURATION_UNAVAILABLE`
/// `selector shows ''` (nhu Python).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn select_fast_duration_10_khong_co_nut_van_ok() {
    let fx = Fx::open("compose.html?open=1&dur=none").await;
    let p = fx.page();
    assert_eq!(text_of(&fx, "dur-btn").await, None);
    select_fast_duration(p, 10).await.unwrap();
    let err = select_fast_duration(p, 15).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "DURATION_UNAVAILABLE: requested 15s, selector shows ''"
    );
    fx.close().await;
}

/// ADR-044: hook o MAIN world doi `duration` trong body `/chat/completion` truoc khi gui ->
/// `data-lvc-hook = patched:15`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn hook_patch_duration_15() {
    let fx = Fx::open("compose.html").await;
    let p = fx.page();
    assert_eq!(hook_state(p).await.unwrap(), "");
    install_duration_hook(p, 15).await.unwrap();
    assert_eq!(hook_state(p).await.unwrap(), "installed:15");
    // Chua co request -> chua patched (20 x 250 ms)
    let t0 = std::time::Instant::now();
    assert!(!verify_hook_patched(p, 15).await.unwrap());
    assert!(t0.elapsed() >= Duration::from_secs(5), "{:?}", t0.elapsed());
    // fetch that qua chuoi hook -> stub fixture -> server 200
    let status: i64 = js(
        &fx,
        "window.fetch('/chat/completion', {method: 'POST', body: JSON.stringify({chat_ability: {ability_param: JSON.stringify({model: 'seedance_v2.5', duration: 10})}})}).then(r => r.status)",
    )
    .await;
    assert_eq!(status, 200);
    assert!(verify_hook_patched(p, 15).await.unwrap());
    assert_eq!(hook_state(p).await.unwrap(), "patched:15");
    let log = fx.log().await;
    let line = log
        .lines()
        .find(|l| l.starts_with("fetch /chat/completion"))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(line.contains(r#"\"duration\":15"#), "{line}");
    assert!(!line.contains(r#"\"duration\":10"#), "{line}");
    // URL khac -> khong doi
    let _: i64 = js(
        &fx,
        "window.fetch('/im/chain/recent_conv', {method: 'POST', body: '{\"chat_ability\":{\"duration\":10}}'}).then(r => r.status)",
    )
    .await;
    assert!(
        fx.log()
            .await
            .contains("fetch /im/chain/recent_conv body={\"chat_ability\":{\"duration\":10}}")
    );
    // Cai lai (hook da co) -> installed:15, khong boc them lan nua
    install_duration_hook(p, 15).await.unwrap();
    assert_eq!(hook_state(p).await.unwrap(), "installed:15");
    // M-4 (review Task 7): body /chat/completion KHONG co `chat_ability` -> hook de nguyen
    // (`patch` chi doi khi `body.includes('chat_ability')`), state van `installed:15`.
    let status: i64 = js(
        &fx,
        "window.fetch('/chat/completion', {method: 'POST', body: '{\"duration\":10}'}).then(r => r.status)",
    )
    .await;
    assert_eq!(status, 200);
    let log = fx.log().await;
    let plain = log
        .lines()
        .filter(|l| l.starts_with("fetch /chat/completion"))
        .nth(1)
        .unwrap_or_else(|| panic!("{log}"));
    assert_eq!(plain, "fetch /chat/completion body={\"duration\":10}");
    assert_eq!(
        hook_state(p).await.unwrap(),
        "installed:15",
        "khong co chat_ability -> khong patched"
    );
    assert!(!verify_hook_patched(p, 15).await.unwrap());
    assert_eq!(hook_state(p).await.unwrap(), "installed:15");
    // Cai so khac -> installed:30, sau do khong phai patched:15
    install_duration_hook(p, 30).await.unwrap();
    assert_eq!(hook_state(p).await.unwrap(), "installed:30");
    fx.close().await;
}

/// :2117-2133 — dong menu -> bam o soan -> Ctrl+A, Delete -> insertText -> 600 ms -> Enter; fixture
/// gui `/chat/completion` khi Enter -> hook doi duration.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn type_and_send_ghi_log_enter() {
    let fx = Fx::open("compose.html?open=1").await;
    let p = fx.page();
    install_duration_hook(p, 15).await.unwrap();
    // Chu cu con trong o (session am) phai bi xoa truoc khi go
    let _: serde_json::Value =
        js(&fx, "document.getElementById('prompt').value = 'cu'; null").await;
    // Mo menu de chac type_and_send dong truoc
    assert!(
        ops::click_js(p, "document.getElementById('mode-btn')")
            .await
            .unwrap()
    );
    assert!(has_menu(&fx).await);
    let text = "xin chào 日本語\ndòng 2";
    let t0 = std::time::Instant::now();
    type_and_send(p, text, false).await.unwrap();
    assert!(t0.elapsed() >= Duration::from_millis(600));
    let log = fx.log().await;
    let esc = pos(&log, "keydown Escape");
    let click = pos(&log, "click prompt");
    let ctrl = pos(&log, "keydown A code=KeyA ctrl=true");
    let del = pos(&log, "keydown Delete");
    let ins = pos(&log, "input insertText");
    let enter = pos(&log, "keydown Enter");
    assert!(
        esc < click && click < ctrl && ctrl < del && del < ins && ins < enter,
        "{log}"
    );
    assert!(
        log.contains(&format!("send {}", serde_json::to_string(text).unwrap())),
        "{log}"
    );
    assert!(!log.contains("send \"cu"), "chu cu phai bi xoa: {log}");
    assert_eq!(fx.textarea().await, "", "fixture xoa o sau khi gui");
    assert!(!has_menu(&fx).await);
    // Enter -> fixture fetch /chat/completion -> hook doi 10 -> 15
    assert!(verify_hook_patched(p, 15).await.unwrap());
    assert_eq!(hook_state(p).await.unwrap(), "patched:15");

    // append=True: End roi go noi, KHONG Ctrl+A/Delete (pro: " " + text)
    let _: serde_json::Value =
        js(&fx, "document.getElementById('prompt').value = 'abc'; null").await;
    type_and_send(p, " xyz", true).await.unwrap();
    let log = fx.log().await;
    assert!(log.contains("keydown End"), "{log}");
    assert_eq!(count(&log, "keydown Delete"), 1, "{log}");
    assert!(log.contains("send \"abc xyz\""), "{log}");
    fx.close().await;
}

/// :2349-2367 — the ky nang SVG -> `cleared-svg`; khong con -> `none`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test compose_smoke -- --ignored"]
async fn clear_ability_tag_svg() {
    let fx = Fx::open("compose.html?tag=1").await;
    let p = fx.page();
    assert_eq!(clear_ability_tag(p).await.unwrap(), "cleared-svg");
    assert!(fx.log().await.contains("tag cleared"));
    assert_eq!(clear_ability_tag(p).await.unwrap(), "none");
    fx.close().await;

    let fx = Fx::open("compose.html").await;
    assert_eq!(clear_ability_tag(fx.page()).await.unwrap(), "none");
    fx.close().await;
}
