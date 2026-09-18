//! Smoke test cho `lvc_browser::dola::skill` (Task 12: pro / `pro_max` / agent + `send_chat_message`)
//! tren fixture cuc bo `tests/fixtures/chat_submit.html` (server test phuc vu o `/chat` va
//! `/chat/<id>`; KHONG goi dola.com; cookie `sessionid` GIA chi nam trong profile tam). Tat ca
//! `#[ignore]` vi can Chromium: `cargo test -p lvc-browser --test skill_smoke -- --ignored`.
//!
//! Fixture (query Task 12): `?skill=1` nut スキル -> `[role="listbox"]` `[role="option"]` 動画の生成 ->
//! the ky nang; `?tag=1` the ky nang co san; `?sendbtn=1` Enter KHONG gui, chi nut
//! `button[aria-label="送信"]` (co `<svg>`, dung SAU nut moi `#decoy-svg`) gui; `?nomode=1` khong co
//! nut che do; `?stuck=1` menu che do khong doi duoc nhan; `?create=hidden` an nut 動画を作成.

mod common;

use std::time::{Duration, Instant};

use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use common::Fx;
use lvc_browser::dola::skill::{open_video_composer_pro, send_chat_message};
use lvc_browser::dola::submit::{ConversationId, SubmitCtx, SubmitRequest, submit_at};
use lvc_browser::dola::{DolaError, PollPage};
use lvc_browser::ops;
use lvc_core::{Limits, Mode, Model, Ratio, agent_prompt, pro_max_prompt, pro_prompt};

const ACCOUNT: &str = "acc_test";
const PROMPT: &str = "một con mèo chạy trên bãi biển lúc hoàng hôn";
const CONV_ID: &str = "123456789012";

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

fn req(mode: Mode, model: Model, duration: u16, ratio: Option<Ratio>) -> SubmitRequest {
    SubmitRequest {
        account: ACCOUNT.into(),
        prompt: PROMPT.into(),
        mode,
        ratio,
        duration,
        model,
        reference_images: Vec::new(),
    }
}

async fn js<T: serde::de::DeserializeOwned>(fx: &Fx, expr: &str) -> T {
    ops::eval_json(fx.page(), expr).await.expect("eval")
}

async fn has(fx: &Fx, selector: &str) -> bool {
    js(fx, &format!("!!document.querySelector('{selector}')")).await
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

/// Dong `send "<json>"` trong log -> chuoi da gui (fixture ghi `JSON.stringify(value)`).
fn sent_text(log: &str) -> String {
    let line = log
        .lines()
        .find(|l| l.starts_with("send \""))
        .unwrap_or_else(|| panic!("khong co dong send trong log:\n{log}"));
    serde_json::from_str(&line["send ".len()..]).unwrap_or_else(|e| panic!("{line}: {e}"))
}

/// `:2091-2098`: nut スキル visible -> bam (8 s) -> 1200 ms -> `[role="option"]` 動画の生成 (8 s) ->
/// 1200 ms -> dong menu; the ky nang xuat hien, listbox dong, KHONG bam 動画を作成. `:2099-2103`:
/// khong co スキル -> bam 動画を作成 (5 s) + 1200 ms. Khong co ca hai -> `Ok` (Python khong bao loi).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test skill_smoke -- --ignored"]
async fn open_composer_pro_qua_skill_menu() {
    let fx = Fx::open("chat?skill=1").await;
    let p = fx.page();
    let t0 = Instant::now();
    open_video_composer_pro(p).await.unwrap();
    let dt = t0.elapsed();
    assert!(dt >= Duration::from_millis(2400), "{dt:?}");
    let log = fx.log().await;
    assert!(
        pos(&log, "click skill-btn") < pos(&log, "skill 動画の生成")
            && pos(&log, "skill 動画の生成") < pos(&log, "tag inserted"),
        "{log}"
    );
    assert_eq!(count(&log, "click create-btn"), 0, "{log}");
    assert_eq!(count(&log, "skill "), 1, "chi chon 1 option: {log}");
    assert!(!has(&fx, "[role=\"listbox\"]").await, "listbox phai dong");
    assert!(
        has(&fx, "[data-input-engine-action-source]").await,
        "the ky nang phai co"
    );
    fx.close().await;

    // Khong co スキル -> nut 動画を作成 (VIDEO_BTN_RE) visible -> bam + 1200 ms -> composer mo.
    let fx = Fx::open("chat").await;
    let t0 = Instant::now();
    open_video_composer_pro(fx.page()).await.unwrap();
    assert!(t0.elapsed() >= Duration::from_millis(1200));
    let log = fx.log().await;
    assert_eq!(count(&log, "click create-btn"), 1, "{log}");
    assert!(has(&fx, "#dur-btn").await, "composer Nhanh mo: {log}");
    fx.close().await;

    // Khong co ca hai (動画を作成 an) -> Ok, khong bam gi (Python `:2099-2103` khong `raise`).
    let fx = Fx::open("chat?create=hidden").await;
    let t0 = Instant::now();
    open_video_composer_pro(fx.page()).await.unwrap();
    assert!(t0.elapsed() < Duration::from_secs(1));
    let log = fx.log().await;
    assert_eq!(count(&log, "click "), 0, "{log}");
    fx.close().await;
}

/// `_submit_pro` `:2304-2311` qua `submit_at`: プロ -> スキル -> 動画の生成 -> (khong anh) ->
/// `type_and_send(" " + pro_prompt(prompt, ratio, duration), append=True)`: `End` roi `insertText`
/// bat dau bang " " (KHONG Ctrl+A/Delete), Enter -> conversation id. Chuoi gui = " " +
/// `lvc_core::pro_prompt(..., pro_no_bgm)` tung byte.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test skill_smoke -- --ignored"]
async fn submit_pro_go_prompt_append_voi_khoang_trang_dau() {
    let fx = Fx::open("chat?skill=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let id = submit_at(
        &ctx,
        &req(Mode::Pro, Model::Seedance20, 15, Some(Ratio::R9x16)),
        &fx.url("chat?skill=1"),
    )
    .await
    .unwrap();
    assert_eq!(id, ConversationId(CONV_ID.into()));
    let log = fx.log().await;
    let expected = format!(
        " {}",
        pro_prompt(PROMPT, Some(Ratio::R9x16), Some(15), limits.pro_no_bgm)
    );
    assert!(
        expected.starts_with(" 15秒の縦動画（9:16）を1本作ってください。内容: "),
        "{expected}"
    );
    assert_eq!(sent_text(&log), expected, "{log}");
    assert_eq!(count(&log, "send \""), 1, "{log}");
    // thu tu: プロ -> スキル -> option -> the -> End -> insertText " ..." -> Enter -> pushState
    assert!(
        pos(&log, "mode プロ") < pos(&log, "click skill-btn")
            && pos(&log, "click skill-btn") < pos(&log, "skill 動画の生成")
            && pos(&log, "skill 動画の生成") < pos(&log, "keydown End")
            && pos(&log, "keydown End") < pos(&log, "input insertText \" 15秒")
            && pos(&log, "input insertText \" 15秒") < pos(&log, "keydown Enter")
            && pos(&log, "keydown Enter") < pos(&log, &format!("pushState /chat/{CONV_ID}")),
        "{log}"
    );
    // append=True: KHONG xoa o (Ctrl+A/Delete), KHONG mo composer Nhanh, KHONG chon ti le/thoi luong
    assert_eq!(count(&log, "keydown Delete"), 0, "{log}");
    assert_eq!(count(&log, "click create-btn"), 0, "{log}");
    assert_eq!(count(&log, "ratio "), 0, "{log}");
    assert_eq!(count(&log, "duration "), 0, "{log}");
    assert!(
        ops::url(p)
            .await
            .unwrap()
            .ends_with(&format!("/chat/{CONV_ID}")),
        "{}",
        ops::url(p).await.unwrap()
    );
    fx.close().await;
}

/// `_submit_pro_max` `:2314-2323`: dang o プロ, KHONG mo スキル (nut co san nhung khong bam), KHONG
/// 動画を作成; `type_and_send(pro_max_prompt(...), append=False)` -> Ctrl+A/Delete roi go
/// `/Create Video <prompt Nhat>` tung byte (`Limits::pro_max_command`).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test skill_smoke -- --ignored"]
async fn submit_pro_max_khong_mo_skill() {
    let fx = Fx::open("chat?skill=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let id = submit_at(
        &ctx,
        &req(Mode::ProMax, Model::Seedance20, 10, Some(Ratio::R16x9)),
        &fx.url("chat?skill=1"),
    )
    .await
    .unwrap();
    assert_eq!(id, ConversationId(CONV_ID.into()));
    let log = fx.log().await;
    let expected = pro_max_prompt(
        PROMPT,
        Some(Ratio::R16x9),
        Some(10),
        limits.pro_no_bgm,
        &limits.pro_max_command,
    );
    assert!(
        expected.starts_with("/Create Video 10秒の横動画（16:9）"),
        "{expected}"
    );
    assert_eq!(sent_text(&log), expected, "{log}");
    assert!(log.contains("mode プロ"), "pro_max dung nut プロ: {log}");
    assert_eq!(count(&log, "click skill-btn"), 0, "{log}");
    assert_eq!(count(&log, "skill "), 0, "{log}");
    assert_eq!(count(&log, "click create-btn"), 0, "{log}");
    assert_eq!(count(&log, "keydown End"), 0, "{log}");
    assert!(
        pos(&log, "mode プロ") < pos(&log, "keydown Delete")
            && pos(&log, "keydown Delete") < pos(&log, "input insertText \"/Create Video ")
            && pos(&log, "input insertText \"/Create Video ") < pos(&log, "keydown Enter"),
        "{log}"
    );
    assert!(!has(&fx, "[data-input-engine-action-source]").await);
    fx.close().await;
}

/// `_submit_agent` `:2370-2381`: the ky nang con dinh (`?tag=1`) bi go TRUOC khi go
/// (`cleared-svg`, log `tag cleared` truoc `input insertText`), KHONG composer/skill; chuoi gui =
/// `agent_prompt(prompt, ratio, duration, model, style=None)` tung byte (khoi `DIRECTIVE:` +
/// `Content:`). Lan 2 voi `Limits::agent_video_style = "anime"` -> `VIDEO_STYLE=anime`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test skill_smoke -- --ignored"]
async fn submit_agent_xoa_ability_tag_truoc() {
    let fx = Fx::open("chat?tag=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    assert!(has(&fx, "[data-input-engine-action-source]").await);
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let id = submit_at(
        &ctx,
        &req(Mode::Agent, Model::Seedance25, 15, Some(Ratio::R9x16)),
        &fx.url("chat?tag=1"),
    )
    .await
    .unwrap();
    assert_eq!(id, ConversationId(CONV_ID.into()));
    let log = fx.log().await;
    let expected = agent_prompt(
        PROMPT,
        Some(Ratio::R9x16),
        Some(15),
        Model::Seedance25,
        None,
    );
    assert!(
        expected.contains("\nDIRECTIVE: Use Seedance 2.5 ("),
        "{expected}"
    );
    assert!(!expected.contains("VIDEO_STYLE="), "{expected}");
    assert_eq!(sent_text(&log), expected, "{log}");
    assert!(log.contains("mode プロ"), "agent dung nut プロ: {log}");
    assert!(
        pos(&log, "mode プロ") < pos(&log, "tag cleared")
            && pos(&log, "tag cleared") < pos(&log, "keydown Delete")
            && pos(&log, "keydown Delete") < pos(&log, "input insertText \"Create a 9:16 video")
            && pos(&log, "input insertText \"Create a 9:16 video") < pos(&log, "keydown Enter"),
        "{log}"
    );
    assert_eq!(count(&log, "click create-btn"), 0, "{log}");
    assert_eq!(count(&log, "click skill-btn"), 0, "{log}");
    assert!(!has(&fx, "[data-input-engine-action-source]").await);
    assert_eq!(count(&log, "send \""), 1, "{log}");

    // Lan 2 (trang dang o /chat/<id> -> goto /chat?tag=1 + 5 s): style tu config -> VIDEO_STYLE=anime
    let limits2 = Limits {
        agent_video_style: " anime ".into(),
        ..Limits::default()
    };
    let ctx2 = SubmitCtx::new(p, &limits2);
    let id2 = submit_at(
        &ctx2,
        &req(Mode::Agent, Model::Seedance20, 10, None),
        &fx.url("chat?tag=1"),
    )
    .await
    .unwrap();
    assert_eq!(id2, ConversationId(CONV_ID.into()));
    assert_eq!(loads(&fx).await, 2);
    let log = fx.log().await;
    let expected2 = agent_prompt(PROMPT, None, Some(10), Model::Seedance20, Some("anime"));
    assert!(
        expected2.contains("MODEL=Seedance 2.0, VIDEO_STYLE=anime, DIALOGUE_DENSITY"),
        "{expected2}"
    );
    assert_eq!(sent_text(&log), expected2, "{log}");
    assert!(
        pos(&log, "tag cleared") < pos(&log, "input insertText"),
        "{log}"
    );
    fx.close().await;
}

/// `:2477-2485`: `select_mode` loi (khong co nut che do -> `MODE_UNAVAILABLE` sau ca reload) ma
/// `mode == agent` -> chi warn, van go prompt vao chat va gui -> conversation id. Cham (~70 s): khong
/// co nut 高速/プロ nen `ensure_japanese` (khong doan duoc ngon ngu -> dat ja + tai lai) va
/// `wait_settled` (25 s) trong `assert_logged_in` cung cham nhu Python — 2 lan tai lai tong cong.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test skill_smoke -- --ignored"]
async fn submit_agent_nuot_loi_mode_switch() {
    let fx = Fx::open("chat?nomode=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let t0 = Instant::now();
    let id = submit_at(
        &ctx,
        &req(Mode::Agent, Model::Seedance20, 10, None),
        &fx.url("chat?nomode=1"),
    )
    .await
    .unwrap();
    let dt = t0.elapsed();
    assert_eq!(id, ConversationId(CONV_ID.into()));
    // select_mode: 8 s (3 ung vien x 5 s) + reload 5 s + 15 s (3 x 5 s) truoc khi bo cuoc
    assert!(dt >= Duration::from_secs(30), "{dt:?}");
    assert_eq!(
        loads(&fx).await,
        3,
        "ensure_japanese tai lai 1 lan + select_mode tai lai 1 lan"
    );
    // log sau reload (trang tai lai trong select_mode): khong `mode `, van send
    let log = fx.log().await;
    assert!(!has(&fx, "#mode-btn").await);
    assert_eq!(count(&log, "mode "), 0, "{log}");
    assert_eq!(
        sent_text(&log),
        agent_prompt(PROMPT, None, Some(10), Model::Seedance20, None),
        "{log}"
    );
    assert!(
        ops::url(p)
            .await
            .unwrap()
            .ends_with(&format!("/chat/{CONV_ID}")),
        "{}",
        ops::url(p).await.unwrap()
    );
    fx.close().await;
}

/// Doi chung `:2484` `else: raise`: cung loi `select_mode` (`?stuck=1` -> `ModeSwitch`) voi Pro ->
/// lan ra, KHONG go/gui; voi Agent -> nuot, gui.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test skill_smoke -- --ignored"]
async fn submit_pro_khong_nuot_loi_mode_switch_agent_thi_nuot() {
    let fx = Fx::open("chat?stuck=1").await;
    let p = fx.page();
    set_fake_dola_sessionid(p).await;
    let limits = Limits::default();
    let ctx = SubmitCtx::new(p, &limits);
    let err = submit_at(
        &ctx,
        &req(Mode::Pro, Model::Seedance20, 10, None),
        &fx.url("chat?stuck=1"),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, DolaError::ModeSwitch { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "Cannot switch Dola mode to pro (プロ/Pro); button shows \"高速\""
    );
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 0, "{log}");
    assert_eq!(count(&log, "input insertText"), 0, "{log}");
    assert!(log.contains("mode stuck"), "{log}");

    let id = submit_at(
        &ctx,
        &req(Mode::Agent, Model::Seedance20, 10, None),
        &fx.url("chat?stuck=1"),
    )
    .await
    .unwrap();
    assert_eq!(id, ConversationId(CONV_ID.into()));
    let log = fx.log().await;
    assert_eq!(count(&log, "send \""), 1, "{log}");
    assert!(
        sent_text(&log).starts_with("Create a 16:9 video (duration: 10s).\n"),
        "{log}"
    );
    fx.close().await;
}

/// `_send_chat_message` `:1242-1262`: dong menu -> bam o soan -> `insertText` (KHONG Ctrl+A/End) ->
/// 400 ms -> Enter -> 600 ms; o soan CON chu (`?sendbtn=1`: Enter khong gui) -> bam nut CUOI trong
/// `button[aria-label*="Gửi" i], …[*="Send" i], …[*="送信" i], button:has(svg)` (nut 送信, KHONG phai
/// nut moi `#decoy-svg` co svg dung truoc) -> fixture gui. Trang thuong: Enter gui, khong bam nut.
/// `PollPage::send_reply` cua `Page` (Task 11) goi dung ham nay.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test skill_smoke -- --ignored"]
async fn send_chat_message_bam_nut_gui_khi_enter_khong_gui() {
    let fx = Fx::open("chat?sendbtn=1").await;
    let p = fx.page();
    // menu dang mo -> phai dong truoc
    assert!(
        ops::click_js(p, "document.getElementById('mode-btn')")
            .await
            .unwrap()
    );
    assert!(has(&fx, "[role=\"menu\"]").await);
    let text = "Yes, please proceed with a 10s video. 日本語";
    let t0 = Instant::now();
    send_chat_message(p, text).await.unwrap();
    let dt = t0.elapsed();
    assert!(dt >= Duration::from_secs(1), "{dt:?}");
    let log = fx.log().await;
    // (listener cua o soan/nut chay TRUOC listener document -> `enter ignored` truoc `keydown Enter`,
    // `send` truoc `click send-btn`)
    assert!(
        pos(&log, "keydown Escape") < pos(&log, "click prompt")
            && pos(&log, "click prompt") < pos(&log, "input insertText")
            && pos(&log, "input insertText") < pos(&log, "enter ignored")
            && pos(&log, "enter ignored") < pos(&log, "keydown Enter")
            && pos(&log, "keydown Enter") < pos(&log, "send \"")
            && pos(&log, "keydown Enter") < pos(&log, "click send-btn"),
        "{log}"
    );
    assert_eq!(count(&log, "click send-btn"), 1, "{log}");
    assert_eq!(sent_text(&log), text, "{log}");
    assert_eq!(count(&log, "click decoy-svg"), 0, "{log}");
    assert_eq!(count(&log, "keydown Delete"), 0, "{log}");
    assert_eq!(count(&log, "keydown End"), 0, "{log}");
    assert_eq!(fx.textarea().await, "");
    assert!(!has(&fx, "[role=\"menu\"]").await);

    // PollPage::send_reply (poll.rs) -> send_chat_message: cung duong di, bam nut gui lan nua
    PollPage::send_reply(p, "second").await.unwrap();
    let log = fx.log().await;
    assert_eq!(count(&log, "click send-btn"), 2, "{log}");
    assert!(log.contains("send \"second\""), "{log}");
    fx.close().await;

    // Trang thuong: Enter gui -> o trong -> KHONG tim/bam nut gui (khong co nut nao co svg)
    let fx = Fx::open("chat").await;
    send_chat_message(fx.page(), "ok").await.unwrap();
    let log = fx.log().await;
    assert_eq!(sent_text(&log), "ok", "{log}");
    assert_eq!(count(&log, "click "), 1, "chi bam o soan: {log}");
    assert!(log.contains("click prompt"), "{log}");
    fx.close().await;
}
