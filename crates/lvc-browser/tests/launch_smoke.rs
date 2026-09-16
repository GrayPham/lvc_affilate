use lvc_browser::{BrowserDriver, LaunchOpts, chromium::ChromiumDriver};

#[tokio::test]
#[ignore = "can Chrome/Chromium tren may; chay: cargo test -p lvc-browser -- --ignored"]
async fn launch_headless_va_dong() {
    let dir = tempfile::tempdir().unwrap();
    let opts = LaunchOpts {
        profile_dir: dir.path().join("p"),
        headless: true,
        proxy_server: None,
        window_offscreen: false,
        chrome_executable: None,
    };
    let d = ChromiumDriver;
    let s = d.launch(&opts).await.expect("launch");
    // navigator.webdriver phai la undefined/false sau stealth
    let wd: serde_json::Value = s
        .page
        .evaluate("navigator.webdriver")
        .await
        .unwrap()
        .into_value()
        .unwrap_or(serde_json::Value::Null);
    assert!(
        wd.is_null() || wd == serde_json::Value::Bool(false),
        "navigator.webdriver = {wd}"
    );
    let lang: String = s
        .page
        .evaluate("navigator.language")
        .await
        .unwrap()
        .into_value()
        .unwrap();
    assert_eq!(lang, "ja-JP");
    d.close(s).await.unwrap();
}
