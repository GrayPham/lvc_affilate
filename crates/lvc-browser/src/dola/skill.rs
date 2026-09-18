//! Skill / Pro / Pro Max / Agent (Task 12, P2) — port `video_worker_ui.py` (HEAD 3d07c19):
//!
//! | Rust | Python |
//! |---|---|
//! | [`open_video_composer_pro`] | `open_video_composer(page, "pro")` `:2091-2103` |
//! | [`submit_pro`] | `_submit_pro` `:2304-2311` |
//! | [`submit_pro_max`] | `_submit_pro_max` `:2314-2323` |
//! | [`submit_agent`] | `_submit_agent` `:2370-2381` |
//! | [`send_chat_message`] | `_send_chat_message` `:1242-1262` (bo nhanh `page.send_chat` `:1244-1246` — HTTP cho warm session) |
//! | [`click_send_button_if_text_left`] | `:1254-1262` (`try/except: pass`) |
//! | [`agent_style`] | `:2379` `getattr(config, "AGENT_VIDEO_STYLE", "") or None` |
//!
//! Prompt cua tung che do lay tu `lvc_core` (`modes.py`): `pro_prompt` (`:366-390`), `pro_max_prompt`
//! (`:398-403`, `Limits::pro_max_command`), `agent_prompt` (`:421-451`, model cua yeu cau +
//! `Limits::agent_video_style`). `_clear_composer_attachments`/`_attach_refs` dung chung voi Nhanh:
//! [`submit::clear_old_attachments`], [`submit::attach_refs`] (pro/agent khong co `input[type=file]`
//! -> `refs` tu roi sang PASTE, `refs.rs`).
//!
//! # Khac Python
//! - `_submit_*` in `text[:60]` ra console (`:2311`, `:2323`, `:2381`); o day KHONG log noi dung, chi
//!   so ky tu (chinh sach log P2).
//! - `_send_chat_message` `:1244-1246`: `page.send_chat` (v0.55q, HTTP qua cua so chung) — P2 moi tai
//!   khoan mot Chrome, khong port.
//! - `open_video_composer_pro`: Python KHONG bao loi khi khong co ca スキル lan 動画を作成 (`:2099-2103`
//!   la `if`, khong `else: raise`) — giu nguyen: `Ok(())` + `debug`.

use std::time::Duration;

use chromiumoxide::Page;
use lvc_core::{agent_prompt, pro_max_prompt, pro_prompt};

use super::compose::{self, click_wait, is_visible};
use super::error::DolaError;
use super::labels::{
    COMPOSER_TEXT_JS, SEND_BTN_FINDER, SKILL_BTN_SRC, VIDEO_BTN_SRC, VIDEO_SKILL_OPT_SRC,
    finder_button_text, finder_composer, finder_menu_item_in,
};
use super::submit::{self, SubmitCtx, SubmitRequest};
use crate::BrowserError;
use crate::ops;

// ---------------------------------------------------------------------------------------------
// Hang thoi gian — moi so la literal cua Python (dong ghi ben canh)
// ---------------------------------------------------------------------------------------------

/// `skill_loc.click(timeout=8000)` `:2093`; `option.click(timeout=8000)` `:2096`.
const SKILL_CLICK_TIMEOUT: Duration = Duration::from_secs(8);
/// `wait_for_timeout(1200)` `:2094`, `:2097`, `:2103`.
const SKILL_SETTLE: Duration = Duration::from_millis(1200);
/// Nhanh khong co スキル: `loc.click(timeout=5000)` `:2102`.
const VIDEO_BTN_CLICK_TIMEOUT: Duration = Duration::from_secs(5);
/// `_send_chat_message`: `box.click(timeout=10000)` `:1249`.
const CHAT_BOX_CLICK_TIMEOUT: Duration = Duration::from_secs(10);
/// `wait_for_timeout(400)` `:1251` (truoc Enter) va `wait_for_timeout(600)` `:1253` (sau Enter).
const CHAT_BEFORE_ENTER: Duration = Duration::from_millis(400);
const CHAT_AFTER_ENTER: Duration = Duration::from_millis(600);
/// `send_btn.click(timeout=2000)` `:1260`.
const SEND_BTN_CLICK_TIMEOUT: Duration = Duration::from_secs(2);

/// `:2095` `page.locator('[role="option"]', has_text=...)`.
const SKILL_OPTION_SEL: &str = r#"[role="option"]"#;

// ---------------------------------------------------------------------------------------------
// Ham thuan
// ---------------------------------------------------------------------------------------------

/// `:2379` `style=getattr(config, "AGENT_VIDEO_STYLE", "") or None` — rong -> `None`
/// (`agent_prompt` tu `strip` va roi ve dong `Style:` trong prompt).
#[must_use]
pub fn agent_style(limits: &lvc_core::Limits) -> Option<&str> {
    let s = limits.agent_video_style.as_str();
    (!s.is_empty()).then_some(s)
}

// ---------------------------------------------------------------------------------------------
// Mo composer Pro (skill)
// ---------------------------------------------------------------------------------------------

/// Port `open_video_composer(page, "pro")` (`:2091-2103`): nut `button` khop `SKILL_BTN_SRC`
/// (スキル|Kỹ năng|Skills?) `count() and is_visible()` -> bam (8 s) -> 1200 ms -> `[role="option"]`
/// khop `VIDEO_SKILL_OPT_SRC` (動画の生成|Tạo video|Create Video) bam (8 s) -> 1200 ms ->
/// [`compose::close_open_menus`]. Khong co スキル: nut `VIDEO_BTN_SRC` (動画を作成) visible -> bam (5 s)
/// -> 1200 ms. Khong co ca hai -> `Ok(())` (Python khong `raise`).
///
/// # Errors
/// `Browser` (`Timeout` khi option 動画の生成 khong hien trong 8 s; loi CDP/JS).
pub async fn open_video_composer_pro(page: &Page) -> Result<(), DolaError> {
    let skill = finder_button_text(SKILL_BTN_SRC);
    if is_visible(page, &skill).await? {
        // :2092-2098
        click_wait(page, &skill, SKILL_CLICK_TIMEOUT, "nut スキル").await?;
        tokio::time::sleep(SKILL_SETTLE).await;
        let option = finder_menu_item_in(SKILL_OPTION_SEL, VIDEO_SKILL_OPT_SRC);
        click_wait(page, &option, SKILL_CLICK_TIMEOUT, "option 動画の生成").await?;
        tokio::time::sleep(SKILL_SETTLE).await;
        compose::close_open_menus(page).await?;
        tracing::info!("skill 動画の生成 đã chọn (スキル)");
        return Ok(());
    }
    // :2099-2103
    let video = finder_button_text(VIDEO_BTN_SRC);
    if is_visible(page, &video).await? {
        click_wait(page, &video, VIDEO_BTN_CLICK_TIMEOUT, "nut 動画を作成").await?;
        tokio::time::sleep(SKILL_SETTLE).await;
        tracing::info!("không có nút スキル → đã bấm 動画を作成");
    } else {
        tracing::debug!("không có nút スキル lẫn 動画を作成 → đi tiếp (Python không báo lỗi)");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Gui theo che do
// ---------------------------------------------------------------------------------------------

/// Port `_submit_pro` (`:2304-2311`): [`open_video_composer_pro`] -> [`submit::clear_old_attachments`]
/// -> [`submit::attach_refs`] -> `text = pro_prompt(prompt, ratio, duration)` (`Limits::pro_no_bgm`)
/// -> `type_and_send(" " + text, append=True)` (the ky nang da nam trong o soan: `End` roi go noi,
/// KHONG xoa o).
///
/// # Errors
/// `RefUpload`, `LoggedOut` (account rong — [`submit::submit_at`] dien), `Browser`.
pub async fn submit_pro(ctx: &SubmitCtx<'_>, req: &SubmitRequest) -> Result<(), DolaError> {
    let page = ctx.page;
    let account = req.account.as_str();
    ctx.hooks.on_stage(submit::STAGE_COMPOSER); // P3 Task 4
    open_video_composer_pro(page).await?; // :2306
    submit::clear_old_attachments(page, account).await?; // :2307
    submit::attach_refs(ctx, req).await?; // :2308
    let text = pro_prompt(
        &req.prompt,
        req.ratio,
        Some(req.duration),
        ctx.limits.pro_no_bgm,
    ); // :2309
    submit::before_prompt(ctx, account)?; // P3 R4 + :2122
    compose::type_and_send(page, &format!(" {text}"), true).await?; // :2310
    // :2311 — Python in `text[:60]`; o day KHONG log noi dung
    tracing::info!(
        account,
        chars = text.chars().count(),
        "UI submitted prompt (pro)"
    );
    Ok(())
}

/// Port `_submit_pro_max` (`:2314-2323`, REQ-003/ADR-N1-002): dang o プロ, KHONG mo スキル;
/// [`submit::clear_old_attachments`] -> [`submit::attach_refs`] ->
/// `text = pro_max_prompt(prompt, ratio, duration)` (`Limits::pro_max_command` + `pro_no_bgm`) ->
/// `type_and_send(text, append=False)`.
///
/// # Errors
/// `RefUpload`, `LoggedOut`, `Browser`.
pub async fn submit_pro_max(ctx: &SubmitCtx<'_>, req: &SubmitRequest) -> Result<(), DolaError> {
    let page = ctx.page;
    let account = req.account.as_str();
    submit::clear_old_attachments(page, account).await?; // :2319
    submit::attach_refs(ctx, req).await?; // :2320
    let text = pro_max_prompt(
        &req.prompt,
        req.ratio,
        Some(req.duration),
        ctx.limits.pro_no_bgm,
        &ctx.limits.pro_max_command,
    ); // :2321
    submit::before_prompt(ctx, account)?; // P3 R4 + :2122
    compose::type_and_send(page, &text, false).await?; // :2322
    // :2323 — Python in `text[:60]`; o day KHONG log noi dung
    tracing::info!(
        account,
        chars = text.chars().count(),
        "UI submitted prompt (pro_max)"
    );
    Ok(())
}

/// Port `_submit_agent` (`:2370-2381`, TUT Bo skill): KHONG mo composer, KHONG skill.
/// `_clear_ability_tag` (`:2376` — the 動画を作成 con dinh -> Dola "Illegal request"; loi nuot nhu
/// Python) -> [`submit::clear_old_attachments`] (`:2377`) -> [`submit::attach_refs`] (`:2378`) ->
/// `text = agent_prompt(prompt, ratio, duration, model=req.model, style=AGENT_VIDEO_STYLE or None)`
/// (`:2379`) -> `type_and_send(text, append=False)` (`:2380`).
///
/// # Errors
/// `RefUpload`, `LoggedOut`, `Browser`.
pub async fn submit_agent(ctx: &SubmitCtx<'_>, req: &SubmitRequest) -> Result<(), DolaError> {
    let page = ctx.page;
    let account = req.account.as_str();
    let cleared = compose::clear_ability_tag_lenient(page).await; // :2376
    tracing::info!(account, cleared = %cleared, "gỡ thẻ kỹ năng trước khi gửi Agent");
    submit::clear_old_attachments(page, account).await?; // :2377
    submit::attach_refs(ctx, req).await?; // :2378
    let text = agent_prompt(
        &req.prompt,
        req.ratio,
        Some(req.duration),
        req.model,
        agent_style(ctx.limits),
    ); // :2379
    submit::before_prompt(ctx, account)?; // P3 R4 + :2122
    compose::type_and_send(page, &text, false).await?; // :2380
    // :2381 — Python in `text[:60]!r` + `prompt[:100]!r`; o day KHONG log noi dung
    tracing::info!(
        account,
        chars = text.chars().count(),
        prompt_chars = req.prompt.chars().count(),
        "UI submitted agent prompt"
    );
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Tra loi trong chat dang mo (xac nhan/lua chon cho agent Pro/Agent)
// ---------------------------------------------------------------------------------------------

/// `:1254-1262` (`try/except: pass`): o soan CON chu sau Enter (`COMPOSER_TEXT_JS` `:1256`, `.strip()`
/// `:1257`) -> nut gui `SEND_BTN_FINDER` (`.last`, `:1258`) `count() and is_visible()` (`:1259`) -> bam
/// (2 s, `:1260`). Tra `Ok(true)` khi da bam
/// nut; caller nuot `Err` nhu Python.
async fn click_send_button_if_text_left(page: &Page) -> Result<bool, BrowserError> {
    let left: String = ops::eval_json(page, COMPOSER_TEXT_JS).await?;
    if left.trim().is_empty() {
        return Ok(false);
    }
    if !is_visible(page, SEND_BTN_FINDER).await? {
        tracing::debug!(
            left_chars = left.chars().count(),
            "ô soạn còn chữ sau Enter nhưng không có nút gửi"
        );
        return Ok(false);
    }
    click_wait(page, SEND_BTN_FINDER, SEND_BTN_CLICK_TIMEOUT, "nut gui").await?;
    Ok(true)
}

/// Port `_send_chat_message` (`:1242-1262`): go MOT tin vao o nhap cua hoi thoai dang mo va gui
/// (tra loi xac nhan/lua chon cho agent Pro/Agent — `poll.rs` `PollPage::send_reply`):
/// [`compose::close_open_menus`] -> bam o soan (`finder_composer`, 10 s) -> `Input.insertText`
/// (`_enter_text`, `TYPE_PASTE_THRESHOLD=0` -> luon `insert_text`; KHONG `End`/`Control+A` nhu
/// `_type_and_send`) -> 400 ms -> `Enter` -> 600 ms -> neu o soan con chu -> bam nut gui CUOI
/// ([`click_send_button_if_text_left`]; moi loi o buoc nay chi `debug`, nhu `except: pass`).
/// Khong log noi dung, chi so ky tu.
///
/// # Errors
/// `Timeout` (khong co o soan trong 10 s), loi CDP truoc buoc bam nut gui.
pub async fn send_chat_message(page: &Page, text: &str) -> Result<(), BrowserError> {
    compose::close_open_menus(page).await?; // :1247
    click_wait(page, finder_composer(), CHAT_BOX_CLICK_TIMEOUT, "o soan").await?; // :1248-1249
    ops::insert_text(page, text).await?; // :1250 `_enter_text`
    tokio::time::sleep(CHAT_BEFORE_ENTER).await; // :1251
    ops::press(page, "Enter").await?; // :1252
    tokio::time::sleep(CHAT_AFTER_ENTER).await; // :1253
    // :1254-1262 — nuot loi
    let clicked = match click_send_button_if_text_left(page).await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(error = %e, "kiểm/bấm nút gửi sau Enter lỗi → bỏ qua");
            false
        }
    };
    tracing::info!(
        len = text.chars().count(),
        clicked_send_button = clicked,
        "đã gửi tin nhắn chat"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use lvc_core::Limits;

    use super::*;

    /// Hang thoi gian khop literal Python (`:2093`, `:2094`, `:2102`, `:1249`, `:1251`, `:1253`, `:1260`).
    #[test]
    fn hang_thoi_gian_khop_python() {
        assert_eq!(SKILL_CLICK_TIMEOUT.as_millis(), 8000);
        assert_eq!(SKILL_SETTLE.as_millis(), 1200);
        assert_eq!(VIDEO_BTN_CLICK_TIMEOUT.as_millis(), 5000);
        assert_eq!(CHAT_BOX_CLICK_TIMEOUT.as_millis(), 10000);
        assert_eq!(CHAT_BEFORE_ENTER.as_millis(), 400);
        assert_eq!(CHAT_AFTER_ENTER.as_millis(), 600);
        assert_eq!(SEND_BTN_CLICK_TIMEOUT.as_millis(), 2000);
    }

    /// `:2379` `AGENT_VIDEO_STYLE or None`: rong -> `None`; co -> `Some` (`agent_prompt` tu strip).
    #[test]
    fn agent_style_rong_la_none() {
        assert_eq!(agent_style(&Limits::default()), None);
        let l = Limits {
            agent_video_style: " anime ".into(),
            ..Limits::default()
        };
        assert_eq!(agent_style(&l), Some(" anime "));
        assert_eq!(
            agent_prompt(
                "x",
                None,
                None,
                lvc_core::Model::Seedance20,
                agent_style(&l)
            )
            .lines()
            .nth(1)
            .unwrap_or(""),
            "[PARAMETERS: DISPLAY_FORMAT=16:9, DURATION=10s, SCENE_COUNT=1, MODEL=Seedance 2.0, \
             VIDEO_STYLE=anime, DIALOGUE_DENSITY=minimal, MIN_WORDS_PER_PROMPT=60]"
        );
    }

    /// Finder JS cho skill: `[role="option"]` + regex `VIDEO_SKILL_OPT_SRC` (`:2095`); nut スキル theo
    /// `SKILL_BTN_SRC` (`:2091`); nut gui/o soan la hang trong `labels` (`:1255`, `:1257`).
    #[test]
    fn finder_skill_va_nut_gui() {
        let opt = finder_menu_item_in(SKILL_OPTION_SEL, VIDEO_SKILL_OPT_SRC);
        assert!(
            opt.contains(r#"querySelectorAll('[role="option"]')"#),
            "{opt}"
        );
        assert!(
            opt.contains("/^\\s*(動画の生成|Tạo video|Create Video)/i.test("),
            "{opt}"
        );
        let skill = finder_button_text(SKILL_BTN_SRC);
        assert!(
            skill.contains("/^\\s*(スキル|Kỹ năng|Skills?)\\s*$/i.test("),
            "{skill}"
        );
        for needle in [
            "'gửi'",
            "'send'",
            "'送信'",
            "querySelector('svg')",
            "all[all.length - 1]",
        ] {
            assert!(SEND_BTN_FINDER.contains(needle), "{needle}");
        }
        assert!(COMPOSER_TEXT_JS.contains("tagName.toLowerCase() === 'textarea'"));
        assert!(COMPOSER_TEXT_JS.contains(r#"'textarea, [contenteditable="true"]'"#));
    }
}
