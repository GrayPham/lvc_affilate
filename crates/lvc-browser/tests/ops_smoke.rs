//! Smoke test cho `lvc_browser::ops` tren fixture cuc bo `tests/fixtures/composer.html`
//! (KHONG goi dola.com). Tat ca `#[ignore]` vi can Chromium:
//! `cargo test -p lvc-browser --test ops_smoke -- --ignored` (env `LVC_CHROME` tuy chon;
//! mac dinh resolver chon Chromium Playwright rev cao nhat).
//!
//! Fixture duoc phuc vu qua mot server HTTP ti hon tren `127.0.0.1:0` (xem [`serve_fixtures`])
//! chu KHONG mo `file://`: do 2026-09-18 tren Chromium 1234 - `fetch()` tu origin `file://`
//! bi CORS chan ("Failed to fetch") va `Network.responseReceived` KHONG bao gio ban, nen
//! `count_responses` khong test duoc; 10/12 test con lai chay duoc tren file:// nhung dung
//! mot transport cho ca bo de don gian (va gan Dola https hon).

use std::path::PathBuf;
use std::time::Duration;

use lvc_browser::chromium::{ChromiumDriver, ChromiumSession};
use lvc_browser::{BrowserDriver, LaunchOpts, ops};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn opts_headless(dir: &tempfile::TempDir) -> LaunchOpts {
    LaunchOpts {
        profile_dir: dir.path().join("p"),
        headless: true,
        executable_override: std::env::var_os("LVC_CHROME").map(PathBuf::from),
        ..LaunchOpts::default()
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Tra (status, content-type, body) cho duong dan HTTP: 2 endpoint gia lap upload cua Dola tra
/// 200 JSON, `/upload/v1/loi` tra 500 (de chac `count_2xx` KHONG dem non-2xx), con lai doc file
/// trong `tests/fixtures/` (cam `..`).
fn route(path: &str) -> (&'static str, &'static str, Vec<u8>) {
    match path {
        "/alice/resource/prepare_upload" | "/upload/v1/x" => {
            ("200 OK", "application/json", b"{\"ok\":true}".to_vec())
        }
        "/upload/v1/loi" => (
            "500 Internal Server Error",
            "application/json",
            b"{}".to_vec(),
        ),
        p if p.contains("..") => ("400 Bad Request", "text/plain", b"cam".to_vec()),
        p => match std::fs::read(fixtures_dir().join(p.trim_start_matches('/'))) {
            Ok(b) => ("200 OK", "text/html; charset=utf-8", b),
            Err(_) => ("404 Not Found", "text/plain", b"khong co".to_vec()),
        },
    }
}

/// Server HTTP/1.1 toi gian (moi ket noi 1 request, `Connection: close`) tren cong ngau nhien;
/// tra `http://127.0.0.1:<port>` + task accept (abort trong `Fx::close`).
async fn serve_fixtures() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0");
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0_u8; 16 * 1024];
                let mut n = 0;
                loop {
                    let r = sock.read(&mut buf[n..]).await.unwrap_or(0);
                    if r == 0 {
                        break;
                    }
                    n += r;
                    if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") || n == buf.len() {
                        break;
                    }
                }
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split(' ').nth(1))
                    .unwrap_or("/")
                    .split('?')
                    .next()
                    .unwrap_or("/")
                    .to_string();
                let (status, ctype, body) = route(&path);
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\
                     Cache-Control: no-store\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.write_all(&body).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    (format!("http://127.0.0.1:{port}"), task)
}

/// Tam rect (doc lai tu trang) de so voi toa do mousedown trong log.
#[derive(serde::Deserialize)]
struct Center {
    cx: f64,
    cy: f64,
}

struct Fx {
    d: ChromiumDriver,
    s: ChromiumSession,
    server: tokio::task::JoinHandle<()>,
    _dir: tempfile::TempDir,
}

impl Fx {
    async fn open() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (base, server) = serve_fixtures().await;
        let d = ChromiumDriver;
        let s = d.launch(&opts_headless(&dir)).await.expect("launch");
        d.goto(&s, &format!("{base}/composer.html"))
            .await
            .expect("goto fixture");
        Self {
            d,
            s,
            server,
            _dir: dir,
        }
    }

    fn page(&self) -> &chromiumoxide::Page {
        self.s.page()
    }

    async fn log(&self) -> String {
        ops::eval_json(self.page(), "document.getElementById('log').textContent")
            .await
            .expect("doc #log")
    }

    async fn textarea(&self) -> String {
        ops::eval_json(self.page(), "document.getElementById('prompt').value")
            .await
            .expect("doc textarea")
    }

    async fn close(self) {
        self.d.close(self.s).await.expect("close");
        self.server.abort();
    }
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn eval_json_va_eval_with_arg() {
    let fx = Fx::open().await;
    let p = fx.page();

    let sum3: i64 = ops::eval_json(p, "1 + 2").await.unwrap();
    assert_eq!(sum3, 3);
    // Promise duoc await (awaitPromise=true)
    let resolved: String = ops::eval_json(p, "Promise.resolve('xong')").await.unwrap();
    assert_eq!(resolved, "xong");
    // undefined -> JSON null (khong loi "No value found")
    let undef: serde_json::Value = ops::eval_json(p, "undefined").await.unwrap();
    assert!(undef.is_null());
    let missing: Option<String> = ops::eval_json(p, "window.__khong_co").await.unwrap();
    assert_eq!(missing, None);
    // Ngoai le JS -> BrowserError::Js voi thong diep doc duoc
    let err = ops::eval_json::<i64>(p, "null.foo").await.unwrap_err();
    assert!(
        matches!(&err, lvc_browser::BrowserError::Js(m) if m.contains("TypeError")),
        "{err:?}"
    );

    let sum: i64 = ops::eval_with_arg(p, "(o) => o.a + o.b", &serde_json::json!({"a": 2, "b": 40}))
        .await
        .unwrap();
    assert_eq!(sum, 42);
    let echo: String = ops::eval_with_arg(p, "(s) => s + '!'", &"dấu \"nháy\" \\ và 日本語\n")
        .await
        .unwrap();
    assert_eq!(echo, "dấu \"nháy\" \\ và 日本語\n!");
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn click_js_bam_dung_tam_nut() {
    let fx = Fx::open().await;
    let p = fx.page();

    // #far-btn nam duoi spacer 1500px -> ngoai viewport -> phai scrollIntoView roi bam.
    let hit = ops::click_js(p, "document.getElementById('far-btn')")
        .await
        .unwrap();
    assert!(hit);
    let log = fx.log().await;
    assert!(log.contains("far clicked"), "{log}");
    assert!(log.contains("click far-btn detail=1"), "{log}");

    // Toa do mousedown phai la TAM rect hien tai cua nut (sai so 1px).
    let r: Center = ops::eval_json(
        p,
        "(()=>{const r=document.getElementById('far-btn').getBoundingClientRect();return {cx:r.x+r.width/2,cy:r.y+r.height/2};})()",
    )
    .await
    .unwrap();
    let md = log
        .lines()
        .find(|l| l.starts_with("mousedown ") && l.contains(" far-btn "))
        .unwrap_or_else(|| panic!("khong co mousedown tren far-btn: {log}"));
    let parts: Vec<&str> = md.split(' ').collect();
    let (x, y): (f64, f64) = (parts[1].parse().unwrap(), parts[2].parse().unwrap());
    assert!(
        (x - r.cx).abs() <= 1.0 && (y - r.cy).abs() <= 1.0,
        "{md} vs ({}, {})",
        r.cx,
        r.cy
    );
    assert!(md.ends_with("button=0"), "nut trai: {md}");

    // Khong tim thay -> Ok(false); phan tu display:none (rect 0x0) -> Ok(false), khong bam.
    assert!(
        !ops::click_js(p, "document.querySelector('#khong-co')")
            .await
            .unwrap()
    );
    assert!(
        !ops::click_js(p, "document.getElementById('hidden-btn')")
            .await
            .unwrap()
    );
    assert!(!fx.log().await.contains("hidden-btn"));

    // Chuoi hanh dong composer: 高速 -> menu プロ -> 動画を作成 -> nut 10s/比率.
    assert!(
        ops::click_js(p, "document.getElementById('mode-btn')")
            .await
            .unwrap()
    );
    let menu_open: bool = ops::eval_json(
        p,
        "document.getElementById('mode-menu').classList.contains('open')",
    )
    .await
    .unwrap();
    assert!(menu_open);
    assert!(ops::click_js(p, "[...document.querySelectorAll('[role=menuitem]')].find(e => e.textContent.trim() === 'プロ')").await.unwrap());
    let mode: String = ops::eval_json(p, "document.getElementById('mode-btn').textContent")
        .await
        .unwrap();
    assert_eq!(mode, "プロ");
    assert!(ops::click_js(p, "[...document.querySelectorAll('button')].find(b => b.textContent.trim() === '動画を作成')").await.unwrap());
    assert!(
        ops::click_js(
            p,
            "[...document.querySelectorAll('button')].find(b => /比率/.test(b.textContent))"
        )
        .await
        .unwrap()
    );
    assert!(fx.log().await.contains("click ratio-btn"));
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn click_js_at_offset_bam_goc_phai_tren_the_dinh_kem() {
    let fx = Fx::open().await;
    let p = fx.page();
    // Nut x 16x16 o goc phai tren the 120x90: bam (phai-4, tren+4) nhu Python
    // _clear_composer_attachments (video_worker_ui.py:820).
    let hit = ops::click_js_at_offset(p, "document.getElementById('attach-card')", 4.0, 4.0)
        .await
        .unwrap();
    assert!(hit);
    let log = fx.log().await;
    assert!(log.contains("attach removed"), "{log}");
    assert!(
        log.contains("mousedown ") && log.contains(" attach-x "),
        "{log}"
    );
    // The da bi xoa -> lan sau khong tim thay -> false
    assert!(
        !ops::click_js_at_offset(p, "document.getElementById('attach-card')", 4.0, 4.0)
            .await
            .unwrap()
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn press_ctrl_a_delete_xoa_textarea() {
    let fx = Fx::open().await;
    let p = fx.page();
    assert!(
        ops::click_js(p, "document.getElementById('prompt')")
            .await
            .unwrap()
    );
    ops::insert_text(p, "noi dung cu can xoa").await.unwrap();
    assert_eq!(fx.textarea().await, "noi dung cu can xoa");

    ops::press(p, "Control+A").await.unwrap();
    ops::press(p, "Delete").await.unwrap();
    assert_eq!(fx.textarea().await, "", "Ctrl+A roi Delete phai xoa sach");

    let log = fx.log().await;
    assert!(
        log.contains("keydown Control code=ControlLeft ctrl=true"),
        "{log}"
    );
    assert!(
        log.contains("keydown A code=KeyA ctrl=true shift=false keyCode=65"),
        "{log}"
    );
    assert!(
        log.contains("keydown Delete code=Delete ctrl=false"),
        "{log}"
    );
    // Ctrl+A KHONG duoc chen chu 'A' (text rong khi co Ctrl)
    assert!(
        !log.contains("insertText \"noi dung cu can xoaA\""),
        "{log}"
    );
    // Thu tu nha phim: keyup A truoc, keyup Control sau (Playwright parity)
    let up_a = log.find("keyup A").unwrap();
    let up_ctrl = log.find("keyup Control").unwrap();
    assert!(up_a < up_ctrl, "{log}");

    // End + Enter: Enter sinh xuong dong trong textarea
    ops::insert_text(p, "x").await.unwrap();
    ops::press(p, "End").await.unwrap();
    ops::press(p, "Enter").await.unwrap();
    assert_eq!(fx.textarea().await, "x\n");
    assert!(fx.log().await.contains("keydown Enter code=Enter"));

    // Phim la -> loi ro, khong panic
    assert!(ops::press(p, "PhimKhongCo").await.is_err());
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn insert_text_mot_phat() {
    let fx = Fx::open().await;
    let p = fx.page();
    assert!(
        ops::click_js(p, "document.getElementById('prompt')")
            .await
            .unwrap()
    );

    // Mot dong: dung 1 su kien `input` (inputType insertText), khong keydown nao.
    let one = "một dòng 日本語 \"quote\" \\ back";
    ops::insert_text(p, one).await.unwrap();
    assert_eq!(fx.textarea().await, one);
    let log = fx.log().await;
    assert!(
        !log.contains("keydown"),
        "insertText khong sinh keydown nao: {log}"
    );
    let inputs = log.lines().filter(|l| l.starts_with("input ")).count();
    assert_eq!(inputs, 1, "mot su kien input duy nhat (mot phat): {log}");
    assert!(log.contains("input insertText "), "{log}");

    // Nhieu dong: gia tri textarea == chuoi co `\n`, KHONG gui Enter (do 2026-09-18: Chrome tach
    // commit da dong thanh nhieu su kien `input` insertText - van la MOT lenh CDP).
    ops::press(p, "Control+A").await.unwrap();
    ops::press(p, "Delete").await.unwrap();
    let text = "dòng 1\ndòng 2 日本語\n\ndòng cuối";
    ops::insert_text(p, text).await.unwrap();
    assert_eq!(fx.textarea().await, text);
    let log = fx.log().await;
    assert!(
        !log.contains("keydown Enter"),
        "khong duoc gui Enter: {log}"
    );
    assert!(
        !log.lines().any(|l| l.starts_with("input insertLineBreak")),
        "\\n di qua insertText, khong phai insertLineBreak: {log}"
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn set_file_input_2_file() {
    let fx = Fx::open().await;
    let p = fx.page();
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("anh_a.png");
    let b = dir.path().join("anh_b.png");
    std::fs::write(&a, b"\x89PNG a").unwrap();
    std::fs::write(&b, b"\x89PNG b").unwrap();

    ops::set_file_input(p, "input[type=file]", &[a, b])
        .await
        .unwrap();
    let ok = ops::wait_until(
        p,
        "document.getElementById('files').textContent === '2'",
        Duration::from_secs(5),
        Duration::from_millis(50),
    )
    .await
    .unwrap();
    assert!(ok, "log: {}", fx.log().await);
    let names: Vec<String> = ops::eval_json(
        p,
        "[...document.getElementById('file-input').files].map(f => f.name)",
    )
    .await
    .unwrap();
    assert_eq!(names, vec!["anh_a.png", "anh_b.png"]);

    // CSS khong khop -> loi (khong im lang)
    assert!(
        ops::set_file_input(p, "#khong-co-input", &[dir.path().join("x")])
            .await
            .is_err()
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn wait_until_het_han_tra_false() {
    let fx = Fx::open().await;
    let p = fx.page();

    let t0 = std::time::Instant::now();
    let r = ops::wait_until(
        p,
        "false",
        Duration::from_millis(400),
        Duration::from_millis(50),
    )
    .await
    .unwrap();
    assert!(!r);
    assert!(
        t0.elapsed() >= Duration::from_millis(400),
        "{:?}",
        t0.elapsed()
    );

    // Dung ngay khi true (khong cho het han)
    let t0 = std::time::Instant::now();
    assert!(
        ops::wait_until(
            p,
            "1 === 1",
            Duration::from_secs(10),
            Duration::from_millis(50)
        )
        .await
        .unwrap()
    );
    assert!(t0.elapsed() < Duration::from_secs(2));

    // Tro thanh true sau 300ms
    let _: serde_json::Value =
        ops::eval_json(p, "setTimeout(() => { window.__ok = true; }, 300); null")
            .await
            .unwrap();
    assert!(
        ops::wait_until(
            p,
            "window.__ok === true",
            Duration::from_secs(5),
            Duration::from_millis(50)
        )
        .await
        .unwrap()
    );

    // Gia tri khong phai bool: coi theo truthiness JS (undefined/null/0/'' -> false)
    assert!(
        !ops::wait_until(
            p,
            "window.__khong_co",
            Duration::from_millis(100),
            Duration::from_millis(50)
        )
        .await
        .unwrap()
    );
    assert!(
        ops::wait_until(
            p,
            "'chuoi'",
            Duration::from_millis(100),
            Duration::from_millis(50)
        )
        .await
        .unwrap()
    );

    // Trang tu dieu huong giua chung (context bi huy) -> KHONG loi, tiep tuc cho toi khi true.
    let _: serde_json::Value = ops::eval_json(p, "setTimeout(() => location.reload(), 100); null")
        .await
        .unwrap();
    let ok = ops::wait_until(
        p,
        "document.readyState === 'complete' && performance.now() < 1500",
        Duration::from_secs(10),
        Duration::from_millis(20),
    )
    .await
    .unwrap();
    assert!(ok, "sau reload trang moi co performance.now() nho");

    // Loi JS that (khong phai context) -> Err
    assert!(
        ops::wait_until(
            p,
            "null.x",
            Duration::from_millis(100),
            Duration::from_millis(50)
        )
        .await
        .is_err()
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn reload_cho_load_event_roi_settle() {
    let fx = Fx::open().await;
    let p = fx.page();
    let _: serde_json::Value = ops::eval_json(
        p,
        "window.__marker = 7; document.getElementById('log').textContent = 'cu'; null",
    )
    .await
    .unwrap();
    let t0 = std::time::Instant::now();
    ops::reload(p, Duration::from_millis(300)).await.unwrap();
    assert!(t0.elapsed() >= Duration::from_millis(300));
    let m: Option<i64> = ops::eval_json(p, "window.__marker").await.unwrap();
    assert_eq!(m, None, "document moi sau reload");
    assert_eq!(fx.log().await, "", "log moi rong");
    // Trang van dung duoc sau reload (context moi)
    assert!(
        ops::click_js(p, "document.getElementById('far-btn')")
            .await
            .unwrap()
    );
    assert!(fx.log().await.contains("far clicked"));
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn count_responses_dem_2xx() {
    let fx = Fx::open().await;
    let p = fx.page();
    let counter = ops::count_responses(p).await.unwrap();
    assert_eq!(counter.count_2xx("/upload/v1/"), 0);

    assert!(
        ops::click_js(p, "document.getElementById('fetch-btn')")
            .await
            .unwrap()
    );
    let done = ops::wait_until(
        p,
        "window.__fetched === 3",
        Duration::from_secs(10),
        Duration::from_millis(50),
    )
    .await
    .unwrap();
    assert!(done, "log: {}", fx.log().await);
    // Su kien CDP co the den sau khi JS thay ket qua mot chut
    tokio::time::sleep(Duration::from_millis(300)).await;
    let seen = counter.snapshot();
    assert_eq!(
        counter.count_2xx("/upload/v1/"),
        1,
        "chi /upload/v1/x (200), khong dem /upload/v1/loi (500) - log: {}\nseen: {seen:?}",
        fx.log().await
    );
    assert_eq!(
        counter.count_2xx("/alice/resource/prepare_upload"),
        1,
        "seen: {seen:?}"
    );
    assert_eq!(counter.count_2xx("/khong/co/"), 0);
    assert!(
        seen.iter()
            .any(|(u, st)| u.ends_with("/upload/v1/loi") && *st == 500),
        "500 van duoc ghi nhan trong snapshot: {seen:?}"
    );
    let log = fx.log().await;
    assert!(log.contains("fetch upload/v1/x status=200"), "{log}");
    assert!(log.contains("fetch upload/v1/loi status=500"), "{log}");
    drop(counter);
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn frame_urls_thay_bdcaptcha() {
    let fx = Fx::open().await;
    let p = fx.page();
    let before = ops::frame_urls(p).await.unwrap();
    assert_eq!(before.len(), 1, "{before:?}");
    assert!(before[0].ends_with("composer.html"), "{before:?}");

    assert!(
        ops::click_js(p, "document.getElementById('toggle-captcha')")
            .await
            .unwrap()
    );
    assert!(
        ops::wait_until(
            p,
            "window.__captcha_loaded === true",
            Duration::from_secs(10),
            Duration::from_millis(50)
        )
        .await
        .unwrap()
    );
    let after = ops::frame_urls(p).await.unwrap();
    assert_eq!(after.len(), 2, "{after:?}");
    assert!(
        after.iter().any(|u| u.contains("bdcaptcha.html")),
        "{after:?}"
    );

    // Bo iframe -> het
    assert!(
        ops::click_js(p, "document.getElementById('toggle-captcha')")
            .await
            .unwrap()
    );
    assert!(
        ops::wait_until(
            p,
            "!document.querySelector('iframe')",
            Duration::from_secs(5),
            Duration::from_millis(50)
        )
        .await
        .unwrap()
    );
    let gone = ops::frame_urls(p).await.unwrap();
    assert!(
        !gone.iter().any(|u| u.contains("bdcaptcha.html")),
        "{gone:?}"
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn cookie_header_va_delete_cookies() {
    use chromiumoxide::cdp::browser_protocol::network::CookieParam;
    let fx = Fx::open().await;
    let p = fx.page();
    // Cookie cho origin http://127.0.0.1/ (khong phai trang hien tai) - dat qua Network.setCookie
    // voi url tuong minh; doc lai bang cookie_header(url) giong Python context.cookies(url).
    for (n, v) in [("a", "1"), ("b", "2"), ("sessionid", "abc")] {
        p.set_cookie(
            CookieParam::builder()
                .name(n)
                .value(v)
                .url("http://127.0.0.1/")
                .build()
                .unwrap(),
        )
        .await
        .unwrap();
    }
    let header = ops::cookie_header(p, "http://127.0.0.1/").await.unwrap();
    let mut parts: Vec<&str> = header.split("; ").collect();
    parts.sort_unstable();
    assert_eq!(parts, vec!["a=1", "b=2", "sessionid=abc"], "{header}");
    // URL khac host -> khong co cookie nao
    assert_eq!(
        ops::cookie_header(p, "http://example.invalid/")
            .await
            .unwrap(),
        ""
    );

    ops::delete_cookies(p, "127.0.0.1", &["a", "sessionid", "khong_co"])
        .await
        .unwrap();
    assert_eq!(
        ops::cookie_header(p, "http://127.0.0.1/").await.unwrap(),
        "b=2"
    );
    // Sai domain -> khong xoa gi
    ops::delete_cookies(p, "example.invalid", &["b"])
        .await
        .unwrap();
    assert_eq!(
        ops::cookie_header(p, "http://127.0.0.1/").await.unwrap(),
        "b=2"
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test ops_smoke -- --ignored"]
async fn user_agent_va_url() {
    let fx = Fx::open().await;
    let p = fx.page();
    let ua = ops::user_agent(p).await.unwrap();
    assert!(ua.contains("Chrome/"), "{ua}");
    let js_ua: String = ops::eval_json(p, "navigator.userAgent").await.unwrap();
    assert_eq!(ua, js_ua, "UA phai la navigator.userAgent that cua trang");
    let url = ops::url(p).await.unwrap();
    assert!(
        url.starts_with("http://127.0.0.1:") && url.ends_with("/composer.html"),
        "{url}"
    );
    let href: String = ops::eval_json(p, "location.href").await.unwrap();
    assert_eq!(url, href);
    fx.close().await;
}
