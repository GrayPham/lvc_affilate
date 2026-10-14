//! THI NGHIEM MOT LAN (khong phai code san xuat): nap cookie that tu `cookie_test/cookie.txt` vao
//! mot profile Chromium, mo trinh duyet THAT, dieu huong `muse.ai`, doc HTML trang + thu goi 2 API
//! bootstrap (`/api/hatch/vm/wake`, `/api/hatch/token`) qua `eval_json` (fetch trong CHINH trang,
//! khong giả mạo gì — xem `crates/providers/lvc-museai/CLAUDE.md`). Muc dich: xac nhan/sua logic
//! doan tu `auth.py` tham khao truoc khi viet code san xuat o `lvc-engine`.
//!
//! Chay: `cargo run -p lvc-browser --example museai_probe -- [--headed]`
//! Ghi KHONG BAO GIO in gia tri cookie/token day du ra stdout (luat #3 CLAUDE.md goc) — chi in
//! trang thai co/khong va do dai.

use std::path::PathBuf;
use std::sync::Arc;

use lvc_browser::chromium::ChromiumDriver;
use lvc_browser::{BrowserDriver, LaunchOpts, ops};
use lvc_core::cookie::parse_netscape;

#[tokio::main]
async fn main() {
    let headed = std::env::args().any(|a| a == "--headed");
    let cookie_path = PathBuf::from("cookie_test/cookie.txt");
    let text = std::fs::read_to_string(&cookie_path)
        .unwrap_or_else(|e| panic!("doc {} that bai: {e}", cookie_path.display()));
    let cookies = parse_netscape(&text);
    println!(
        "doc duoc {} cookie tu {}",
        cookies.len(),
        cookie_path.display()
    );

    let profile_dir = std::env::temp_dir().join("lvc-museai-probe-profile");
    let logs_dir = std::env::temp_dir().join("lvc-museai-probe-logs");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::create_dir_all(&logs_dir).unwrap();

    let driver = Arc::new(ChromiumDriver);
    let opts = LaunchOpts {
        profile_dir,
        headless: !headed,
        logs_dir,
        ..Default::default()
    };

    println!("dang mo trinh duyet...");
    let session = driver.launch(&opts).await.expect("launch that bai");
    driver
        .add_cookies(&session, &cookies)
        .await
        .expect("add_cookies that bai");
    println!("da nap cookie, dieu huong toi muse.ai...");
    driver
        .goto(&session, "https://muse.ai/")
        .await
        .expect("goto that bai");

    let page = session.page();
    let html: String = ops::eval_json(page, "document.documentElement.outerHTML")
        .await
        .expect("doc HTML that bai");
    let out_path = std::env::temp_dir().join("muse_bootstrap_page.html");
    std::fs::write(&out_path, &html).unwrap();
    println!(
        "da ghi HTML trang ({} byte) vao {}",
        html.len(),
        out_path.display()
    );

    for field in [
        "gatewayUrl",
        "vmName",
        "authToken",
        "notaryToken",
        "viewerLocale",
    ] {
        let present = html.contains(&format!("\"{field}\""));
        println!(
            "  field {field}: {}",
            if present {
                "CO trong HTML"
            } else {
                "KHONG thay"
            }
        );
    }

    // auth_check — xac nhan cookie that su dang nhap (khong lo gi, chi tra ok:true/false).
    let auth_check_js = r"
        fetch('/api/auth/check', {
            method: 'POST',
            headers: {'Accept': '*/*'},
            credentials: 'same-origin'
        }).then(r => r.text()).then(t => ({status: 0, body: t})).catch(e => ({status: -1, body: String(e)}))
    ";
    match ops::eval_json::<serde_json::Value>(page, auth_check_js).await {
        Ok(v) => println!("auth/check: {v}"),
        Err(e) => println!("auth/check LOI: {e}"),
    }

    // Trich xuat vmName/gatewayUrl tho (quick-and-dirty, chi de kiem chung, khong phai parser cuoi
    // cung) — giai nen \" -> " giong `_find_string` ben Python truoc khi regex.
    let unescaped = html.replace("\\\"", "\"").replace("\\/", "/");
    let vm_name = extract_field(&unescaped, "vmName");
    println!("vmName trich duoc: {vm_name:?}");

    if let Some(vm_name) = &vm_name {
        let wake_js = format!(
            r"
            fetch('/api/hatch/vm/wake', {{
                method: 'POST',
                headers: {{'Accept': '*/*', 'Content-Type': 'application/json', 'Origin': location.origin, 'Referer': location.href}},
                credentials: 'same-origin',
                body: JSON.stringify({{vm_id: {vm_name:?}, retry_count: 0, connect_attempt_id: crypto.randomUUID()}})
            }}).then(async r => ({{status: r.status, body: await r.text()}})).catch(e => ({{status: -1, body: String(e)}}))
        "
        );
        match ops::eval_json::<serde_json::Value>(page, &wake_js).await {
            Ok(v) => println!("hatch/vm/wake: {v}"),
            Err(e) => println!("hatch/vm/wake LOI: {e}"),
        }

        let gateway_url = extract_field(&unescaped, "gatewayUrl");
        let token_js = format!(
            r"
            fetch('/api/hatch/token', {{
                method: 'POST',
                headers: {{'Accept': '*/*', 'Content-Type': 'application/json', 'Origin': location.origin, 'Referer': location.href}},
                credentials: 'same-origin',
                body: JSON.stringify({{vmAddress: {gw:?}, vmName: {vm_name:?}}})
            }}).then(async r => ({{status: r.status, body: await r.text()}})).catch(e => ({{status: -1, body: String(e)}}))
        ",
            gw = gateway_url.unwrap_or_default()
        );
        match ops::eval_json::<serde_json::Value>(page, &token_js).await {
            Ok(v) => {
                let redacted = redact_tokens(&v.to_string());
                println!("hatch/token (da che token dai): {redacted}");
            }
            Err(e) => println!("hatch/token LOI: {e}"),
        }
    }

    driver.close(session).await.ok();
    println!("xong.");
}

fn extract_field(text: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\":\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn redact_tokens(s: &str) -> String {
    use std::fmt::Write;
    // Che moi chuoi lien tuc >=20 ky tu "token-giong" (chu/so/_/-) de khong lo gia tri nhay cam ra
    // terminal/chat — chi giu do dai + vai ky tu dau de con biet dinh dang.
    let mut out = String::new();
    let mut buf = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            buf.push(ch);
        } else {
            if buf.len() >= 20 {
                out.push_str(&buf[..6.min(buf.len())]);
                let _ = write!(out, "<...{}byte...>", buf.len());
            } else {
                out.push_str(&buf);
            }
            buf.clear();
            out.push(ch);
        }
    }
    if buf.len() >= 20 {
        out.push_str(&buf[..6.min(buf.len())]);
        let _ = write!(out, "<...{}byte...>", buf.len());
    } else {
        out.push_str(&buf);
    }
    out
}
