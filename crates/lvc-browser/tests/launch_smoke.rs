use lvc_browser::{BrowserDriver, LaunchOpts, chromium::ChromiumDriver};
use lvc_core::{Cookie, SessionState};

fn opts_headless(dir: &tempfile::TempDir) -> LaunchOpts {
    LaunchOpts {
        profile_dir: dir.path().join("p"),
        headless: true,
        proxy_server: None,
        window_offscreen: false,
        chrome_executable: None,
    }
}

#[tokio::test]
#[ignore = "can Chrome/Chromium tren may; chay: cargo test -p lvc-browser -- --ignored"]
async fn navigator_webdriver_la_false() {
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let s = d.launch(&opts_headless(&dir)).await.expect("launch");
    // navigator.webdriver phai la undefined/false (native, khong qua script tiem — xem C-1)
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
    d.close(s).await.unwrap();
}

#[tokio::test]
#[ignore = "can Chrome/Chromium tren may; chay: cargo test -p lvc-browser -- --ignored"]
async fn navigator_language_la_ja_jp() {
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let s = d.launch(&opts_headless(&dir)).await.expect("launch");
    let lang: String = s
        .page
        .evaluate("navigator.language")
        .await
        .unwrap()
        .into_value()
        .unwrap();
    assert_eq!(lang, "ja-JP");
    let langs: Vec<String> = s
        .page
        .evaluate("navigator.languages")
        .await
        .unwrap()
        .into_value()
        .unwrap();
    assert_eq!(langs.first().map(String::as_str), Some("ja-JP"));
    d.close(s).await.unwrap();
}

#[tokio::test]
#[ignore = "can Chrome/Chromium tren may; chay: cargo test -p lvc-browser -- --ignored"]
async fn cookie_dieu_huong_probe_screenshot() {
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let s = d.launch(&opts_headless(&dir)).await.expect("launch");

    // Review Task 3-6 (I-2): dat cookie TRUOC khi dieu huong, khop dung thu tu cua
    // examples/probe_login.rs (launch -> add_cookies -> goto). add_cookies() tu dat
    // CookieParam.url nen khong can trang nao dang mo san.
    let cookie = Cookie {
        name: "lvc_probe_test".to_string(),
        value: "1".to_string(),
        domain: "example.com".to_string(),
        path: "/".to_string(),
        secure: false,
        http_only: false,
        expires: None,
    };
    d.add_cookies(&s, std::slice::from_ref(&cookie))
        .await
        .expect("add_cookies");

    // goto sau khi da co cookie — chi de doc lai qua page.get_cookies() (cookie do
    // theo "tab hien tai") va cho probe_session() co mot URL that de doc.
    d.goto(&s, "https://example.com/")
        .await
        .expect("goto example.com");

    let seen = s.page.get_cookies().await.expect("get_cookies");
    assert!(
        seen.iter()
            .any(|c| c.name == "lvc_probe_test" && c.value == "1"),
        "cookie vua dat phai doc lai duoc: {seen:?}"
    );

    // Khong co sessionid Dola tren example.com -> LoggedOut theo classify_session
    let probe = d.probe_session(&s).await.expect("probe_session");
    assert!(
        !probe.has_sessionid,
        "example.com khong co cookie sessionid"
    );
    assert_eq!(probe.state, SessionState::LoggedOut);

    let shot = dir.path().join("probe.png");
    d.screenshot(&s, &shot).await.expect("screenshot");
    let meta = tokio::fs::metadata(&shot).await.expect("screenshot file");
    assert!(meta.len() > 0, "screenshot phai co du lieu");

    d.close(s).await.unwrap();
}
