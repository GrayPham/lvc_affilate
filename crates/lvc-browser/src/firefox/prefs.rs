//! Dựng nội dung `user.js` đặt proxy cho profile Firefox (engine `FirefoxPortable`).
//! THUẦN — chỉ sinh chuỗi, không I/O; `lvc-engine` ghi ra `<profile>/user.js`.
//!
//! Firefox đọc proxy từ các pref `network.proxy.*`. Proxy có `user:pass` KHÔNG đặt được qua prefs
//! (Firefox sẽ hỏi user/pass lúc chạy) — ta chỉ ghi host/port + ghi chú, TUYỆT ĐỐI không ghi mật
//! khẩu vào file (luật #3 an toàn bí mật).

use lvc_core::proxy::ProxySpec;

/// Sinh nội dung `user.js` cho `proxy` đã phân tích:
/// - `None` (IP máy / `direct`) → `network.proxy.type = 0` (không proxy) để XOÁ cấu hình proxy cũ
///   nếu profile từng có.
/// - `socks5` → đặt `network.proxy.socks*` (+ `socks_remote_dns` để DNS đi qua proxy).
/// - `http`/`https` → đặt `network.proxy.http`/`ssl` + `share_proxy_settings` (dùng cho mọi giao
///   thức).
///
/// Proxy có xác thực: CHỈ ghi host/port + ghi chú; Firefox hỏi user/pass lúc chạy (không ghi mật
/// khẩu).
#[must_use]
pub fn user_js_for_proxy(proxy: Option<&ProxySpec>) -> String {
    const HEADER: &str = "// user.js — sinh boi LVC (engine Firefox Portable). KHONG sua tay.\n";
    let Some(spec) = proxy else {
        return format!("{HEADER}user_pref(\"network.proxy.type\", 0);\n");
    };
    let host = spec.host.as_str();
    let port = spec.port;
    let body = if spec.scheme == "socks5" {
        format!(
            "user_pref(\"network.proxy.socks\", \"{host}\");\n\
             user_pref(\"network.proxy.socks_port\", {port});\n\
             user_pref(\"network.proxy.socks_version\", 5);\n\
             user_pref(\"network.proxy.socks_remote_dns\", true);\n",
        )
    } else {
        // http/https: cùng một proxy cho HTTP lẫn HTTPS (HTTPS đi qua CONNECT).
        format!(
            "user_pref(\"network.proxy.http\", \"{host}\");\n\
             user_pref(\"network.proxy.http_port\", {port});\n\
             user_pref(\"network.proxy.ssl\", \"{host}\");\n\
             user_pref(\"network.proxy.ssl_port\", {port});\n\
             user_pref(\"network.proxy.share_proxy_settings\", true);\n",
        )
    };
    // Proxy có xác thực: KHÔNG ghi mật khẩu — Firefox sẽ hỏi user/pass khi chạy (prefs không đặt
    // được auth proxy). Chỉ ghi một dòng ghi chú.
    let auth_note = if spec.has_auth() {
        "// Proxy co xac thuc: Firefox se hoi user/pass khi chay (khong dat qua prefs).\n"
    } else {
        ""
    };
    format!("{HEADER}user_pref(\"network.proxy.type\", 1);\n{body}{auth_note}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lvc_core::proxy::parse_proxy;

    fn spec(raw: &str) -> ProxySpec {
        parse_proxy(raw).unwrap().unwrap()
    }

    #[test]
    fn khong_proxy_thi_type_0() {
        let s = user_js_for_proxy(None);
        assert!(s.contains("network.proxy.type\", 0"), "{s}");
        assert!(
            !s.contains("network.proxy.http"),
            "khong duoc dat host nao: {s}"
        );
    }

    #[test]
    fn http_dat_http_va_ssl() {
        let s = user_js_for_proxy(Some(&spec("1.2.3.4:8080")));
        assert!(s.contains("network.proxy.type\", 1"), "{s}");
        assert!(s.contains("network.proxy.http\", \"1.2.3.4\""), "{s}");
        assert!(s.contains("network.proxy.http_port\", 8080"), "{s}");
        assert!(s.contains("network.proxy.ssl\", \"1.2.3.4\""), "{s}");
        assert!(s.contains("network.proxy.ssl_port\", 8080"), "{s}");
        assert!(!s.contains("network.proxy.socks"), "{s}");
    }

    #[test]
    fn socks5_dat_socks_va_version_5() {
        let s = user_js_for_proxy(Some(&spec("socks5://host.example.com:1080")));
        assert!(
            s.contains("network.proxy.socks\", \"host.example.com\""),
            "{s}"
        );
        assert!(s.contains("network.proxy.socks_port\", 1080"), "{s}");
        assert!(s.contains("network.proxy.socks_version\", 5"), "{s}");
        assert!(s.contains("network.proxy.socks_remote_dns\", true"), "{s}");
        assert!(!s.contains("network.proxy.http\""), "{s}");
    }

    #[test]
    fn proxy_co_xac_thuc_ghi_chu_khong_lo_mat_khau() {
        let s = user_js_for_proxy(Some(&spec("1.2.3.4:8080:user:s3cr3t")));
        assert!(
            !s.contains("s3cr3t"),
            "khong duoc ghi mat khau vao user.js: {s}"
        );
        assert!(
            s.contains("Firefox se hoi user/pass"),
            "phai co ghi chu: {s}"
        );
        // Van ghi host/port de nguoi dung thay proxy nao.
        assert!(s.contains("network.proxy.http\", \"1.2.3.4\""), "{s}");
    }
}
