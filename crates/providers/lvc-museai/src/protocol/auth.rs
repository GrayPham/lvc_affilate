//! "Vé vào cửa" cổng Hatch — port CHỈ phần struct dữ liệu từ `MuseBootstrap` (`auth.py`, tham
//! khảo, CHỈ ĐỌC). THUẦN, không gọi mạng — khác bản Python (`MuseAuth` còn login/bootstrap qua
//! `curl_cffi`, CỐ Ý không port, xem `CLAUDE.md` "bối cảnh rủi ro"). Giá trị các trường lấy từ
//! đâu là việc của `lvc-engine` (trình duyệt thật đã đăng nhập, qua `eval_json`), không phải ở
//! đây — `MuseBootstrap` chỉ là nơi giữ + tính `websocket_url()` từ giá trị đã có sẵn.

/// 5 giá trị cần để mở WebSocket Hatch — `notary` thường có nhưng không bắt buộc (một số VM
/// không trả về, khớp `str | None = None` bên Python).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuseBootstrap {
    pub gateway_url: String,
    pub vm_name: String,
    pub admission: String,
    pub notary: Option<String>,
    pub locale: String,
}

impl MuseBootstrap {
    /// `vm_name` nếu khác rỗng; nếu không, phần đầu hostname của `gateway_url` (trước dấu `.`
    /// đầu tiên) — khớp `MuseBootstrap.vm_id` bên Python.
    #[must_use]
    pub fn vm_id(&self) -> String {
        if !self.vm_name.is_empty() {
            return self.vm_name.clone();
        }
        hostname_of(&self.gateway_url)
            .and_then(|h| h.split('.').next().map(str::to_string))
            .unwrap_or_default()
    }

    /// URL WebSocket đầy đủ (kèm query `vm_id`/`auth_token`/`app_id`/`request_id`[/`notary_token`])
    /// sẵn sàng truyền cho [`crate::protocol::transport::HatchConnection::connect`].
    #[must_use]
    pub fn websocket_url(&self) -> String {
        let mut params = vec![
            ("vm_id".to_string(), self.vm_id()),
            ("auth_token".to_string(), self.admission.clone()),
            ("app_id".to_string(), "hatch-web".to_string()),
            ("request_id".to_string(), uuid::Uuid::new_v4().to_string()),
        ];
        if let Some(notary) = &self.notary {
            params.push(("notary_token".to_string(), notary.clone()));
        }
        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", url_encode_query(k), url_encode_query(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("wss://hatch.metaaivm.com/v1/noise?{query}")
    }
}

/// Giải nén `gatewayUrl`/`vmName`/`authToken`/`notaryToken`/`viewerLocale` từ HTML trang chủ
/// `muse.ai` — port `_find_string`/`bootstrap_page` (`auth.py`, tham khảo). ĐÃ XÁC NHẬN qua trang
/// thật 2026-10-11 (probe `crates/lvc-browser/examples/museai_probe.rs`, cookie thật của người
/// dùng): dữ liệu bootstrap nhúng trong trang dưới dạng JSON — một khối KHÔNG escape (chứa
/// `gatewayUrl`/`vmName`/`authToken`) và một khối `initialServerData` CÓ escape (`\"`, chứa
/// `viewerLocale`) riêng biệt. Giải nén escape ĐỀU trên cả trang trước khi tìm field (khớp cách
/// Python `_find_string` tự `html.unescape` + bỏ `\"`/`\/` trước khi regex) nên khớp CẢ HAI khối.
/// `notaryToken` không phải lúc nào cũng có trong trang (xác nhận: trang thật của tài khoản test
/// KHÔNG có field này — vẫn lấy được qua [phản hồi `/api/hatch/token`], xem `node-museai-video/
/// CLAUDE.md`), nên trả `None` thay vì lỗi khi thiếu.
///
/// # Errors
/// Chuỗi mô tả khi thiếu `gatewayUrl`/`authToken` (coi là bắt buộc, khớp Python `required=True`).
pub fn parse_bootstrap_html(html: &str) -> std::result::Result<MuseBootstrap, String> {
    let unescaped = html.replace("\\\"", "\"").replace("\\/", "/");
    let gateway_url = find_string_field(&unescaped, "gatewayUrl")
        .ok_or_else(|| "khong tim thay truong gatewayUrl trong trang muse.ai".to_string())?;
    let vm_name = find_string_field(&unescaped, "vmName").unwrap_or_default();
    let admission = find_string_field(&unescaped, "authToken")
        .ok_or_else(|| "khong tim thay truong authToken trong trang muse.ai".to_string())?;
    let notary = find_string_field(&unescaped, "notaryToken");
    let locale = find_string_field(&unescaped, "viewerLocale")
        .unwrap_or_else(|| "en-US".to_string())
        .replace('_', "-");
    Ok(MuseBootstrap {
        gateway_url,
        vm_name,
        admission,
        notary,
        locale,
    })
}

/// Tìm `"field":"value"` (sau khi đã giải nén escape) — dừng ở dấu `"` ĐẦU TIÊN sau giá trị. Đơn
/// giản hơn regex lồng-escape của Python vì các trường ở đây (URL/UUID/token) không chứa ký tự
/// `"` thật trong giá trị — đủ dùng, xác nhận qua trang thật.
fn find_string_field(text: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let start = text.find(&needle)? + needle.len();
    let end = text[start..].find('"')?;
    Some(text[start..start + end].to_string())
}

/// Lấy hostname từ một URL `scheme://[user:pass@]host[:port][/path...]` — chỉ cần đủ dùng cho
/// `gateway_url` (luôn là URL hợp lệ do chính muse.ai trả về), không phải parser URL tổng quát.
fn hostname_of(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    let after_userinfo = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = after_userinfo.split(':').next().unwrap_or(after_userinfo);
    (!host.is_empty()).then(|| host.to_string())
}

/// `application/x-www-form-urlencoded` tối giản (khớp `urllib.parse.urlencode` bên Python: giữ
/// ký tự "an toàn", escape phần còn lại, dấu cách thành `+`).
fn url_encode_query(value: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bootstrap(vm_name: &str, notary: Option<&str>) -> MuseBootstrap {
        MuseBootstrap {
            gateway_url: "https://vm-abc123.hatch.metaaivm.com/".to_string(),
            vm_name: vm_name.to_string(),
            admission: "tok-1".to_string(),
            notary: notary.map(str::to_string),
            locale: "vi-VN".to_string(),
        }
    }

    #[test]
    fn vm_id_dung_vm_name_khi_co() {
        assert_eq!(bootstrap("my-vm", None).vm_id(), "my-vm");
    }

    #[test]
    fn vm_id_suy_tu_gateway_url_khi_vm_name_rong() {
        assert_eq!(bootstrap("", None).vm_id(), "vm-abc123");
    }

    #[test]
    fn websocket_url_co_du_truong_bat_buoc() {
        let url = bootstrap("my-vm", None).websocket_url();
        assert!(url.starts_with("wss://hatch.metaaivm.com/v1/noise?"));
        assert!(url.contains("vm_id=my-vm"));
        assert!(url.contains("auth_token=tok-1"));
        assert!(url.contains("app_id=hatch-web"));
        assert!(url.contains("request_id="));
        assert!(!url.contains("notary_token"));
    }

    #[test]
    fn websocket_url_them_notary_token_khi_co() {
        let url = bootstrap("my-vm", Some("n-1")).websocket_url();
        assert!(url.contains("notary_token=n-1"));
    }

    #[test]
    fn hostname_of_bo_qua_scheme_userinfo_port_path() {
        assert_eq!(
            hostname_of("https://user:pass@vm-x.example.com:443/path?q=1"),
            Some("vm-x.example.com".to_string())
        );
    }

    /// Mô phỏng ĐÚNG cấu trúc trang thật đã xác nhận (2026-10-11, xem doc
    /// `parse_bootstrap_html`): một khối KHÔNG escape (`gatewayUrl`/`vmName`/`authToken`) + một
    /// khối `initialServerData` CÓ escape (`viewerLocale`) riêng biệt trong cùng trang. Giá trị ở
    /// đây đều GIẢ (không phải dữ liệu thật của người dùng).
    fn fake_page_html() -> String {
        let mut html = String::new();
        html.push_str(r#"<script>window.__HATCH__={"version":1,"gatewayUrl":"wss://vm-fake123.hatch.metaaivm.com/","vmName":"vm-fake123","authToken":"s0:fake-admission-token"};</script>"#);
        html.push_str(r#"<script>window.__DATA__=JSON.parse("{\"optionalFlow\":null},\"initialServerData\":{\"viewerLocale\":\"ja_JP\",\"gatewayUrl\":\"wss://vm-fake123.hatch.metaaivm.com/\"}");</script>"#);
        html
    }

    #[test]
    fn parse_bootstrap_html_lay_dung_ca_khoi_escape_lan_khong_escape() {
        let b = parse_bootstrap_html(&fake_page_html()).unwrap();
        assert_eq!(b.gateway_url, "wss://vm-fake123.hatch.metaaivm.com/");
        assert_eq!(b.vm_name, "vm-fake123");
        assert_eq!(b.admission, "s0:fake-admission-token");
        assert_eq!(
            b.locale, "ja-JP",
            "gach duoi doi thanh gach ngang khop Python"
        );
    }

    #[test]
    fn parse_bootstrap_html_notary_vang_mat_tra_none_khong_loi() {
        // Trang thuc te cua tai khoan test KHONG co notaryToken nhung van hop le (lay qua
        // /api/hatch/token sau, khong phai o day).
        let b = parse_bootstrap_html(&fake_page_html()).unwrap();
        assert_eq!(b.notary, None);
    }

    #[test]
    fn parse_bootstrap_html_locale_vang_mat_mac_dinh_en_us() {
        let html = r#"{"gatewayUrl":"wss://x/","vmName":"v","authToken":"t"}"#;
        let b = parse_bootstrap_html(html).unwrap();
        assert_eq!(b.locale, "en-US");
    }

    #[test]
    fn parse_bootstrap_html_thieu_gateway_url_bao_loi_ro_rang() {
        let html = r#"{"vmName":"v","authToken":"t"}"#;
        let err = parse_bootstrap_html(html).unwrap_err();
        assert!(err.contains("gatewayUrl"));
    }

    #[test]
    fn parse_bootstrap_html_thieu_auth_token_bao_loi_ro_rang() {
        let html = r#"{"gatewayUrl":"wss://x/","vmName":"v"}"#;
        let err = parse_bootstrap_html(html).unwrap_err();
        assert!(err.contains("authToken"));
    }
}
