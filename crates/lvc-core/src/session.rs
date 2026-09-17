//! Luật "tài khoản đã bị đăng xuất" (HOP-DONG-nhom1 v1, REQ-002). Thuần.

use serde::{Deserialize, Serialize};

/// Chữ đúng nghĩa "đăng nhập"/"khách" trên nhãn; so theo cụm, không phải chuỗi con.
pub const LOGIN_MARKERS: [&str; 9] = [
    "ログイン",
    "Log In",
    "Log in",
    "Sign in",
    "Sign In",
    "登录",
    "登入",
    "ゲスト",
    "Guest",
];

/// JS chạy trong trang: gom nhãn lá (≤40 ký tự, không con) + có ô nhập không.
/// Khớp Python `browser.py` `SESSION_PROBE_JS` từng ký tự.
pub const SESSION_PROBE_JS: &str = r#"
() => {
  const texts = [];
  for (const el of document.querySelectorAll('button, a, [role="button"], h1, h2, h3, div, span')) {
    const t = (el.textContent || '').trim();
    if (t && t.length <= 40 && el.childElementCount === 0) texts.push(t);
    if (texts.length > 400) break;
  }
  const composer = !!(document.querySelector('textarea') || document.querySelector('[contenteditable="true"]'));
  return {texts, composer};
}
"#;

/// M-10: khop Python `browser.py:169` (`DOLA_REGION_RESTRICTED`) — path xuat hien trong URL
/// khi Dola chan theo khu vuc. Duoc dung o ca hai example (`probe_login`, `probe_profile`)
/// thay vi lap lai chuoi literal.
pub const DOLA_REGION_RESTRICTED: &str = "/security/region-restricted";

/// `url` co chua path chan khu vuc cua Dola khong.
#[must_use]
pub fn is_region_restricted(url: &str) -> bool {
    url.contains(DOLA_REGION_RESTRICTED)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    LoggedIn,
    LoggedOut,
    Unknown,
}

impl SessionState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoggedIn => "logged_in",
            Self::LoggedOut => "logged_out",
            Self::Unknown => "unknown",
        }
    }
}

/// `text` là NHÃN một phần tử lá. Khớp khi bắt đầu bằng marker và (với Latin) ký tự
/// sau marker không phải chữ/số; CJK (codepoint > U+2E7F) không cần ranh giới.
#[must_use]
pub fn has_login_marker(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 40 {
        return false;
    }
    for marker in LOGIN_MARKERS {
        let Some(rest) = text.strip_prefix(marker) else {
            continue;
        };
        if rest.is_empty() {
            return true;
        }
        let last_is_cjk = marker.chars().last().is_some_and(|c| (c as u32) > 0x2E7F);
        let next_alnum = rest.chars().next().is_some_and(char::is_alphanumeric);
        if last_is_cjk || !next_alnum {
            return true;
        }
    }
    false
}

/// - không sessionid → `LoggedOut`
/// - có nhãn đăng nhập/khách → `LoggedOut` (Dola cho khách gõ chat nên ô nhập không đủ)
/// - có sessionid + ô nhập + không marker → `LoggedIn`
/// - còn lại → `Unknown` (trang chưa tải xong), caller không được kết luận
#[must_use]
pub fn classify_session(
    has_sessionid: bool,
    page_texts: &[String],
    has_composer: bool,
) -> SessionState {
    if !has_sessionid {
        return SessionState::LoggedOut;
    }
    if page_texts.iter().any(|t| has_login_marker(t)) {
        return SessionState::LoggedOut;
    }
    if has_composer {
        SessionState::LoggedIn
    } else {
        SessionState::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_khop_dau_nhan_khong_dinh_tu_khac() {
        assert!(has_login_marker("Log In"));
        assert!(has_login_marker("ログイン"));
        assert!(
            has_login_marker("ゲストモード"),
            "CJK khong can khoang trang"
        );
        assert!(
            !has_login_marker("login history"),
            "Log in la tien to cua tu khac? 'login' != 'Log in'"
        );
        assert!(
            !has_login_marker("Guestbook"),
            "dinh chu cai sau marker Latin"
        );
        assert!(has_login_marker("Guest"));
        assert!(!has_login_marker(""));
        assert!(
            !has_login_marker(&"x".repeat(41)),
            "nhan > 40 ky tu khong tinh"
        );
    }

    #[test]
    fn is_region_restricted_khop_path_con_lai_thi_khong() {
        assert!(is_region_restricted(
            "https://www.dola.com/security/region-restricted?x=1"
        ));
        assert!(!is_region_restricted("https://www.dola.com/chat"));
    }

    #[test]
    fn khong_sessionid_la_logged_out() {
        assert_eq!(classify_session(false, &[], true), SessionState::LoggedOut);
    }

    #[test]
    fn co_marker_la_logged_out_ke_ca_khi_co_o_nhap() {
        // do that 2026-09-06: trang chua dang nhap co o nhap + nut ログイン
        let texts = vec!["Home".to_string(), "ログイン".to_string()];
        assert_eq!(
            classify_session(true, &texts, true),
            SessionState::LoggedOut
        );
    }

    #[test]
    fn sessionid_va_o_nhap_khong_marker_la_logged_in() {
        let texts = vec!["Home".to_string(), "Settings".to_string()];
        assert_eq!(classify_session(true, &texts, true), SessionState::LoggedIn);
    }

    #[test]
    fn thieu_o_nhap_la_unknown() {
        assert_eq!(classify_session(true, &[], false), SessionState::Unknown);
    }

    #[test]
    fn probe_js_khop_python() {
        // M-8: ghim do dai chinh xac (431 byte, khop Python browser.py:328-340 tung ky tu) —
        // cac assert starts_with/contains ben duoi qua yeu, se khong bat duoc mot doan chen/xoa
        // giua nhung diem neo do.
        assert_eq!(SESSION_PROBE_JS.len(), 431);
        assert!(SESSION_PROBE_JS.starts_with('\n'));
        assert!(SESSION_PROBE_JS.ends_with("}\n"));
        assert!(SESSION_PROBE_JS.contains("el.childElementCount === 0"));
        assert!(SESSION_PROBE_JS.contains("[contenteditable=\"true\"]"));
        assert!(SESSION_PROBE_JS.contains("texts.length > 400"));
    }
}
