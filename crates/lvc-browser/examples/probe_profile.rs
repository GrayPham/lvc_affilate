//! Cua tu P1 — bien the dung PROFILE co san (khong can cookies.txt).
//! Chay: `cargo run -p lvc-browser --example probe_profile -- <profile_dir> [--headed]`
//! Bien moi truong `LVC_CHROME` = duong dan chrome.exe (mac dinh: chromiumoxide tu tim).
//! Profile do Python (patchright, Chromium 1234) tao thi PHAI mo bang dung binary do —
//! Google Chrome khong giai ma duoc cookie cua Chromium khac va nguoc lai.
//! CHI mo profile, dem cookie, goto /chat, probe, screenshot. KHONG gui prompt.

use std::path::PathBuf;

use lvc_browser::{BrowserDriver, LaunchOpts, chromium::ChromiumDriver};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let Some(profile_arg) = args.get(1) else {
        eprintln!(
            "cach dung: probe_profile <profile_dir> [--headed]   (env LVC_CHROME=chrome.exe)"
        );
        std::process::exit(2);
    };
    let profile_dir = PathBuf::from(profile_arg);
    let headed = args.iter().any(|a| a == "--headed");

    let d = ChromiumDriver;
    let t0 = std::time::Instant::now();
    let s = d
        .launch(&LaunchOpts {
            profile_dir,
            headless: !headed,
            proxy_server: None,
            window_offscreen: false,
            chrome_executable: std::env::var("LVC_CHROME").ok().map(PathBuf::from),
        })
        .await?;
    eprintln!("launch: {:?}", t0.elapsed());

    let ua: String = s.page.evaluate("navigator.userAgent").await?.into_value()?;
    let lang: String = s.page.evaluate("navigator.language").await?.into_value()?;
    let wd: serde_json::Value = s
        .page
        .evaluate("navigator.webdriver")
        .await?
        .into_value()
        .unwrap_or(serde_json::Value::Null);
    eprintln!("ua={ua}\nlanguage={lang} webdriver={wd}");

    // Chan doan: dem cookie toan browser (Storage.getCookies) TRUOC goto — tach
    // "profile khong nap cookie" khoi "server thu hoi sau khi vao /chat".
    let count = |label: &'static str, s: &lvc_browser::chromium::ChromiumSession| {
        let page = s.page.clone();
        async move {
            // Loi CDP phai hien ra rieng, khong duoc gop voi "0 cookie" that.
            let all = match page
                .execute(chromiumoxide::cdp::browser_protocol::storage::GetCookiesParams::default())
                .await
            {
                Ok(r) => r.result.cookies.clone(),
                Err(e) => {
                    eprintln!("[{label}] Storage.getCookies LOI: {e}");
                    return;
                }
            };
            let dola: Vec<&str> = all
                .iter()
                .filter(|c| c.domain.contains("dola.com"))
                .map(|c| c.name.as_str())
                .collect();
            let sid = all
                .iter()
                .any(|c| c.name == "sessionid" && !c.value.is_empty());
            eprintln!(
                "[{label}] total={} dola={} sessionid={sid} names={dola:?}",
                all.len(),
                dola.len()
            );
        }
    };
    count("truoc goto", &s).await;

    d.goto(&s, "https://www.dola.com/chat").await?;
    count("ngay sau goto", &s).await;
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    count("sau 6s", &s).await;

    let probe = d.probe_session(&s).await?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let shot = PathBuf::from(format!("probe_profile_{ts}.png"));
    d.screenshot(&s, &shot).await?;

    let markers: Vec<&String> = probe
        .texts
        .iter()
        .filter(|t| lvc_core::has_login_marker(t))
        .collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "state": probe.state.as_str(),
            "url": probe.url,
            "has_sessionid": probe.has_sessionid,
            "composer": probe.composer,
            "region_restricted": probe.url.contains("/security/region-restricted"),
            "login_markers_seen": markers,
            "texts_sample": probe.texts.iter().take(25).collect::<Vec<_>>(),
            "screenshot": shot,
        }))?
    );
    d.close(s).await?;
    Ok(())
}
