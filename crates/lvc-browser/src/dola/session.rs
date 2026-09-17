//! Luong "phien" Dola (Task 6) — port `video_worker_ui.py`:
//! - [`ensure_japanese`] `:1143-1167` `ensure_dola_japanese` (R4: chi duong JS `DOLA_SET_JA_JS`, KHONG
//!   port duong Cai dat UI `switch_dola_language_ui` `:1109-1140`);
//! - [`wait_settled`] `:1058-1077` `wait_dola_settled`;
//! - [`assert_logged_in`] `:1170-1209`;
//! - [`recent_conv_ids`] `:667-675`;
//! - [`raise_if_logged_out`] `:1212-1228` `_raise_if_dola_logged_out`;
//! - [`eval_retry`] `:1036-1052` `_eval_retry`.
//!
//! Ghi log qua `tracing` thay cho `print(...)` cua Python; KHONG bao gio log gia tri cookie.

use std::time::Duration;

use chromiumoxide::Page;
use lvc_core::{
    DOLA_URL, REVOKED_HINT, SessionState, classify_session, dola_self_logout, has_dola_sessionid,
    has_login_marker,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::error::DolaError;
use super::labels::{
    DOLA_CHAT_URL, DOLA_LANG_JS, DOLA_SET_JA_JS, MODE_BTN_JS, RECENT_IDS_JS, SESSION_JS,
};
use crate::BrowserError;
use crate::ops::{self, CookieKv};

/// `wait_dola_settled(timeout=25)` mac dinh (`:1058`).
pub const SETTLE_TIMEOUT: Duration = Duration::from_secs(25);
/// Sau `DOLA_SET_JA_JS` + reload: `wait_for_timeout(4000)` (`:1158`); sau goto kiem lai phien
/// (`:1196`).
const RELOAD_SETTLE: Duration = Duration::from_secs(4);
/// `_eval_retry`: `wait_for_load_state("domcontentloaded", timeout=15000)` roi `wait_for_timeout(2000)`.
const RETRY_LOAD_WAIT: Duration = Duration::from_secs(15);
const RETRY_PAUSE: Duration = Duration::from_secs(2);

/// Ket qua `DOLA_LANG_JS` (`:1019-1025`): `ls` localStorage `i18nextLng`, `ck` cookie `i18next`,
/// `ul` cookie `user_language_code`, `jp` co nut 高速/プロ.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct LangState {
    #[serde(default)]
    pub ls: String,
    #[serde(default)]
    pub ck: String,
    #[serde(default)]
    pub ul: String,
    #[serde(default)]
    pub jp: bool,
}

/// Ket qua `SESSION_JS` (`:1005-1016`): nhan la (<= 40 ky tu) + co o nhap. Thieu truong -> mac dinh
/// (Python `info.get("texts", [])`, `bool(info.get("composer"))`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct SessionInfo {
    #[serde(default)]
    pub texts: Vec<String>,
    #[serde(default)]
    pub composer: bool,
}

/// Port `dola_lang_needs_switch` (`:1098-1106`): ngon ngu dang chay (ls > ck > ul, ha thuong,
/// trim) neu KHONG bat dau bang `ja` -> `Some(lang)`; khong co cookie/ls ma co nut 高速 -> `None`
/// (theo locale ja-JP cua trinh duyet, coi la on); khong co gi va khong nut -> `Some("?")`.
#[must_use]
pub fn dola_lang_needs_switch(st: Option<&LangState>) -> Option<String> {
    let st = st?;
    let lang = [&st.ls, &st.ck, &st.ul]
        .into_iter()
        .find(|s| !s.is_empty())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();
    if !lang.is_empty() && !lang.starts_with("ja") {
        return Some(lang);
    }
    if st.jp || !lang.is_empty() {
        None
    } else {
        Some("?".to_string())
    }
}

/// `browser.py:331-333` `cookie_value`: gia tri dau tien KHONG rong cua cookie ten `name`; khong
/// co -> `""`.
#[must_use]
pub fn cookie_value<'a>(cookies: &'a [CookieKv], name: &str) -> &'a str {
    cookies
        .iter()
        .find(|c| c.name == name && !c.value.is_empty())
        .map_or("", |c| c.value.as_str())
}

/// `has_sid = bool(cookie_value(cookies, "sessionid"))` (`:1179`) tren cookie da loc theo
/// `DOLA_URL` — luat thuan `lvc_core::has_dola_sessionid` (them dieu kien domain chua `dola.com`,
/// vo hai vi `Network.getCookies(urls=[DOLA_URL])` da loc).
#[must_use]
pub fn has_sessionid(cookies: &[CookieKv]) -> bool {
    has_dola_sessionid(
        cookies
            .iter()
            .map(|c| (c.domain.as_str(), c.name.as_str(), c.value.as_str())),
    )
}

/// `_eval_retry` (`:1044`): chi thu lai khi `"context was destroyed" in str(e)` hoac
/// `"navigation" in str(e).lower()`; them "Cannot find context" (cach chromiumoxide dien dat cung
/// tinh huong, xem `ops::context_gone`).
#[must_use]
pub fn retryable_eval_error(e: &BrowserError) -> bool {
    ops::context_gone(e) || e.to_string().to_lowercase().contains("navigation")
}

/// Port `_eval_retry` (`:1036-1052`): `page.evaluate` chiu duoc trang tu dieu huong (Dola tu tai
/// lai khi dong bo ngon ngu/phien). Moi lan thu boc `timeout`; loi dieu huong -> cho
/// `document.readyState != 'loading'` (toi da 15 s, bo qua ket qua) + ngu 2 s roi thu lai; loi
/// khac (JS, timeout) -> tra ngay; het `tries` -> loi cuoi.
///
/// # Errors
/// Loi cuoi cung sau `tries` lan, hoac loi khong-thu-lai dau tien.
pub async fn eval_retry<T: DeserializeOwned>(
    page: &Page,
    js_fn: &str,
    timeout: Duration,
    tries: u32,
) -> Result<T, BrowserError> {
    let mut last = BrowserError::Timeout(format!("eval_retry: tries={tries}"));
    for _ in 0..tries.max(1) {
        let attempt = tokio::time::timeout(timeout, ops::eval_fn::<T>(page, js_fn)).await;
        let err = match attempt {
            Ok(Ok(v)) => return Ok(v),
            Ok(Err(e)) => e,
            Err(_elapsed) => BrowserError::Timeout(format!("evaluate qua {timeout:?}")),
        };
        if !retryable_eval_error(&err) {
            return Err(err);
        }
        last = err;
        let _ = ops::wait_until(
            page,
            "document.readyState !== 'loading'",
            RETRY_LOAD_WAIT,
            Duration::from_millis(200),
        )
        .await;
        tokio::time::sleep(RETRY_PAUSE).await;
    }
    Err(last)
}

/// Port `ensure_dola_japanese` (`:1143-1167`, R4): doc `DOLA_LANG_JS`; ngon ngu != ja ->
/// `DOLA_SET_JA_JS` (xoa localStorage/cookie ngon ngu) -> `reload` + 4 s -> doc lai de log. Tra
/// `Ok(true)` neu DA PHAI doi (Python tra bool). KHONG port duong Cai dat UI (`:1109-1140`).
///
/// Khac Python mot diem: Python nuot MOI loi (ke ca khi khong doc duoc `DOLA_LANG_JS`, tra False);
/// o day loi doc trang thai ban dau -> `Err` de caller biet trang hong ([`assert_logged_in`] tu
/// nuot va di tiep nhu Python); loi trong buoc doi/reload van chi `warn` nhu Python (`:1163`).
///
/// # Errors
/// Loi CDP/JS khi doc `DOLA_LANG_JS` (sau retry).
pub async fn ensure_japanese(page: &Page) -> Result<bool, BrowserError> {
    let st: LangState = eval_retry(page, DOLA_LANG_JS, Duration::from_secs(10), 4).await?;
    let Some(cur) = dola_lang_needs_switch(Some(&st)) else {
        return Ok(false); // tieng Nhat roi -> khong dung gi
    };
    tracing::info!(
        lang = %cur,
        mode_button = st.jp,
        "giao dien Dola khong phai tieng Nhat -> xoa cookie ngon ngu, tai lai (R4)"
    );
    let switched: Result<(), BrowserError> = async {
        let _: serde_json::Value = ops::eval_fn(page, DOLA_SET_JA_JS).await?;
        ops::reload(page, RELOAD_SETTLE).await?;
        Ok(())
    }
    .await;
    match switched {
        Ok(()) => {
            // Doc lai chi de log (Python :1159-1162, loi -> pass). M-2: loi doc lai KHONG duoc
            // bao la "tieng Nhat".
            match eval_retry::<LangState>(page, DOLA_LANG_JS, Duration::from_secs(10), 4).await {
                Ok(st) => match dola_lang_needs_switch(Some(&st)) {
                    None => tracing::info!("sau khi doi: tieng Nhat"),
                    Some(l) => tracing::warn!(
                        lang = %l,
                        "sau khi doi van chua phai tieng Nhat - nhan tieng Nhat co the khong khop"
                    ),
                },
                Err(e) => {
                    tracing::debug!(error = %e, "khong doc lai duoc ngon ngu sau khi doi");
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "dat ngon ngu Dola loi"),
    }
    Ok(true)
}

/// Port `wait_dola_settled` (`:1058-1077`): moi 1 s danh gia `MODE_BTN_JS` (qua [`eval_retry`]
/// timeout 5 s, 2 lan; loi -> false); xong khi nut che do CO va URL dung yen 2 nhip lien tiep ->
/// `Ok(true)`; het `timeout` -> `Ok(false)` (di tiep, nhu Python).
///
/// # Errors
/// Loi CDP khi doc URL trang.
pub async fn wait_settled(page: &Page, timeout: Duration) -> Result<bool, BrowserError> {
    let t0 = tokio::time::Instant::now();
    let mut stable = 0_u32;
    let mut last_url = String::new();
    while t0.elapsed() < timeout {
        let ok = eval_retry::<bool>(page, MODE_BTN_JS, Duration::from_secs(5), 2)
            .await
            .unwrap_or(false);
        let url = ops::url(page).await?;
        stable = if ok && url == last_url { stable + 1 } else { 0 };
        last_url = url;
        if stable >= 2 {
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    tracing::warn!(
        timeout_s = timeout.as_secs(),
        "trang Dola chua on dinh (khong thay nut 高速/プロ) - di tiep"
    );
    Ok(false)
}

/// Nhan khop marker dang nhap/khach, toi da `n` (Python `[... if _has_marker(t)][:n]`).
fn marker_hits(texts: &[String], n: usize) -> Vec<String> {
    texts
        .iter()
        .filter(|t| has_login_marker(t))
        .take(n)
        .cloned()
        .collect()
}

/// `why` cua `AccountLoggedOutError` (`:1206`). Danh sach nhan in kieu Python `f"{[...][:3]}"`
/// (`['a', 'b']`, `lvc_core::error::py_repr`) de thong diep khop byte (review Task 6 M-1).
fn logged_out_why(has_sid: bool, texts: &[String]) -> String {
    if has_sid {
        let items: Vec<String> = marker_hits(texts, 3)
            .iter()
            .map(|s| lvc_core::error::py_repr(s))
            .collect();
        format!(
            "login/guest button visible on page after reload: [{}]",
            items.join(", ")
        )
    } else {
        "no sessionid cookie".to_string()
    }
}

/// Port `assert_logged_in` (`:1170-1209`) — REQ-002, ngay sau khi mo trang chat:
/// [`ensure_japanese`] -> [`wait_settled`] 25 s -> cookie `sessionid` (`Network.getCookies(urls=[DOLA_URL])`)
/// -> `SESSION_JS` (chi khi co sessionid; loi -> "indeterminate", `Ok`) -> `classify_session`.
/// `logged_out` MA con sessionid -> goto [`DOLA_CHAT_URL`] + 4 s + `SESSION_JS` (15 s) roi phan
/// loai lai (loi -> unknown); van `logged_out` -> [`DolaError::LoggedOut`] `revoked: false`;
/// `unknown` -> `Ok` (khong ket luan bua, buoc sau tu that bai co ma).
///
/// # Errors
/// `LoggedOut { revoked: false }`; `Browser` khi CDP hong (doc cookie/URL).
pub async fn assert_logged_in(page: &Page, account: &str) -> Result<(), DolaError> {
    assert_logged_in_at(page, account, DOLA_CHAT_URL).await
}

/// Nhu [`assert_logged_in`] nhung URL de tai lai kiem phien la `chat_url` (test smoke tren fixture
/// cuc bo, KHONG goi dola.com).
///
/// # Errors
/// Nhu [`assert_logged_in`].
pub async fn assert_logged_in_at(
    page: &Page,
    account: &str,
    chat_url: &str,
) -> Result<(), DolaError> {
    // 2026-09-17: truoc moi thao tac theo nhan tieng Nhat. Python nuot loi cua buoc nay.
    if let Err(e) = ensure_japanese(page).await {
        tracing::warn!(account, error = %e, "ensure_japanese loi - di tiep");
    }
    // Dola co the tu tai lai sau khi dong bo ngon ngu -> doi trang dung yen.
    wait_settled(page, SETTLE_TIMEOUT).await?;
    let cookies = ops::cookies(page, DOLA_URL).await?;
    let has_sid = has_sessionid(&cookies);
    let mut info = SessionInfo::default();
    if has_sid {
        match eval_retry::<SessionInfo>(page, SESSION_JS, Duration::from_secs(15), 4).await {
            Ok(i) => info = i,
            Err(e) => {
                // trang chua san sang thi de buoc sau xu ly
                tracing::warn!(account, error = %e, "session check indeterminate");
                return Ok(());
            }
        }
    }
    let mut state = classify_session(has_sid, &info.texts, info.composer);
    if state == SessionState::LoggedOut && has_sid {
        // 2026-09-17 (do that, 10 canh/3 profile): phien nen DUNG LAI bao nut ログイン/khach cho tai
        // khoan van dang nhap (token trong tab het han) -> tai lai trang 1 lan roi moi ket luan.
        tracing::warn!(
            account,
            hits = ?marker_hits(&info.texts, 5),
            "trang co nhan dang nhap/khach du con sessionid -> tai lai trang kiem lai"
        );
        let recheck: Result<SessionInfo, BrowserError> = async {
            ops::goto(page, chat_url, RELOAD_SETTLE).await?;
            tokio::time::timeout(
                Duration::from_secs(15),
                ops::eval_fn::<SessionInfo>(page, SESSION_JS),
            )
            .await
            .unwrap_or_else(|_elapsed| {
                Err(BrowserError::Timeout("SESSION_JS qua 15 s".to_string()))
            })
        }
        .await;
        match recheck {
            Ok(i) => {
                info = i;
                state = classify_session(has_sid, &info.texts, info.composer);
            }
            Err(e) => {
                tracing::warn!(account, error = %e, "tai lai kiem phien loi");
                state = SessionState::Unknown;
            }
        }
    }
    if state == SessionState::LoggedOut {
        let why = logged_out_why(has_sid, &info.texts);
        tracing::warn!(account, why = %why, "LOGGED OUT detected -> dong Chrome, xoay tai khoan");
        return Err(DolaError::LoggedOut {
            account: account.to_string(),
            why,
            revoked: false,
        });
    }
    tracing::info!(account, state = state.as_str(), "session");
    Ok(())
}

/// Port `recent_conv_ids` (`:667-675`): `RECENT_IDS_JS` voi `{msToken, fp}` tu cookie `msToken`,
/// `s_v_web_id` (doc qua `Network.getCookies(urls=[DOLA_URL])`), timeout 10 s; chi giu chuoi toan
/// chu so. Loi JS/timeout/ket qua la -> `Ok(vec![])` (Python: "nhan chat theo URL nhu cu").
///
/// # Errors
/// Chi khi khong doc duoc cookie (CDP hong) — Python cung doc cookie NGOAI `try` (`:2469`, goi
/// `recent_conv_ids` o `:2472`).
pub async fn recent_conv_ids(page: &Page) -> Result<Vec<String>, BrowserError> {
    let cookies = ops::cookies(page, DOLA_URL).await?;
    let arg = serde_json::json!({
        "msToken": cookie_value(&cookies, "msToken"),
        "fp": cookie_value(&cookies, "s_v_web_id"),
    });
    let res = tokio::time::timeout(
        Duration::from_secs(10),
        ops::eval_with_arg::<Vec<serde_json::Value>, _>(page, RECENT_IDS_JS, &arg),
    )
    .await;
    let ids = match res {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "khong lay duoc danh sach chat gan day -> nhan chat theo URL nhu cu");
            return Ok(Vec::new());
        }
        Err(_elapsed) => {
            tracing::warn!(
                "khong lay duoc danh sach chat gan day (timeout 10 s) -> nhan chat theo URL nhu cu"
            );
            return Ok(Vec::new());
        }
    };
    Ok(ids
        .into_iter()
        .map(|v| match v {
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        })
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .collect())
}

/// Port `_raise_if_dola_logged_out` (`:1212-1228`): URL chua `from_logout=1` (Dola tu goi
/// `/passport/web/logout/` sau `STREAM_ERROR` 710022002) -> `LoggedOut { revoked: true }`; khong thi
/// `SESSION_JS` (10 s, loi -> `Ok`) va `classify_session(true, ...)` == `logged_out` -> cung
/// `revoked: true`. Goi TRUOC khi nem `RATIO_UNAVAILABLE`/`NO_CONVERSATION`... de bao dung ma.
///
/// # Errors
/// Chi `LoggedOut { revoked: true }`; loi CDP/JS deu nuot (`Ok`).
pub async fn raise_if_logged_out(page: &Page, account: &str, ctx: &str) -> Result<(), DolaError> {
    let Ok(url) = ops::url(page).await else {
        return Ok(());
    };
    if dola_self_logout(&url) {
        return Err(DolaError::LoggedOut {
            account: account.to_string(),
            why: format!("Dola tự đăng xuất {ctx} (from_logout=1). {REVOKED_HINT}"),
            revoked: true,
        });
    }
    let probe = tokio::time::timeout(
        Duration::from_secs(10),
        ops::eval_fn::<SessionInfo>(page, SESSION_JS),
    )
    .await;
    let Ok(Ok(info)) = probe else {
        return Ok(());
    };
    if classify_session(true, &info.texts, info.composer) == SessionState::LoggedOut {
        return Err(DolaError::LoggedOut {
            account: account.to_string(),
            why: format!("trang hiện nút đăng nhập {ctx}. {REVOKED_HINT}"),
            revoked: true,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BrowserError;
    use crate::ops::CookieKv;

    fn st(ls: &str, ck: &str, ul: &str, jp: bool) -> LangState {
        LangState {
            ls: ls.into(),
            ck: ck.into(),
            ul: ul.into(),
            jp,
        }
    }

    /// Port `dola_lang_needs_switch` (`video_worker_ui.py:1098-1106`): uu tien ls > ck > ul; khong
    /// bat dau "ja" -> can doi; khong co gi ma co nut 高速/プロ -> on; khong co gi va khong nut -> "?".
    #[test]
    fn lang_needs_switch_theo_python() {
        assert_eq!(dola_lang_needs_switch(None), None);
        assert_eq!(
            dola_lang_needs_switch(Some(&st("en", "", "", true))),
            Some("en".into())
        );
        assert_eq!(
            dola_lang_needs_switch(Some(&st("ja-JP", "", "", false))),
            None
        );
        assert_eq!(
            dola_lang_needs_switch(Some(&st("JA", "en", "", false))),
            None,
            "ha thuong"
        );
        assert_eq!(
            dola_lang_needs_switch(Some(&st("", "ko", "vi", true))),
            Some("ko".into())
        );
        assert_eq!(
            dola_lang_needs_switch(Some(&st("", "", " vi ", true))),
            Some("vi".into())
        );
        assert_eq!(dola_lang_needs_switch(Some(&st("", "", "", true))), None);
        assert_eq!(
            dola_lang_needs_switch(Some(&st("", "", "", false))),
            Some("?".into())
        );
    }

    #[test]
    fn lang_state_deserialize_tu_dola_lang_js() {
        let v: LangState = serde_json::from_value(
            serde_json::json!({"ls": "en", "ck": "", "ul": "", "jp": false}),
        )
        .unwrap();
        assert_eq!(v, st("en", "", "", false));
    }

    /// `browser.py:331-333` `cookie_value`: gia tri dau tien KHONG rong cua cookie ten `name`.
    #[test]
    fn cookie_value_dau_tien_khong_rong() {
        let cs = vec![
            CookieKv::new(".dola.com", "msToken", ""),
            CookieKv::new("www.dola.com", "msToken", "tok2"),
            CookieKv::new(".dola.com", "s_v_web_id", "fp1"),
        ];
        assert_eq!(cookie_value(&cs, "msToken"), "tok2");
        assert_eq!(cookie_value(&cs, "s_v_web_id"), "fp1");
        assert_eq!(cookie_value(&cs, "sessionid"), "");
        assert_eq!(cookie_value(&[], "msToken"), "");
    }

    /// `has_dola_sessionid` (lvc-core) tren danh sach cookie doc qua CDP — khong can trinh duyet.
    #[test]
    fn has_sessionid_tu_cookie_list() {
        let ok = vec![CookieKv::new(".dola.com", "sessionid", "abc")];
        assert!(has_sessionid(&ok));
        let empty_value = vec![CookieKv::new(".dola.com", "sessionid", "")];
        assert!(!has_sessionid(&empty_value));
        let other_host = vec![CookieKv::new("127.0.0.1", "sessionid", "abc")];
        assert!(!has_sessionid(&other_host));
        assert!(!has_sessionid(&[]));
    }

    /// `_eval_retry` (`:1036-1052`): chi thu lai khi "context was destroyed" hoac co "navigation"
    /// (khong phan biet hoa/thuong); loi JS/timeout khac -> nem ngay.
    #[test]
    fn eval_retry_chi_thu_lai_loi_dieu_huong() {
        let ctx = BrowserError::Cdp(chromiumoxide::error::CdpError::msg(
            "Error -32000: Execution context was destroyed.",
        ));
        assert!(retryable_eval_error(&ctx));
        let nav = BrowserError::Navigation {
            url: "x".into(),
            reason: "Navigation interrupted".into(),
        };
        assert!(retryable_eval_error(&nav));
        let cannot = BrowserError::Cdp(chromiumoxide::error::CdpError::msg(
            "Cannot find context with specified id",
        ));
        assert!(retryable_eval_error(&cannot));
        assert!(!retryable_eval_error(&BrowserError::Js(
            "TypeError: null".into()
        )));
        assert!(!retryable_eval_error(&BrowserError::Timeout(
            "het gio".into()
        )));
    }

    #[test]
    fn session_info_deserialize_thieu_truong_thi_mac_dinh() {
        let v: SessionInfo = serde_json::from_value(
            serde_json::json!({"texts": ["a", "ログイン"], "composer": true}),
        )
        .unwrap();
        assert_eq!(v.texts, vec!["a", "ログイン"]);
        assert!(v.composer);
        let d: SessionInfo = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(d.texts.is_empty() && !d.composer);
    }

    /// Thong diep `why` cua `LoggedOut` (`:1206`): khong sessionid / co nhan dang nhap sau khi tai lai
    /// (toi da 3 nhan khop marker).
    #[test]
    fn logged_out_why_theo_python() {
        assert_eq!(logged_out_why(false, &[]), "no sessionid cookie");
        let texts: Vec<String> = ["Home", "ログイン", "Guest", "Sign in", "Log In", "x"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let why = logged_out_why(true, &texts);
        // M-1: repr danh sach kieu Python `['a', 'b']` (f"{[...][:3]}") — khop byte voi Python
        assert_eq!(
            why,
            "login/guest button visible on page after reload: ['ログイン', 'Guest', 'Sign in']"
        );
        assert!(!why.contains("Log In"), "chi 3 nhan dau: {why}");
        assert_eq!(
            logged_out_why(true, &[]),
            "login/guest button visible on page after reload: []"
        );
        // chuoi co dau nhay don -> repr Python dung nhay kep
        assert_eq!(
            logged_out_why(true, &["Guest's".to_string()]),
            "login/guest button visible on page after reload: [\"Guest's\"]"
        );
    }

    #[test]
    fn hits_marker_toi_da_5() {
        let texts: Vec<String> = (0..8).map(|_| "ログイン".to_string()).collect();
        assert_eq!(marker_hits(&texts, 5).len(), 5);
        assert_eq!(marker_hits(&texts, 3).len(), 3);
        assert!(marker_hits(&["Home".to_string()], 5).is_empty());
    }
}
