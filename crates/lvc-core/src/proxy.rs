//! Proxy riêng từng tài khoản (HOP-DONG-nhom1 v1, REQ-001) — thuần, không I/O.
//! Nguồn: `src/utils/proxies.py`.
//!
//! Cú pháp người dùng nhập được chấp nhận ở 3 dạng (proxies.py:3-8):
//!   "ip:port"                      → không xác thực
//!   "ip:port:user:pass"            → xác thực user/pass
//!   "scheme://user:pass@host:port" → scheme ∈ http, https, socks5 (user/pass tuỳ chọn)
//! Giá trị đặc biệt: "" → không đặt; "direct" → không dùng proxy nào.

use std::fmt;

use crate::CoreError;
use crate::video_url::percent_decode;

/// proxies.py:21 — giá trị đặc biệt "không dùng proxy nào".
pub const DIRECT: &str = "direct";

/// proxies.py:22 `_SCHEMES`.
const SCHEMES: [&str; 3] = ["http", "https", "socks5"];

/// proxies.py:24 `_ERR` — thông điệp lỗi CỐ ĐỊNH cho MỌI định dạng sai (copy nguyên văn, không
/// chèn giá trị đầu vào — khác các `CoreError::Invalid*` khác dùng `py_repr`).
const ERR: &str = "proxy must be ip:port[:user:pass] or scheme://user:pass@host:port (scheme http/https/socks5), or 'direct'";

fn err() -> CoreError {
    CoreError::InvalidProxy(ERR.to_string())
}

/// Proxy đã phân tích, sẵn sàng đưa vào `--proxy-server` / `proxy_kwargs` (patchright).
#[derive(Clone, PartialEq, Eq)]
pub struct ProxySpec {
    /// `http` | `https` | `socks5`. Dạng `ip:port`/`ip:port:user:pass` luôn ngầm định `http`
    /// (proxies.py:70 `f"http://{host}:{int(port)}"`).
    pub scheme: String,
    /// Không kèm ngoặc vuông kể cả `IPv6` — `server()` tự bọc khi cần (proxies.py:50-51).
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxySpec {
    /// `"scheme://host:port"` — patchright/Chrome nhận nguyên chuỗi này làm `proxy.server`
    /// (proxies.py: trường `"server"` của dict trả về).
    #[must_use]
    pub fn server(&self) -> String {
        if self.host.contains(':') {
            format!("{}://[{}]:{}", self.scheme, self.host, self.port)
        } else {
            format!("{}://{}:{}", self.scheme, self.host, self.port)
        }
    }

    #[must_use]
    pub fn has_auth(&self) -> bool {
        self.username.is_some()
    }
}

/// Tự viết `Debug`: che mật khẩu bằng `"***"` — không được để lộ ra log/panic message.
impl fmt::Debug for ProxySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProxySpec")
            .field("scheme", &self.scheme)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .finish()
    }
}

/// proxies.py:23 `_HOST_RE = r"^[A-Za-z0-9.\-_\[\]:]+$"`.
fn host_char_ok(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '[' | ']' | ':')
}

fn host_ok(host: &str) -> bool {
    !host.is_empty() && host.chars().all(host_char_ok)
}

/// proxies.py:27-28 `_port_ok`: CHỈ chữ số (không nhận `+`/`-` như `u16::from_str`), 1..=65535.
fn port_ok_str(port: &str) -> bool {
    !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
        && port.parse::<u32>().is_ok_and(|p| (1..=65535).contains(&p))
}

/// Cắt phần "authority" (`[user[:pass]@]host[:port]`) khỏi path/query/fragment phía sau —
/// tương ứng `urlsplit(...).netloc` (proxies.py:41-49).
fn split_authority(rest: &str) -> (&str, &str) {
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    (&rest[..end], &rest[end..])
}

/// Tương ứng `urllib.parse.SplitResult._userinfo`/`_hostinfo`: tách `netloc` thành
/// (username, password, host, `port_thô`). Percent-decode username/password (proxies.py:52-53).
fn split_netloc(netloc: &str) -> (Option<String>, Option<String>, String, Option<String>) {
    let (userinfo, hostinfo) = match netloc.rfind('@') {
        Some(idx) => (Some(&netloc[..idx]), &netloc[idx + 1..]),
        None => (None, netloc),
    };
    let (username, password) = match userinfo {
        None => (None, None),
        Some(info) => {
            let (u, p) = match info.find(':') {
                Some(idx) => (&info[..idx], Some(&info[idx + 1..])),
                None => (info, None),
            };
            let username = if u.is_empty() {
                None
            } else {
                Some(percent_decode(u))
            };
            let password = match p {
                Some(p) if !p.is_empty() => Some(percent_decode(p)),
                _ => None,
            };
            (username, password)
        }
    };
    let (host, port) = if let Some(rest) = hostinfo.strip_prefix('[') {
        // IPv6: [::1]:1080 — hostname là phần TRONG ngoặc, port sau ']:' nếu có.
        match rest.find(']') {
            Some(idx) => {
                let host = &rest[..idx];
                let port = rest[idx + 1..]
                    .strip_prefix(':')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string);
                (host.to_string(), port)
            }
            None => (hostinfo.to_string(), None),
        }
    } else {
        match hostinfo.find(':') {
            Some(idx) => {
                let port = &hostinfo[idx + 1..];
                let port = if port.is_empty() {
                    None
                } else {
                    Some(port.to_string())
                };
                (hostinfo[..idx].to_string(), port)
            }
            None => (hostinfo.to_string(), None),
        }
    };
    // proxies.py: `parts.hostname` LOWERCASE hoá host (khác dạng ip:port giữ nguyên hoa/thường).
    (username, password, host.to_lowercase(), port)
}

/// Dạng `scheme://user:pass@host:port` (proxies.py:40-56).
fn parse_url_form(raw: &str) -> Result<ProxySpec, CoreError> {
    let idx = raw.find("://").ok_or_else(err)?;
    let scheme = raw[..idx].to_lowercase();
    if !SCHEMES.contains(&scheme.as_str()) {
        return Err(err());
    }
    let rest = &raw[idx + 3..];
    let (authority, tail) = split_authority(rest);
    // proxies.py:45 path phải rỗng hoặc "/", không query/fragment — cắt gộp path+query+fragment
    // vào `tail` nên chỉ cần so đúng hai giá trị hợp lệ.
    if !(tail.is_empty() || tail == "/") {
        return Err(err());
    }
    let (username, password, host, port) = split_netloc(authority);
    if host.is_empty() {
        return Err(err());
    }
    let Some(port_str) = port else {
        return Err(err());
    };
    if !port_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(err());
    }
    let port_num: u32 = port_str.parse().map_err(|_| err())?;
    if port_num > 65535 {
        return Err(err());
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "vua kiem port_num <= 65535 ngay o tren nen ep u16 an toan"
    )]
    let port = port_num as u16;
    if !host_ok(&host) {
        return Err(err());
    }
    // proxies.py:54-55 — username/password phải CÙNG có hoặc CÙNG không.
    if username.is_none() != password.is_none() {
        return Err(err());
    }
    Ok(ProxySpec {
        scheme,
        host,
        port,
        username,
        password,
    })
}

/// Dạng `ip:port` hoặc `ip:port:user:pass` (proxies.py:57-70).
fn parse_plain_form(raw: &str) -> Result<ProxySpec, CoreError> {
    let pieces: Vec<&str> = raw.split(':').collect();
    let (host, port, username, password) = match pieces.as_slice() {
        [host, port] => (*host, *port, None, None),
        [host, port, user, pass] => {
            // proxies.py:63-64: `if not username or not password: raise` — rỗng cũng là lỗi.
            if user.is_empty() || pass.is_empty() {
                return Err(err());
            }
            (
                *host,
                *port,
                Some((*user).to_string()),
                Some((*pass).to_string()),
            )
        }
        _ => return Err(err()),
    };
    let host = host.trim();
    let port = port.trim();
    if host.is_empty() || !host_ok(host) || !port_ok_str(port) {
        return Err(err());
    }
    // port_ok_str da kiem 1..=65535 nen parse<u16> luon thanh cong.
    let port_num: u16 = port.parse().map_err(|_| err())?;
    Ok(ProxySpec {
        scheme: "http".to_string(),
        host: host.to_string(),
        port: port_num,
        username,
        password,
    })
}

/// proxies.py:31-70 `parse_proxy` — chuỗi → `ProxySpec`; `""`/`"direct"` (không phân biệt hoa
/// thường) → `None`; sai định dạng → `Err`.
///
/// # Errors
/// `CoreError::InvalidProxy` khi chuỗi không khớp 1 trong 3 dạng cho phép.
pub fn parse_proxy(text: &str) -> Result<Option<ProxySpec>, CoreError> {
    let raw = text.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case(DIRECT) {
        return Ok(None);
    }
    if raw.chars().any(char::is_whitespace) {
        return Err(err());
    }
    if raw.contains("://") {
        parse_url_form(raw).map(Some)
    } else {
        parse_plain_form(raw).map(Some)
    }
}

/// proxies.py:73-80 `effective_proxy` gộp với `parse_proxy`: proxy riêng tài khoản (`account_proxy`)
/// thắng proxy chung (`global`); rỗng → dùng chung; `"direct"` → chặn hẳn proxy chung, không dùng
/// proxy nào.
///
/// # Errors
/// `CoreError::InvalidProxy` khi chuỗi được chọn (riêng hoặc chung) sai định dạng.
pub fn effective_proxy(account_proxy: &str, global: &str) -> Result<Option<ProxySpec>, CoreError> {
    let own = account_proxy.trim();
    if own.is_empty() {
        return parse_proxy(global.trim());
    }
    if own.eq_ignore_ascii_case(DIRECT) {
        return Ok(None);
    }
    parse_proxy(own)
}

/// proxies.py:113-133 `mask_proxy` (Task 10, tab Tài khoản — KHÔNG BAO GIỜ hiện mật khẩu/username
/// đầy đủ ra UI/log): rỗng → `""`; `"direct"` (không phân biệt hoa thường) → `"direct"` (khác rỗng —
/// cho biết người dùng CHỦ Ý chặn proxy chung, không phải "chưa đặt gì"); phân tích được → che username
/// hoặc mật khẩu (không bao giờ lộ password), giữ nguyên dạng gốc (`scheme://…` hay `host:port…`);
/// phân tích lỗi (chuỗi rác người dùng gõ nhầm) → giữ 3 mảnh đầu tách bằng `:`, phần còn lại (nếu có)
/// gộp thành `"***"` — không panic, không lộ nguyên văn chuỗi có thể chứa mật khẩu ở mảnh thứ 4+.
#[must_use]
pub fn mask_proxy(text: &str) -> String {
    let raw = text.trim();
    if raw.is_empty() {
        return String::new();
    }
    if raw.eq_ignore_ascii_case(DIRECT) {
        return DIRECT.to_string();
    }
    match parse_proxy(raw) {
        // rong/direct da xu ly o tren; nhanh nay phong thu, thuc te khong toi day.
        Ok(None) => String::new(),
        Ok(Some(spec)) => mask_valid(raw, &spec),
        Err(_) => mask_invalid(raw),
    }
}

/// `raw` đã `parse_proxy` được thành `spec` — che theo dạng gốc (URL hay `ip:port[:user:pass]`).
fn mask_valid(raw: &str, spec: &ProxySpec) -> String {
    let is_url_form = raw.contains("://");
    if let Some(user) = &spec.username {
        if is_url_form {
            // spec.server() = "scheme://host:port" (bọc ngoặc vuông cho IPv6 sẵn) — chèn
            // "user:***@" ngay sau "scheme://" thay vì tự ghép host/port (tránh lặp logic IPv6).
            let server = spec.server();
            let after_scheme = server.find("://").map_or(0, |i| i + 3);
            format!(
                "{}{user}:***@{}",
                &server[..after_scheme],
                &server[after_scheme..]
            )
        } else {
            format!("{}:{}:{user}:***", spec.host, spec.port)
        }
    } else if !is_url_form && spec.scheme == "http" {
        format!("{}:{}", spec.host, spec.port)
    } else {
        spec.server()
    }
}

/// `raw` KHÔNG `parse_proxy` được (định dạng rác) — vẫn không được lộ nguyên văn (có thể chứa mật
/// khẩu ở mảnh thứ 4 trở đi của dạng `ip:port:user:pass` gõ sai): giữ 3 mảnh đầu, phần sau `:` thứ
/// ba (nếu có) gộp thành `"***"`; ≤ 3 mảnh (không có gì để che) → giữ nguyên.
fn mask_invalid(raw: &str) -> String {
    let parts: Vec<&str> = raw.splitn(4, ':').collect();
    if parts.len() <= 3 {
        return raw.to_string();
    }
    format!("{}:{}:{}:***", parts[0], parts[1], parts[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rong_hoac_direct_thi_khong_proxy() {
        assert_eq!(parse_proxy("").unwrap(), None);
        assert_eq!(parse_proxy("   ").unwrap(), None);
        assert_eq!(parse_proxy("direct").unwrap(), None);
        assert_eq!(parse_proxy("DIRECT").unwrap(), None);
        assert_eq!(parse_proxy("  Direct  ").unwrap(), None);
    }

    #[test]
    fn dang_ip_port_khong_xac_thuc() {
        let spec = parse_proxy("1.2.3.4:8080").unwrap().unwrap();
        assert_eq!(spec.server(), "http://1.2.3.4:8080");
        assert_eq!(spec.username, None);
        assert_eq!(spec.password, None);
        assert!(!spec.has_auth());
    }

    #[test]
    fn dang_ip_port_user_pass() {
        let spec = parse_proxy("1.2.3.4:8080:user:pass").unwrap().unwrap();
        assert_eq!(spec.server(), "http://1.2.3.4:8080");
        assert_eq!(spec.username.as_deref(), Some("user"));
        assert_eq!(spec.password.as_deref(), Some("pass"));
        assert!(spec.has_auth());
    }

    #[test]
    fn dang_ip_port_thieu_hoac_thua_manh_la_loi() {
        assert!(parse_proxy("1.2.3.4:8080:user").is_err()); // 3 manh
        assert!(parse_proxy("1.2.3.4:8080:user:").is_err()); // pass rong
        assert!(parse_proxy("1.2.3.4:8080::pass").is_err()); // user rong
        assert!(parse_proxy("1.2.3.4").is_err()); // 1 manh
        assert!(parse_proxy("1.2.3.4:8080:8081:8082:8083").is_err()); // 5 manh
    }

    #[test]
    fn dang_scheme_url_day_du() {
        let spec = parse_proxy("socks5://u:p@host.example.com:1080")
            .unwrap()
            .unwrap();
        assert_eq!(spec.server(), "socks5://host.example.com:1080");
        assert_eq!(spec.username.as_deref(), Some("u"));
        assert_eq!(spec.password.as_deref(), Some("p"));
    }

    #[test]
    fn dang_scheme_url_khong_xac_thuc() {
        let spec = parse_proxy("http://host:1080").unwrap().unwrap();
        assert_eq!(spec.server(), "http://host:1080");
        assert_eq!(spec.username, None);
        assert_eq!(spec.password, None);
    }

    #[test]
    fn dang_scheme_url_giai_ma_percent_user_pass() {
        // oracle: parse_proxy("http://u%40x:p%3Aa@host:1080") -> username "u@x", password "p:a"
        let spec = parse_proxy("http://u%40x:p%3Aa@host:1080")
            .unwrap()
            .unwrap();
        assert_eq!(spec.username.as_deref(), Some("u@x"));
        assert_eq!(spec.password.as_deref(), Some("p:a"));
    }

    #[test]
    fn dang_scheme_url_ipv6_giu_ngoac_vuong_o_server() {
        let spec = parse_proxy("http://[::1]:1080").unwrap().unwrap();
        assert_eq!(spec.host, "::1");
        assert_eq!(spec.server(), "http://[::1]:1080");
    }

    #[test]
    fn dang_scheme_url_thieu_mot_trong_hai_user_pass_la_loi() {
        assert!(parse_proxy("http://u@host:1080").is_err());
        assert!(parse_proxy("http://:p@host:1080").is_err());
    }

    #[test]
    fn dang_scheme_url_co_path_query_fragment_la_loi() {
        assert!(parse_proxy("http://host:1080/path").is_err());
        assert!(parse_proxy("http://host:1080?x=1").is_err());
        assert!(parse_proxy("http://host:1080#frag").is_err());
    }

    #[test]
    fn scheme_khong_hop_le_la_loi() {
        assert!(parse_proxy("ftp://host:1080").is_err());
    }

    #[test]
    fn chuoi_bay_ba_hoac_co_khoang_trang_la_loi() {
        assert!(parse_proxy("abc").is_err());
        assert!(parse_proxy("1.2.3.4:abc").is_err());
        assert!(parse_proxy("1.2.3.4:99999").is_err());
        assert!(parse_proxy("http:// host:1080").is_err());
    }

    #[test]
    fn thong_diep_loi_khop_ban_python() {
        let e = parse_proxy("abc").unwrap_err();
        assert_eq!(
            e.to_string(),
            "proxy must be ip:port[:user:pass] or scheme://user:pass@host:port \
             (scheme http/https/socks5), or 'direct'"
        );
    }

    #[test]
    fn debug_che_mat_khau() {
        let spec = parse_proxy("1.2.3.4:8080:user:s3cr3t").unwrap().unwrap();
        let dbg = format!("{spec:?}");
        assert!(!dbg.contains("s3cr3t"), "khong duoc lo mat khau: {dbg}");
        assert!(dbg.contains("\"***\""), "phai co the cho mat khau: {dbg}");
        assert!(dbg.contains("user"));

        let no_auth = parse_proxy("1.2.3.4:8080").unwrap().unwrap();
        let dbg2 = format!("{no_auth:?}");
        assert!(dbg2.contains("password: None"));
    }

    #[test]
    fn effective_proxy_rong_thi_dung_global() {
        let spec = effective_proxy("", "1.2.3.4:8080").unwrap().unwrap();
        assert_eq!(spec.server(), "http://1.2.3.4:8080");
    }

    #[test]
    fn effective_proxy_own_thang_global() {
        let spec = effective_proxy("9.9.9.9:9090", "1.2.3.4:8080")
            .unwrap()
            .unwrap();
        assert_eq!(spec.server(), "http://9.9.9.9:9090");
    }

    #[test]
    fn effective_proxy_direct_chan_global() {
        assert_eq!(effective_proxy("direct", "1.2.3.4:8080").unwrap(), None);
    }

    #[test]
    fn effective_proxy_ca_hai_rong_thi_khong_proxy() {
        assert_eq!(effective_proxy("  ", "  ").unwrap(), None);
    }

    // ---- mask_proxy (Task 10, proxies.py:113-133) ----

    #[test]
    fn mask_proxy_rong_khac_direct() {
        assert_eq!(mask_proxy(""), "");
        assert_eq!(mask_proxy("   "), "");
    }

    #[test]
    fn mask_proxy_direct_giu_nguyen_khong_phan_biet_hoa_thuong() {
        assert_eq!(mask_proxy("direct"), "direct");
        assert_eq!(mask_proxy("DIRECT"), "direct");
        assert_eq!(mask_proxy("  Direct  "), "direct");
    }

    #[test]
    fn mask_proxy_ip_port_khong_xac_thuc_giu_nguyen() {
        assert_eq!(mask_proxy("1.2.3.4:8080"), "1.2.3.4:8080");
    }

    #[test]
    fn mask_proxy_ip_port_user_pass_che_pass_giu_dang_goc() {
        assert_eq!(
            mask_proxy("1.2.3.4:8080:user:s3cr3t"),
            "1.2.3.4:8080:user:***"
        );
    }

    #[test]
    fn mask_proxy_url_khong_xac_thuc_giu_nguyen_dang_url() {
        assert_eq!(mask_proxy("http://host:1080"), "http://host:1080");
    }

    #[test]
    fn mask_proxy_url_co_xac_thuc_che_pass_giu_scheme_va_user() {
        assert_eq!(
            mask_proxy("socks5://u:p@host.example.com:1080"),
            "socks5://u:***@host.example.com:1080"
        );
    }

    #[test]
    fn mask_proxy_url_ipv6_giu_ngoac_vuong() {
        assert_eq!(
            mask_proxy("socks5://u:p@[::1]:1080"),
            "socks5://u:***@[::1]:1080"
        );
        assert_eq!(mask_proxy("http://[::1]:1080"), "http://[::1]:1080");
    }

    #[test]
    fn mask_proxy_dang_sai_giu_3_manh_dau_che_phan_con_lai() {
        assert_eq!(mask_proxy("abc:def:ghi:jkl:mno"), "abc:def:ghi:***");
        assert_eq!(
            mask_proxy("1.2.3.4:99999:user:pass"),
            "1.2.3.4:99999:user:***"
        );
    }

    #[test]
    fn mask_proxy_dang_sai_it_hon_4_manh_giu_nguyen() {
        assert_eq!(mask_proxy("abc"), "abc");
        assert_eq!(mask_proxy("abc:def"), "abc:def");
        assert_eq!(mask_proxy("abc:def:ghi"), "abc:def:ghi");
    }
}
