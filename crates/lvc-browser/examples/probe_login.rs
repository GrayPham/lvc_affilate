//! Thi nghiem cua tu P1: chromiumoxide co mo duoc phien Dola khong?
//! Chay: `cargo run -p lvc-browser --example probe_login -- profile_dir cookies.txt [--headed] [--proxy scheme://host:port]`
//! CHI DOC `cookies.txt`. Ghi: `profile_dir/`, `./probe_<ts>.png`, stdout JSON.

use std::path::PathBuf;

use lvc_browser::{BrowserDriver, LaunchOpts, chromium::ChromiumDriver};
use lvc_core::{parse_netscape, validate_dola_cookies};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let profile_dir = PathBuf::from(&args[1]);
    let cookies_txt = tokio::fs::read_to_string(&args[2]).await?;
    let headed = args.iter().any(|a| a == "--headed");
    let proxy = args
        .iter()
        .position(|a| a == "--proxy")
        .and_then(|i| args.get(i + 1).cloned());

    let cookies = validate_dola_cookies(&parse_netscape(&cookies_txt))?;
    eprintln!("cookies dola: {} (co sessionid)", cookies.len());

    let d = ChromiumDriver;
    let s = d
        .launch(&LaunchOpts {
            profile_dir,
            headless: !headed,
            proxy_server: proxy,
            window_offscreen: false,
            chrome_executable: None,
        })
        .await?;

    // Cookie can domain hop le -> goto dola truoc (se ve trang khach), set cookie, roi reload
    d.goto(&s, "https://www.dola.com/").await?;
    d.add_cookies(&s, &cookies).await?;
    d.goto(&s, "https://www.dola.com/chat").await?;
    tokio::time::sleep(std::time::Duration::from_secs(4)).await; // Python: 4s cho SPA

    let probe = d.probe_session(&s).await?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let shot = PathBuf::from(format!("probe_{ts}.png"));
    d.screenshot(&s, &shot).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "state": probe.state.as_str(),
            "url": probe.url,
            "has_sessionid": probe.has_sessionid,
            "composer": probe.composer,
            "region_restricted": probe.url.contains("/security/region-restricted"),
            "login_markers_seen": probe.texts.iter().filter(|t| lvc_core::has_login_marker(t)).collect::<Vec<_>>(),
            "screenshot": shot,
        }))?
    );
    d.close(s).await?;
    Ok(())
}
