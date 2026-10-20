//! Cookie Dola từ định dạng Netscape cookies.txt (HOP-DONG v1.3, ADR-005). Thuần.
//!
//! 7 cột cách TAB: domain, `include_subdomains`, path, secure, expiry(epoch, 0=phiên), name, value.
//! `#HttpOnly_` là cookie `HttpOnly` (curl/wget), `#` khác là chú thích.
//! Extension export hay mất tab khi copy-dán → chấp nhận tách bằng ≥2 khoảng trắng.

use serde::{Deserialize, Serialize};

pub const DOLA_DOMAIN: &str = "dola.com";
/// I-2: URL dung de gioi han `probe_session` chi doc cookie cua dola.com (Network/Storage
/// `getCookies(urls=[...])`), khop Python `context.cookies("https://www.dola.com")`
/// (browser.py:346) — khong con doc "cookie cua trang/tab hien tai" (co the la trang OAuth,
/// trang loi, hoac iframe ben thu ba mang `sessionid` rieng cua no).
pub const DOLA_URL: &str = "https://www.dola.com/";
pub const REQUIRED_COOKIE: &str = "sessionid";
pub const MAX_TEXT_BYTES: usize = 200 * 1024;

/// I-2: luat thuan de xet "co sessionid Dola hop le" tu mot danh sach cookie da doc qua CDP —
/// KHONG phu thuoc chromiumoxide nen test duoc khong can Chrome. `cookies` la
/// `(domain, name, value)`; khop domain kieu "chua chuoi con" giong `validate_dola_cookies`
/// (bao gom ca dang co dau cham dau ".dola.com" lan khong).
#[must_use]
pub fn has_dola_sessionid<'a>(
    cookies: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>,
) -> bool {
    cookies.into_iter().any(|(domain, name, value)| {
        domain.to_lowercase().contains(DOLA_DOMAIN) && name == REQUIRED_COOKIE && !value.is_empty()
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// Epoch giây; `None` = cookie phiên.
    pub expires: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CookieError {
    #[error("no cookies for dola.com found (export cookies while logged in at www.dola.com)")]
    NoDolaCookies,
    #[error("missing 'sessionid' cookie for dola.com (not logged in when exported?)")]
    MissingSessionId,
}

/// Tách theo TAB; thiếu cột thì thử tách theo ≥2 khoảng trắng hoặc TAB (mất tab khi dán).
fn split_cols(line: &str) -> Vec<String> {
    let by_tab: Vec<String> = line.split('\t').map(str::to_string).collect();
    if by_tab.len() >= 7 {
        return by_tab;
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut spaces = 0usize;
    for ch in line.chars() {
        match ch {
            '\t' => {
                out.push(std::mem::take(&mut cur));
                spaces = 0;
            }
            ' ' => {
                spaces += 1;
                if spaces == 2 {
                    cur.truncate(cur.len().saturating_sub(1));
                    out.push(std::mem::take(&mut cur));
                } else if spaces == 1 {
                    cur.push(' ');
                }
            }
            _ => {
                spaces = 0;
                cur.push(ch);
            }
        }
    }
    out.push(cur);
    // KHÁC Python có chủ ý: re.split để lại token rỗng đầu dòng khi dòng #HttpOnly_
    // có khoảng trắng thừa sau tiền tố → parts[:6] lệch một cột → Python làm rơi cookie.
    // Bỏ token rỗng ở đây giữ được cookie đó. Không có file nào bị đọc chéo giữa hai
    // tool, nên rộng hơn Python là an toàn.
    out.into_iter().filter(|s| !s.is_empty()).collect()
}

#[must_use]
pub fn parse_netscape(text: &str) -> Vec<Cookie> {
    let mut out = Vec::new();
    for raw in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        let mut line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let mut http_only = false;
        if let Some(rest) = line.strip_prefix("#HttpOnly_") {
            line = rest;
            http_only = true;
        } else if line.starts_with('#') {
            continue;
        }
        let parts = split_cols(line);
        if parts.len() < 7 {
            continue;
        }
        let domain = parts[0].trim();
        let path = parts[2].trim();
        let secure = parts[3].trim().eq_ignore_ascii_case("TRUE");
        let expiry = parts[4].trim();
        let name = parts[5].trim();
        let value = parts[6..].join("\t").trim().to_string();
        if domain.is_empty() || name.is_empty() {
            continue;
        }
        // Epoch cookie thực tế < 2^53 nên f64→i64 chính xác; với chuỗi lạ ("1e300", "inf")
        // Rust bão hoà về i64::MAX thay vì lỗi như Python — chấp nhận được, cookie đó chỉ
        // có hạn xa vô lý chứ không hỏng gì.
        #[allow(clippy::cast_possible_truncation)]
        let expires = expiry
            .parse::<f64>()
            .ok()
            .map(|f| f as i64)
            .filter(|&e| e > 0);
        out.push(Cookie {
            name: name.to_string(),
            value,
            domain: domain.to_string(),
            path: if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            },
            secure,
            http_only,
            expires,
        });
    }
    fill_ss_aliases(out)
}

/// Domain mặc định (có dấu `.` đầu để khớp cả subdomain) cho cookie dạng header KHÔNG kèm domain,
/// suy theo nhà cung cấp. Rỗng = không biết (caller nên báo lỗi/yêu cầu Netscape).
#[must_use]
pub fn default_cookie_domain(provider: &str) -> &'static str {
    match provider {
        "museai" => ".muse.ai",
        "dola" => ".dola.com",
        "gemini" | "google" => ".google.com",
        "facebook" => ".facebook.com",
        "chatgpt" | "openai" => ".openai.com",
        "apple" => ".apple.com",
        _ => "",
    }
}

/// Parse cookie dạng HEADER/trình duyệt: `name=value; name2=value2; ...` (một dòng). KHÔNG có domain
/// nên gán `domain` truyền vào, `path="/"`, `secure=true`, cookie phiên (`expires=None`). Bỏ cặp
/// rỗng/thiếu `=`. KHÔNG log giá trị (bí mật).
#[must_use]
pub fn parse_cookie_header(text: &str, domain: &str) -> Vec<Cookie> {
    let out: Vec<Cookie> = text
        .split(';')
        .filter_map(|kv| {
            let kv = kv.trim();
            let (name, value) = kv.split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some(Cookie {
                name: name.to_string(),
                value: value.trim().to_string(),
                domain: domain.to_string(),
                path: "/".to_string(),
                secure: true,
                http_only: false,
                expires: None,
            })
        })
        .collect();
    fill_ss_aliases(out)
}

/// Parse cookie TỰ PHÁT HIỆN định dạng: thử Netscape (tab) trước; rỗng thì thử header
/// (`name=value; ...`) với `default_domain`. Dùng cho ô "Dán cookie" (người dùng dán kiểu nào cũng
/// nhận).
#[must_use]
pub fn parse_cookies(text: &str, default_domain: &str) -> Vec<Cookie> {
    let ns = parse_netscape(text);
    if !ns.is_empty() {
        return ns;
    }
    parse_cookie_header(text, default_domain)
}

/// Tách DÁN HÀNG LOẠT: mỗi dòng một tài khoản theo `email|...|cookies` (các trường ngăn bởi `|`).
/// Tự tìm trường cookie (trường chứa `=`, ưu tiên có cả `;`); nhãn = trường ĐẦU (email) nếu khác
/// trường cookie, không thì `accN`. Dòng không có `|` → cả dòng là cookie. Trả `(nhãn, chuỗi
/// cookie thô)` để caller tiêm qua luồng dán-một sẵn có. KHÔNG log.
#[must_use]
pub fn extract_bulk_cookie_lines(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (i, raw) in text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .enumerate()
    {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('|').map(str::trim).collect();
        let cookie_field = fields
            .iter()
            .rev()
            .find(|f| f.contains('=') && f.contains(';'))
            .or_else(|| fields.iter().rev().find(|f| f.contains('=')))
            .copied()
            .unwrap_or(line);
        if cookie_field.is_empty() || !cookie_field.contains('=') {
            continue;
        }
        let label = fields
            .first()
            .copied()
            .filter(|s| !s.is_empty() && *s != cookie_field)
            .map_or_else(|| format!("acc{}", i + 1), str::to_string);
        out.push((label, cookie_field.to_string()));
    }
    out
}

/// CK-BUG-006: `sessionid_ss` → thêm bản sao `sessionid` (httpOnly+secure) khi chưa có. Không ghi đè gốc.
#[must_use]
pub fn fill_ss_aliases(cookies: Vec<Cookie>) -> Vec<Cookie> {
    let mut have: std::collections::HashSet<(String, String)> = cookies
        .iter()
        .map(|c| (c.domain.clone(), c.name.clone()))
        .collect();
    let mut extra = Vec::new();
    for c in &cookies {
        if let Some(base) = c.name.strip_suffix("_ss") {
            if !base.is_empty() && !have.contains(&(c.domain.clone(), base.to_string())) {
                let mut alias = c.clone();
                alias.name = base.to_string();
                alias.http_only = true;
                alias.secure = true;
                have.insert((c.domain.clone(), base.to_string()));
                extra.push(alias);
            }
        }
    }
    let mut all = cookies;
    all.extend(extra);
    all
}

/// Chỉ giữ cookie `*.dola.com`; bắt buộc có `sessionid` không rỗng.
///
/// # Errors
/// `NoDolaCookies` / `MissingSessionId` — thông điệp khớp Python.
pub fn validate_dola_cookies(cookies: &[Cookie]) -> Result<Vec<Cookie>, CookieError> {
    let dola: Vec<Cookie> = cookies
        .iter()
        .filter(|c| c.domain.to_lowercase().contains(DOLA_DOMAIN))
        .cloned()
        .collect();
    if dola.is_empty() {
        return Err(CookieError::NoDolaCookies);
    }
    if !dola
        .iter()
        .any(|c| c.name == REQUIRED_COOKIE && !c.value.is_empty())
    {
        return Err(CookieError::MissingSessionId);
    }
    Ok(dola)
}

#[must_use]
pub fn sessionid_expires(cookies: &[Cookie]) -> Option<i64> {
    cookies
        .iter()
        .find(|c| c.name == REQUIRED_COOKIE && c.domain.to_lowercase().contains(DOLA_DOMAIN))
        .and_then(|c| c.expires)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_format_parse_dung() {
        let c = parse_cookie_header("wd=1366x900; dpr=2; hatch_sess=abc.def", ".muse.ai");
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].name, "wd");
        assert_eq!(c[0].value, "1366x900");
        assert_eq!(c[2].name, "hatch_sess");
        assert_eq!(c[2].value, "abc.def");
        assert_eq!(c[2].domain, ".muse.ai");
        assert!(c[2].secure);
    }

    #[test]
    fn parse_cookies_tu_phat_hien_header_khi_khong_phai_netscape() {
        // Dán kiểu header -> parse_netscape rỗng -> fallback header.
        let c = parse_cookies("datr=xxx; ps_l=0; hatch_sess=s1", ".muse.ai");
        assert_eq!(c.len(), 3);
        assert_eq!(c[2].name, "hatch_sess");
    }

    #[test]
    fn bulk_tach_dung_email_va_cookie() {
        // Định dạng người dùng: email|2fa|pass|cookies  (cookie là trường có '=' và ';').
        let txt = "kd1@merepost.com|NCF9SC|v9JX%j|wd=1366x900; dpr=2; hatch_sess=s1\n\
                   acc2@x.com|AAA|pw|datr=yyy; hatch_sess=s2\n";
        let rows = extract_bulk_cookie_lines(txt);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "kd1@merepost.com");
        assert!(rows[0].1.contains("hatch_sess=s1"));
        assert_eq!(rows[1].0, "acc2@x.com");
        assert!(rows[1].1.contains("hatch_sess=s2"));
    }

    #[test]
    fn bulk_dong_chi_co_cookie_khong_co_pipe() {
        let rows = extract_bulk_cookie_lines("wd=1; hatch_sess=s9");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, "acc1");
        assert!(rows[0].1.contains("hatch_sess=s9"));
    }

    const SAMPLE: &str = "# Netscape HTTP Cookie File\n\
        .dola.com\tTRUE\t/\tTRUE\t1800000000\tsessionid\tabc123\n\
        #HttpOnly_.dola.com\tTRUE\t/\tTRUE\t0\tuid_tt\tu1\n\
        .other.com\tTRUE\t/\tFALSE\t1800000000\tfoo\tbar\n";

    #[test]
    fn doc_7_cot_tab_bo_comment_nhan_httponly() {
        let c = parse_netscape(SAMPLE);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].name, "sessionid");
        assert_eq!(c[0].domain, ".dola.com");
        assert!(c[0].secure);
        assert!(!c[0].http_only);
        assert_eq!(c[0].expires, Some(1_800_000_000));
        assert_eq!(c[1].name, "uid_tt");
        assert!(c[1].http_only, "#HttpOnly_ prefix");
        assert_eq!(c[1].expires, None, "expiry 0 = cookie phien");
    }

    #[test]
    fn mat_tab_thi_tach_bang_nhieu_khoang_trang() {
        let txt = ".dola.com   TRUE   /   TRUE   1800000000   sessionid   xyz\n";
        let c = parse_netscape(txt);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].value, "xyz");
    }

    #[test]
    fn dong_thieu_cot_hoac_rac_bi_bo() {
        assert!(parse_netscape("a\tb\tc\n\n   \n# x\n").is_empty());
    }

    #[test]
    fn ss_alias_duoc_nhan_ban_khi_chua_co_goc() {
        // CK-BUG-006: Cookie-Editor xuat ban ByteDance chi co sessionid_ss
        let txt = ".dola.com\tTRUE\t/\tTRUE\t0\tsessionid_ss\tS1\n";
        let c = parse_netscape(txt);
        assert_eq!(c.len(), 2);
        let alias = c.iter().find(|k| k.name == "sessionid").unwrap();
        assert_eq!(alias.value, "S1");
        assert!(alias.http_only && alias.secure);
    }

    #[test]
    fn ss_alias_khong_ghi_de_goc_da_co() {
        let txt = ".dola.com\tTRUE\t/\tTRUE\t0\tsessionid\tGOC\n\
                   .dola.com\tTRUE\t/\tTRUE\t0\tsessionid_ss\tSS\n";
        let c = parse_netscape(txt);
        assert_eq!(c.len(), 2);
        assert_eq!(
            c.iter().find(|k| k.name == "sessionid").unwrap().value,
            "GOC"
        );
    }

    #[test]
    fn validate_chi_giu_dola_va_can_sessionid() {
        let c = parse_netscape(SAMPLE);
        let dola = validate_dola_cookies(&c).unwrap();
        assert_eq!(dola.len(), 2, "bo .other.com");
        let no_sid = parse_netscape(".dola.com\tTRUE\t/\tTRUE\t0\tfoo\tbar\n");
        let e = validate_dola_cookies(&no_sid).unwrap_err();
        assert_eq!(
            e.to_string(),
            "missing 'sessionid' cookie for dola.com (not logged in when exported?)"
        );
        let e = validate_dola_cookies(&[]).unwrap_err();
        assert_eq!(
            e.to_string(),
            "no cookies for dola.com found (export cookies while logged in at www.dola.com)"
        );
    }

    #[test]
    fn sessionid_expires_lay_epoch_hoac_none() {
        let c = parse_netscape(SAMPLE);
        assert_eq!(sessionid_expires(&c), Some(1_800_000_000));
        let s = parse_netscape(".dola.com\tTRUE\t/\tTRUE\t0\tsessionid\tx\n");
        assert_eq!(sessionid_expires(&s), None);
    }

    #[test]
    fn has_dola_sessionid_dung_domain_khong_rong_la_true() {
        assert!(has_dola_sessionid([
            (".dola.com", "sessionid", "abc"),
            (".dola.com", "uid_tt", "u1"),
        ]));
        assert!(
            has_dola_sessionid([("www.dola.com", "sessionid", "abc")]),
            "domain khong dau cham cung phai khop (chua chuoi con dola.com)"
        );
    }

    #[test]
    fn has_dola_sessionid_o_domain_khac_la_false() {
        assert!(!has_dola_sessionid([(
            "evil-oauth.example.com",
            "sessionid",
            "abc"
        )]));
    }

    #[test]
    fn has_dola_sessionid_value_rong_la_false() {
        assert!(!has_dola_sessionid([(".dola.com", "sessionid", "")]));
    }

    #[test]
    fn httponly_co_khoang_trang_thua_sau_tien_to_van_doc_duoc_khac_python() {
        // Python: token rỗng đầu dòng làm parts[:6] lệch → domain='' → rơi cookie.
        let txt = "#HttpOnly_  .dola.com  TRUE  /  TRUE  0  sessionid  val\n";
        let c = parse_netscape(txt);
        assert_eq!(c.len(), 1, "Rust giữ cookie mà Python làm rơi");
        assert_eq!(c[0].domain, ".dola.com");
        assert_eq!(c[0].name, "sessionid");
        assert!(c[0].http_only);
    }
}
