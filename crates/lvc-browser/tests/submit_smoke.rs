//! Smoke test cho `lvc_browser::dola::submit` tren fixture cuc bo `tests/fixtures/chat_submit.html`
//! (server test phuc vu o `/chat` va `/chat/<id>`; KHONG goi dola.com; cookie `sessionid` GIA chi nam
//! trong profile tam). Tat ca `#[ignore]` (tru test thuan cuoi file) vi can Chromium:
//! `cargo test -p lvc-browser --test submit_smoke -- --ignored`.
//!
//! Fixture: nut che do 高速, 動画を作成 -> composer (モデル 2.0高速 / 10s / 比率 9:16), o soan; Enter ->
//! `send "<text>"` + fetch `/chat/completion` (duration 10) roi tuy query: `pushState('/chat/<id>')`
//! sau 1 s (mac dinh), `?captcha=1|late|jump` (iframe bdcaptcha.html), `?noconv=1`, `?login_after=1`,
//! `?jump=1` (chat CU + `recent_conv` tra chat moi tu lan goi 2), `?dur=none`, `?nofetch=1`.
//! Che do Pro / Pro Max / Agent (Task 12) qua cung `submit_at`: xem `tests/skill_smoke.rs`.

mod common;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use common::{Fx, RECENT_CONV_NEW_ID};
use lvc_browser::dola::DolaError;
use lvc_browser::dola::compose::hook_state;
use lvc_browser::dola::submit::{
    ConversationId, NoopSubmitHooks, SubmitCtx, SubmitHooks, SubmitRequest, submit_at,
};
use lvc_browser::ops;
use lvc_core::{FailureCode, Limits, Mode, Model, Ratio};

/// P3 Task 4: ghi stage; `cancel_at` = thay stage do thi `should_cancel` = true tu do ve sau.
#[derive(Default)]
struct RecHooks {
    stages: Mutex<Vec<String>>,
    cancel: AtomicBool,
    cancel_at: Option<&'static str>,
}

impl RecHooks {
    fn stages(&self) -> Vec<String> {
        self.stages.lock().unwrap().clone()
    }
}

impl SubmitHooks for RecHooks {
    fn on_stage(&self, stage: &str) {
        self.stages.lock().unwrap().push(stage.to_string());
        if self.cancel_at == Some(stage) {
            self.cancel.store(true, Ordering::SeqCst);
        }
    }
    fn should_cancel(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

const ACCOUNT: &str = "acc_test";
const PROMPT: &str = "một con mèo chạy trên bãi biển lúc hoàng hôn";

/// Cookie `sessionid` GIA cho origin dola.com trong profile tam (`assert_logged_in` doc qua
/// `Network.getCookies(urls=[DOLA_URL])`); trang khong bao gio dieu huong toi dola.com.
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

fn req(mode: Mode, duration: u16, ratio: Option<Ratio>) -> SubmitRequest {
    SubmitRequest {
        account: ACCOUNT.into(),
        prompt: PROMPT.into(),
        mode,
        ratio,
        duration,
        model: Model::Seedance20,
        reference_images: Vec::new(),
    }
}

async fn loads(fx: &Fx) -> i64 {
    ops::eval_json(
        fx.page(),
        "parseInt(document.getElementById('loads').textContent, 10)",
    )
    .await
    .unwrap()
}

fn count(log: &str, needle: &str) -> usize {
    log.lines().filter(|l| l.contains(needle)).count()
}

fn pos(log: &str, needle: &str) -> usize {
    log.find(needle)
        .unwrap_or_else(|| panic!("khong thay {needle:?} trong log:\n{log}"))
}

/// Luong Nhanh day du (`generate_video` -> `_submit_fast` -> `_after_submit`): mo tu trang KHAC
/// (`chat.html`) -> goto `/chat` + 5 s -> `assert_logged_in` -> 高速 -> 動画を作成 -> model 2.0 (da
/// dung) -> ti le 16:9 -> 10s (da dung) -> go + Enter -> conversation id tu URL sau 1 s.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_tra_conv_id() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let hooks = RecHooks::default();
    let ctx = SubmitCtx::new(p, &limits).with_hooks(&hooks);
    let t0 = Instant::now();
    let id = submit_at(
        &ctx,
        &req(Mode::Fast, 10, Some(Ratio::R16x9)),
        &fx.url("chat"),
    )
    .await
    .unwrap();
    let dt = t0.elapsed();
    assert_eq!(id, ConversationId("123456789012".into()));
    // P3 Task 4: chuoi stage dung thu tu Python (khong `refs` vi khong co anh)
    assert_eq!(
        hooks.stages(),
        vec![
            "page",
            "mode",
            "composer",
            "prompt",
            "sending",
            "generating"
        ]
    );
    assert_eq!(id.to_string(), "123456789012");
    // goto + 5 s (:2465) + settle/composer/ratio/type + >= 1 nhip
    assert!(dt >= Duration::from_secs(9), "{dt:?}");
    assert!(dt < Duration::from_secs(40), "{dt:?}");
    assert_eq!(loads(&fx).await, 2, "chat.html -> goto /chat = 1 lan tai");
    assert!(
        ops::url(p).await.unwrap().ends_with("/chat/123456789012"),
        "{}",
        ops::url(p).await.unwrap()
    );
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 1, "{log}");
    assert!(
        log.contains(&format!("send {}", serde_json::to_string(PROMPT).unwrap())),
        "{log}"
    );
    assert!(log.contains("ratio 16:9"), "{log}");
    // thu tu: mo composer -> ti le -> gui -> pushState
    assert!(
        pos(&log, "click create-btn") < pos(&log, "ratio 16:9"),
        "{log}"
    );
    assert!(pos(&log, "ratio 16:9") < pos(&log, "send \""), "{log}");
    assert!(
        pos(&log, "send \"") < pos(&log, "pushState /chat/123456789012"),
        "{log}"
    );
    // 10s da dung -> khong bam nut thoi luong; hook khong cai
    assert_eq!(count(&log, "click dur-btn"), 0, "{log}");
    assert_eq!(hook_state(p).await.unwrap(), "");
    fx.close().await;
}

/// R3/R6: iframe `bdcaptcha.html` xuat hien sau khi gui -> `RISK_CONTROL` NGAY o nhip dau, ke ca khi
/// URL DA co conversation id (captcha kiem TRUOC conv id trong moi nhip).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_captcha_bao_risk_control() {
    let fx = Fx::open("chat?captcha=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let t0 = Instant::now();
    let err = submit_at(&ctx, &req(Mode::Fast, 10, None), &fx.url("chat?captcha=1"))
        .await
        .unwrap_err();
    let dt = t0.elapsed();
    assert!(matches!(err, DolaError::RiskControl(_)), "{err:?}");
    assert_eq!(
        err.to_string(),
        "RISK_CONTROL: captcha detected (bdcaptcha.html) — khong giai o P2"
    );
    assert_eq!(err.failure_code(), None);
    assert!(err.rotates_account());
    // Trang da o /chat (1 s, khong goto) -> khong cho 20 s nhu Python
    assert!(dt < Duration::from_secs(20), "{dt:?}");
    assert_eq!(loads(&fx).await, 1, "URL da la /chat -> khong goto");
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 1, "{log}");
    assert!(log.contains("captcha iframe"), "{log}");
    assert!(log.contains("pushState /chat/123456789012"), "{log}");
    let frames = ops::frame_urls(p).await.unwrap();
    assert!(
        frames.iter().any(|u| u.contains("bdcaptcha.html")),
        "{frames:?}"
    );
    fx.close().await;
}

/// Vong 2 I-1: captcha xuat hien SAU khi URL da co conversation id (300 ms sau pushState, ca hai truoc
/// nhip 1 s dau) -> van `RISK_CONTROL` (moi nhip kiem captcha truoc khi doc URL).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_captcha_sau_conv_id_van_bao_risk_control() {
    let fx = Fx::open("chat?captcha=late").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let err = submit_at(
        &ctx,
        &req(Mode::Fast, 10, None),
        &fx.url("chat?captcha=late"),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, DolaError::RiskControl(_)), "{err:?}");
    let log = fx.log().await;
    assert!(
        pos(&log, "pushState /chat/123456789012") < pos(&log, "captcha iframe"),
        "{log}"
    );
    assert!(ops::url(p).await.unwrap().ends_with("/chat/123456789012"));
    fx.close().await;
}

/// Vong 2 I-1c: nhanh chat CU -> `recent_conv` -> goto `/chat/<id moi>` + 2 s; trang moi hien captcha
/// ngay khi tai -> kiem tra `has_captcha` NGAY TRUOC khi tra id phai bat duoc -> `RISK_CONTROL`
/// (khong tra `Ok(id)` de roi poll toi TIMEOUT khong ma).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_captcha_tren_chat_moi_sau_goto_bao_risk_control() {
    let fx = Fx::open("chat?captcha=jump").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let res = submit_at(
        &ctx,
        &req(Mode::Fast, 10, None),
        &fx.url("chat?captcha=jump"),
    )
    .await;
    let err = match res {
        Err(e) => e,
        Ok(id) => panic!("phai RISK_CONTROL, lai tra id {id}"),
    };
    assert!(matches!(err, DolaError::RiskControl(_)), "{err:?}");
    assert!(err.rotates_account());
    let url = ops::url(p).await.unwrap();
    assert_eq!(url, fx.url(&format!("chat/{RECENT_CONV_NEW_ID}")), "{url}");
    assert_eq!(loads(&fx).await, 2);
    let log = fx.log().await;
    assert!(log.contains("captcha iframe"), "{log}");
    fx.close().await;
}

/// `:2589-2600`: het `Limits::no_conversation_timeout_s` (3 s cho test) khong co id -> anh
/// `<logs_dir>/no_conv_<account>_<ts>.png` (PNG that) -> `NO_CONVERSATION` dung thong diep Python
/// (`url` bo query, `anh=` duong dan anh).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_khong_conv_bao_no_conversation() {
    let fx = Fx::open("chat?noconv=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits {
        no_conversation_timeout_s: 3,
        ..Limits::default()
    };
    let logs = tempfile::tempdir().unwrap();
    let logs_dir: PathBuf = logs.path().join("logs");
    assert!(!logs_dir.exists());
    let ctx = SubmitCtx {
        page: p,
        limits: &limits,
        logs_dir: logs_dir.clone(),
        hooks: &NoopSubmitHooks,
    };
    let t0 = Instant::now();
    let err = submit_at(&ctx, &req(Mode::Fast, 10, None), &fx.url("chat?noconv=1"))
        .await
        .unwrap_err();
    let dt = t0.elapsed();
    let DolaError::NoConversation(msg) = &err else {
        panic!("sai bien the: {err:?}");
    };
    let prefix = format!(
        "Dola khong tao hoi thoai sau 3s (url={}, anh=",
        fx.url("chat")
    );
    assert!(msg.starts_with(&prefix), "{msg}");
    assert!(
        err.to_string()
            .starts_with("NO_CONVERSATION: Dola khong tao hoi thoai sau 3s (url="),
        "{err}"
    );
    assert_eq!(err.failure_code(), Some(FailureCode::NoConversation));
    // anh: <logs_dir>\no_conv_acc_test_<ts>.png ton tai, la PNG
    let shot = msg[prefix.len()..].trim_end_matches(')');
    let shot_path = PathBuf::from(shot);
    assert!(shot_path.starts_with(&logs_dir), "{shot}");
    let name = shot_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(name.starts_with("no_conv_acc_test_"), "{name}");
    assert_eq!(
        shot_path.extension().and_then(|e| e.to_str()),
        Some("png"),
        "{name}"
    );
    let bytes = std::fs::read(&shot_path).unwrap_or_else(|e| panic!("{shot}: {e}"));
    assert_eq!(&bytes[..4], b"\x89PNG", "{shot}");
    // 3 nhip x 1 s, khong 20 s captcha
    assert!(dt >= Duration::from_secs(4), "{dt:?}");
    assert!(dt < Duration::from_secs(25), "{dt:?}");
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 1, "{log}");
    assert!(!log.contains("pushState"), "{log}");
    fx.close().await;
}

/// `:2597-2599` (do that 2026-09-17): sau khi gui trang hien ログイン (Dola tu logout) va khong co
/// conversation id -> `LOGGED_OUT` `revoked=true` (KHONG phai `NO_CONVERSATION`), `account` dien dung,
/// `why` chua duong dan anh `no_conv_*`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_khong_conv_ma_hien_login_bao_logged_out_revoked() {
    let fx = Fx::open("chat?noconv=1&login_after=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits {
        no_conversation_timeout_s: 2,
        ..Limits::default()
    };
    let logs = tempfile::tempdir().unwrap();
    let ctx = SubmitCtx {
        page: p,
        limits: &limits,
        logs_dir: logs.path().join("logs"),
        hooks: &NoopSubmitHooks,
    };
    let err = submit_at(
        &ctx,
        &req(Mode::Fast, 10, None),
        &fx.url("chat?noconv=1&login_after=1"),
    )
    .await
    .unwrap_err();
    match &err {
        DolaError::LoggedOut {
            account,
            why,
            revoked,
        } => {
            assert_eq!(account, ACCOUNT);
            assert!(*revoked, "{why}");
            assert!(
                why.starts_with("trang hiện nút đăng nhập ngay sau khi gửi (ảnh "),
                "{why}"
            );
            assert!(why.contains("no_conv_acc_test_"), "{why}");
        }
        other => panic!("sai bien the: {other:?}"),
    }
    assert_eq!(err.failure_code(), Some(FailureCode::LoggedOut));
    assert!(err.rotates_account());
    assert!(fx.log().await.contains("login button shown"));
    fx.close().await;
}

/// `:2569-2587` (do that 17:42): URL roi vao chat CU (`12345` co trong `known`) -> nhip thu 3
/// (`i % 3 == 2`) hoi `recent_conv` -> id dau tien chua co truoc do -> `goto /chat/<id>` + 2 s ->
/// tra id do.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_url_chat_cu_tim_chat_moi_qua_recent_conv() {
    let fx = Fx::open("chat?jump=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let t0 = Instant::now();
    let id = submit_at(&ctx, &req(Mode::Fast, 10, None), &fx.url("chat?jump=1"))
        .await
        .unwrap();
    let dt = t0.elapsed();
    assert_eq!(id, ConversationId(RECENT_CONV_NEW_ID.into()));
    // 3 nhip (i = 0, 1, 2) + goto + 2 s
    assert!(dt >= Duration::from_secs(7), "{dt:?}");
    let url = ops::url(p).await.unwrap();
    assert_eq!(url, fx.url(&format!("chat/{RECENT_CONV_NEW_ID}")), "{url}");
    assert_eq!(loads(&fx).await, 2, "goto /chat/<id> moi = 1 lan tai");
    // trang sau goto la fixture moi (khong query) — log cu da mat, chi con dong load
    let log = fx.log().await;
    assert!(
        log.contains(&format!("load 2 /chat/{RECENT_CONV_NEW_ID}")),
        "{log}"
    );
    fx.close().await;
}

/// `:2279-2288` (ADR-044): UI khong co nut thoi luong ma yeu cau 15 s -> `select_fast_duration(15)`
/// loi -> giu 10 s (ruling §3.7) + `install_duration_hook(15)` -> Enter -> body `/chat/completion`
/// doi `"duration":15` -> `patched:15` -> conversation id nhu thuong.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_15s_khong_co_nut_dung_hook() {
    let fx = Fx::open("chat?dur=none").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let id = submit_at(&ctx, &req(Mode::Fast, 15, None), &fx.url("chat?dur=none"))
        .await
        .unwrap();
    assert_eq!(id, ConversationId("123456789012".into()));
    assert_eq!(hook_state(p).await.unwrap(), "patched:15");
    let log = fx.log().await;
    let line = log
        .lines()
        .find(|l| l.starts_with("fetch /chat/completion"))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(line.contains(r#"\"duration\":15"#), "{line}");
    assert!(!line.contains(r#"\"duration\":10"#), "{line}");
    assert_eq!(count(&log, "send \""), 1, "{log}");
    fx.close().await;
}

/// `:2290-2299`: hook da cai nhung body `/chat/completion` khong bao gio di qua (fixture
/// `?nofetch=1`) -> sau 20 x 250 ms -> `DURATION_UNAVAILABLE` dung thong diep Python voi
/// `state='installed:15'`. Prompt DA gui (Python cung vay — loi sau Enter).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_fast_hook_khong_patch_bao_duration_unavailable() {
    let fx = Fx::open("chat?dur=none&nofetch=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let err = submit_at(
        &ctx,
        &req(Mode::Fast, 15, None),
        &fx.url("chat?dur=none&nofetch=1"),
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "DURATION_UNAVAILABLE: hook không đổi được duration trong body /chat/completion \
         (state='installed:15') — Dola có thể đã đổi định dạng request; video sẽ là 10s nếu tiếp tục"
    );
    assert_eq!(err.failure_code(), Some(FailureCode::DurationUnavailable));
    assert_eq!(hook_state(p).await.unwrap(), "installed:15");
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 1, "{log}");
    assert_eq!(count(&log, "fetch /chat/completion"), 0, "{log}");
    fx.close().await;
}

/// REQ-002: khong co cookie `sessionid` dola.com -> `LOGGED_OUT` TRUOC khi go/gui (khong ton luot);
/// `account` dien dung.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_khong_sessionid_bao_logged_out_truoc_khi_gui() {
    let fx = Fx::open("chat").await;
    let p = fx.page();
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let err = submit_at(&ctx, &req(Mode::Fast, 10, None), &fx.url("chat"))
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "LOGGED_OUT: acc_test: no sessionid cookie");
    assert!(matches!(err, DolaError::LoggedOut { revoked: false, .. }));
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 0, "{log}");
    assert_eq!(count(&log, "click create-btn"), 0, "{log}");
    fx.close().await;
}

/// P3 Task 4 (R4 "truoc submit"): nguoi dung bam Dung khi dang o stage `mode` -> `Cancelled` ngay
/// truoc `prompt`: KHONG go/Enter (`send` = 0 trong log fixture), composer da mo (`click create-btn`
/// = 1), stage dung `page, mode, composer` (khong `prompt`/`sending`), ma `cancelled`, khong xoay.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_huy_truoc_prompt_khong_gui() {
    let fx = Fx::open("chat").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let hooks = RecHooks {
        cancel_at: Some("mode"),
        ..RecHooks::default()
    };
    let ctx = SubmitCtx::new(p, &limits).with_hooks(&hooks);
    let err = submit_at(&ctx, &req(Mode::Fast, 10, None), &fx.url("chat"))
        .await
        .unwrap_err();
    assert!(matches!(err, DolaError::Cancelled), "{err:?}");
    assert_eq!(err.to_string(), "cancelled");
    assert_eq!(err.failure_code(), Some(FailureCode::Cancelled));
    assert!(!err.rotates_account());
    assert_eq!(hooks.stages(), vec!["page", "mode", "composer"]);
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 0, "khong duoc gui: {log}");
    assert_eq!(count(&log, "click create-btn"), 1, "{log}");
    assert!(!log.contains("pushState"), "{log}");
    fx.close().await;
}

/// P3 Task 4 (R4 "truoc page"): co huy bat tu dau -> `Cancelled` truoc khi cham trang (khong goto,
/// khong stage nao, < 1 s).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_huy_truoc_page_khong_dieu_huong() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let hooks = RecHooks::default();
    hooks.cancel.store(true, Ordering::SeqCst);
    let ctx = SubmitCtx::new(p, &limits).with_hooks(&hooks);
    let t0 = Instant::now();
    let err = submit_at(&ctx, &req(Mode::Fast, 10, None), &fx.url("chat"))
        .await
        .unwrap_err();
    assert!(matches!(err, DolaError::Cancelled), "{err:?}");
    assert!(t0.elapsed() < Duration::from_secs(1), "{:?}", t0.elapsed());
    assert!(hooks.stages().is_empty());
    assert_eq!(loads(&fx).await, 1, "khong goto");
    assert!(ops::url(p).await.unwrap().ends_with("/chat.html"));
    fx.close().await;
}

/// Kiem tra thuan `:2399-2403` chay TRUOC moi I/O: khong dat cookie sessionid, khong dieu huong —
/// neu co I/O se ra `LOGGED_OUT`/goto; o day ra `DURATION_UNAVAILABLE` trong < 1 s, trang khong
/// doi (loads = 1, khong `send`).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test submit_smoke -- --ignored"]
async fn submit_precheck_duration_truoc_moi_io_tren_trang() {
    let fx = Fx::open("chat.html").await;
    let p = fx.page();
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let t0 = Instant::now();
    let err = submit_at(&ctx, &req(Mode::Fast, 30, None), &fx.url("chat"))
        .await
        .unwrap_err();
    assert!(t0.elapsed() < Duration::from_secs(1), "{:?}", t0.elapsed());
    assert_eq!(
        err.to_string(),
        "DURATION_UNAVAILABLE: 30s is only available with mode=fast and model seedance-2.5 \
         (Dola caps agent/pro and Seedance 2.0 at 15s)"
    );
    assert_eq!(loads(&fx).await, 1, "khong goto");
    assert!(ops::url(p).await.unwrap().ends_with("/chat.html"));
    fx.close().await;
}

/// Thuan, KHONG can trinh duyet (chay trong `cargo test` thuong): `submit_at` goi
/// `precheck_duration` truoc moi I/O — cung ham, cung thong diep Python `:2399-2403`.
#[test]
fn submit_tu_choi_duration_khong_hop_le_truoc_io() {
    use lvc_browser::dola::submit::{precheck_duration, submit_timeout};
    let r = req(Mode::Fast, 20, None);
    let err = precheck_duration(r.mode, r.model, r.duration).unwrap_err();
    assert_eq!(
        err.to_string(),
        "DURATION_UNAVAILABLE: Dola supports durations of 10s, 15s and 30s"
    );
    assert_eq!(err.failure_code(), Some(FailureCode::DurationUnavailable));
    let r30 = req(Mode::Fast, 30, None);
    assert!(
        precheck_duration(r30.mode, r30.model, r30.duration)
            .unwrap_err()
            .to_string()
            .starts_with("DURATION_UNAVAILABLE: 30s is only available")
    );
    precheck_duration(Mode::Fast, Model::Seedance25, 30).unwrap();
    assert_eq!(
        submit_timeout(&req(Mode::Fast, 10, None), &Limits::default()),
        Duration::from_secs(300)
    );
}
