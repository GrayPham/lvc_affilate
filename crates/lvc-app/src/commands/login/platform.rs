//! Bảng nền tảng đăng nhập + URL trang đăng nhập (THUẦN, test được) — nền cho B (đăng nhập thủ công:
//! mở đúng trang để người dùng tự đăng nhập). Mỗi nền tảng khớp một `provider` của kho credentials.
#![allow(
    dead_code,
    reason = "nền cho B (đăng nhập thủ công) — dùng khi wiring mở trình duyệt ở bước kế tiếp"
)]

/// Nền tảng đăng nhập hỗ trợ (khớp `provider` của `lvc-vault`).
pub const PLATFORMS: &[&str] = &[
    "dola",
    "google",
    "facebook",
    "apple",
    "gemini",
    "chatgpt_web",
    "museai",
];

/// URL trang đăng nhập của nền tảng (để B mở cho người dùng tự đăng nhập). `None` nếu không rõ.
#[must_use]
pub fn login_url(platform: &str) -> Option<&'static str> {
    Some(match platform {
        "dola" => "https://www.dola.com/",
        "google" | "gemini" => "https://accounts.google.com/",
        "facebook" => "https://www.facebook.com/login/",
        "apple" => "https://appleid.apple.com/",
        "chatgpt_web" => "https://chatgpt.com/auth/login",
        "museai" => "https://muse.ai/",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moi_platform_co_url() {
        for p in PLATFORMS {
            assert!(login_url(p).is_some(), "thiếu URL cho {p}");
        }
        assert_eq!(login_url("khong-co"), None);
        assert!(login_url("gemini").unwrap().contains("google"));
    }
}
