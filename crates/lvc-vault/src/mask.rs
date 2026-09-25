//! Che bí mật để hiển thị trên UI — không bao giờ để lộ plaintext, chỉ gợi ý 4 ký tự cuối.

/// Che một API key / mật khẩu: `••••` + tối đa 4 ký tự cuối. Rỗng → rỗng.
/// Chuỗi rất ngắn (≤4) → toàn `•` (không lộ gì).
#[must_use]
pub fn mask_key(secret: &str) -> String {
    let n = secret.chars().count();
    if n == 0 {
        return String::new();
    }
    if n <= 4 {
        return "•".repeat(n);
    }
    let tail: String = secret.chars().skip(n - 4).collect();
    format!("••••{tail}")
}

/// Mô tả một phiên cookie mà không lộ giá trị: chỉ báo có `sessionid` hay không + số cookie.
/// `cookie_json` là JSON `[{name,value,...}]` (đã mã hoá khi lưu; chỉ gọi hàm này sau khi giải mã
/// TRONG vault, kết quả trả ra UI là chuỗi này, KHÔNG phải cookie).
#[must_use]
pub fn describe_cookies(count: usize, has_sessionid: bool) -> String {
    if count == 0 {
        return "chưa có cookie".to_string();
    }
    let sid = if has_sessionid {
        "có sessionid"
    } else {
        "thiếu sessionid"
    };
    format!("{count} cookie · {sid}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_key_giu_4_ky_tu_cuoi() {
        assert_eq!(mask_key(""), "");
        assert_eq!(mask_key("abcd"), "••••");
        assert_eq!(mask_key("sk-proj-1234"), "••••1234");
        assert!(!mask_key("sk-secret-key-999").contains("secret"));
    }

    #[test]
    fn describe_cookies_khong_lo_gia_tri() {
        assert_eq!(describe_cookies(0, false), "chưa có cookie");
        assert_eq!(describe_cookies(12, true), "12 cookie · có sessionid");
        assert_eq!(describe_cookies(3, false), "3 cookie · thiếu sessionid");
    }
}
