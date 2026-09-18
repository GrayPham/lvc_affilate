//! Task 13 smoke (`#[ignore]`, can Chromium): vong doi tien trinh, trang dau, phan loai loi dieu
//! huong, xac thuc proxy qua Fetch (proxy GIA cuc bo), dong lenh Chrome. KHONG bao gio goi dola.com
//! hay mang ngoai: proxy gia tu tra loi moi request (khong forward).
//! Chay: `LVC_CHROME=... cargo test -p lvc-browser --test lifecycle_smoke -- --ignored`

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use base64::Engine as _;
use chromiumoxide::cdp::browser_protocol::target::GetTargetsParams;
use common::opts_headless;
use lvc_browser::chromium::{ChromiumDriver, LAUNCH_ARGS};
use lvc_browser::{BrowserDriver, BrowserError, LaunchOpts, ops};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Cong loopback vua dong (bind `127.0.0.1:0` roi tha) → `Navigation{net_error:
/// net::ERR_CONNECTION_REFUSED}`, khong phai proxy chet; URL trong loi khong co query. (Brief goi y
/// cong 1 nhung Chrome chan cong do tu dau bang `net::ERR_UNSAFE_PORT` — do 2026-09-18.)
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn goto_url_khong_ton_tai_bao_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let s = d.launch(&opts_headless(&dir)).await.expect("launch");
    assert!(s.is_connected());
    let port = closed_port().await;
    let t0 = Instant::now();
    let err = d
        .goto(&s, &format!("http://127.0.0.1:{port}/?bi_mat=1"))
        .await
        .expect_err("cong da dong phai bi tu choi");
    match &err {
        BrowserError::Navigation {
            url_host_path,
            net_error,
        } => {
            assert_eq!(url_host_path, &format!("http://127.0.0.1:{port}/"));
            assert_eq!(net_error, "net::ERR_CONNECTION_REFUSED");
        }
        other => panic!("sai bien the: {other:?}"),
    }
    assert!(!err.is_proxy_down());
    assert!(!err.to_string().contains("bi_mat"), "{err}");
    assert!(t0.elapsed() < Duration::from_secs(30), "{:?}", t0.elapsed());
    d.close(s).await.unwrap();
}

/// Proxy `--proxy-server` tro toi cong khong ai nghe → `net::ERR_PROXY_CONNECTION_FAILED` →
/// `is_proxy_down()` (engine T15 gan `FailureCode::ProxyDown`).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn goto_qua_proxy_chet_bao_is_proxy_down() {
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let port = closed_port().await;
    let opts = LaunchOpts {
        proxy: lvc_core::parse_proxy(&format!("http://127.0.0.1:{port}")).unwrap(),
        ..opts_headless(&dir)
    };
    let s = d.launch(&opts).await.expect("launch");
    let err = d
        .goto(&s, "http://proxy-chet.lvc.invalid/x")
        .await
        .expect_err("proxy chet phai loi");
    assert!(err.is_proxy_down(), "{err:?}");
    assert!(
        matches!(&err, BrowserError::Navigation { net_error, .. } if net_error.starts_with("net::ERR_PROXY_")),
        "{err:?}"
    );
    d.close(s).await.unwrap();
}

/// Drop `ChromiumSession` (khong `close()`) trong runtime tokio → Chrome duoc dong NHE NHANG trong
/// nen: sau 5 s PID khong con, `lockfile` khong con bi giu (Chrome mo no voi `DELETE_ON_CLOSE` nen
/// thuong bien mat — do 2026-09-18; con lai thi phai mo duoc), va `Preferences.profile.exit_type ==
/// "Normal"` — bang chung thoat SACH (kill → Chrome khong kip ghi, van "Crashed").
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn drop_session_khong_bo_roi_chrome() {
    let dir = tempfile::tempdir().unwrap();
    let opts = opts_headless(&dir);
    let d = ChromiumDriver;
    let s = d.launch(&opts).await.expect("launch");
    let pid = s.pid().expect("pid");
    let profile = std::path::absolute(&opts.profile_dir).unwrap();
    let lockfile = profile.join("lockfile");
    assert!(lockfile.exists(), "Chrome phai tao lockfile");
    assert!(pid_alive(pid).await, "Chrome phai dang chay");

    drop(s);

    tokio::time::sleep(Duration::from_secs(5)).await;
    assert!(!pid_alive(pid).await, "Chrome {pid} van song 5 s sau drop");
    match std::fs::OpenOptions::new()
        .write(true)
        .truncate(false)
        .open(&lockfile)
    {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => panic!("lockfile van bi giu: {e}"),
    }
    assert_eq!(
        exit_type(&profile),
        "Normal",
        "Chrome phai thoat sach sau drop"
    );
}

/// `close()` cung thoat sach (`exit_type == "Normal"`) va tra ve truoc 30 s (khong doi het `CLOSE_WAIT`).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn close_thoat_sach_va_khong_doi_het_30s() {
    let dir = tempfile::tempdir().unwrap();
    let opts = opts_headless(&dir);
    let d = ChromiumDriver;
    let s = d.launch(&opts).await.expect("launch");
    let pid = s.pid().expect("pid");
    let profile = std::path::absolute(&opts.profile_dir).unwrap();
    let t0 = Instant::now();
    d.close(s).await.unwrap();
    assert!(t0.elapsed() < Duration::from_secs(20), "{:?}", t0.elapsed());
    assert!(!pid_alive(pid).await);
    assert_eq!(exit_type(&profile), "Normal");
}

/// Chrome bi kill tu ngoai → `is_connected()` ve `false` (handler ket thuc) va `close()` khong treo
/// 30 s (kenh dong → tra ngay; `wait()` thay tien trinh da chet).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn chrome_bi_kill_thi_is_connected_false_va_close_khong_treo() {
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let s = d.launch(&opts_headless(&dir)).await.expect("launch");
    let pid = s.pid().expect("pid");
    assert!(s.is_connected());

    let out = tokio::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .await
        .expect("taskkill");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );

    let t0 = Instant::now();
    while s.is_connected() && t0.elapsed() < Duration::from_secs(10) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        !s.is_connected(),
        "handler phai ket thuc sau khi Chrome chet"
    );
    // lenh CDP sau do → Disconnected (khong phai Cdp chung chung)
    let err = ops::url(s.page()).await.expect_err("Chrome da chet");
    assert!(matches!(err, BrowserError::Disconnected), "{err:?}");

    let t0 = Instant::now();
    d.close(s).await.unwrap();
    assert!(
        t0.elapsed() < Duration::from_secs(10),
        "close() treo {:?}",
        t0.elapsed()
    );
}

/// M-3: dung TRANG DAU cua Chrome (khong `Target.createTarget`) → dung 1 tab, dang o `about:blank`,
/// va `page()` chinh la tab do. Headed (cua so ngoai man hinh) vi headless luon mo about:blank san,
/// con headed mo NTP — truong hop kho hon.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn trang_dau_dung_1_trang_headed() {
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let opts = LaunchOpts {
        headless: false,
        window_offscreen: true,
        ..opts_headless(&dir)
    };
    let s = d.launch(&opts).await.expect("launch headed");
    let browser = s.browser().expect("browser truoc close");
    let pages = browser.pages().await.unwrap();
    assert_eq!(pages.len(), 1, "phai dung 1 tab sau launch");
    assert_eq!(pages[0].target_id(), s.page().target_id());
    assert_eq!(
        ops::url(s.page()).await.unwrap(),
        "about:blank",
        "tab dau phai duoc dua ve about:blank"
    );
    // Hoi thang Chrome (khong qua so sach cua chromiumoxide): so target kieu "page".
    let targets = browser
        .execute(GetTargetsParams::default())
        .await
        .unwrap()
        .result
        .target_infos;
    let n_pages = targets.iter().filter(|t| t.r#type == "page").count();
    assert_eq!(n_pages, 1, "{targets:?}");
    d.close(s).await.unwrap();
}

/// Proxy GIA cuc bo: khong co `Proxy-Authorization` dung → `407` + `Proxy-Authenticate: Basic`; co →
/// tu tra `200` (khong forward, khong mang ngoai). Ghi lai gia tri header da thay.
/// - `GET http://…/ok` → trang co dinh `<body id="ok">`; `/song-song` → trang ban 20 `fetch('/sub/i')`
///   cung luc (review T13 I-1); `/sub/<i>` → text ngan.
/// - `CONNECT host:443` (review T13 M-9): 407 khi thieu header; co → `200 Connection established`, doc
///   `ClientHello` roi DONG (khong co TLS) → Chrome bao `ERR_CONNECTION_CLOSED` (tunnel da qua xac thuc).
struct FakeProxy {
    addr: String,
    n407: Arc<AtomicUsize>,
    n200: Arc<AtomicUsize>,
    auth_seen: Arc<std::sync::Mutex<Vec<String>>>,
    /// Moi dong `CONNECT`: (host:port, gia tri `Proxy-Authorization` neu co) theo thu tu den.
    connect_seen: ConnectLog,
    task: tokio::task::JoinHandle<()>,
}

/// Nhat ky `CONNECT` cua proxy gia: `(host:port, Proxy-Authorization)` theo thu tu den.
type ConnectLog = Arc<std::sync::Mutex<Vec<(String, Option<String>)>>>;

/// Trang cho `/song-song`: 20 fetch cung luc toi `/sub/i`, xong thi `body#done` + `data-ok=<so 200>`.
const SONG_SONG_HTML: &str = r"<html><head><title>song song</title></head><body>
<script>
Promise.all(Array.from({length: 20}, (_, i) => fetch('/sub/' + i, {cache: 'no-store'}).then(r => r.status).catch(() => 0)))
  .then(st => { document.body.dataset.ok = String(st.filter(x => x === 200).length); document.body.id = 'done'; });
</script></body></html>";

impl FakeProxy {
    async fn start(expected_basic: String) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
        let n407 = Arc::new(AtomicUsize::new(0));
        let n200 = Arc::new(AtomicUsize::new(0));
        let auth_seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let connect_seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let (n407c, n200c, seenc, connc) = (
            Arc::clone(&n407),
            Arc::clone(&n200),
            Arc::clone(&auth_seen),
            Arc::clone(&connect_seen),
        );
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    break;
                };
                let (n407, n200, seen, conn, expected) = (
                    Arc::clone(&n407c),
                    Arc::clone(&n200c),
                    Arc::clone(&seenc),
                    Arc::clone(&connc),
                    expected_basic.clone(),
                );
                tokio::spawn(async move {
                    let mut buf = vec![0_u8; 32 * 1024];
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
                    let mut first = req.lines().next().unwrap_or("").split(' ');
                    let method = first.next().unwrap_or("").to_string();
                    let target = first.next().unwrap_or("").to_string();
                    let auth = req
                        .lines()
                        .find_map(|l| l.strip_prefix("Proxy-Authorization: "))
                        .map(str::trim)
                        .map(str::to_string);
                    if let Some(a) = &auth {
                        seen.lock().unwrap().push(a.clone());
                    }
                    let ok = auth.as_deref() == Some(expected.as_str());
                    if method == "CONNECT" {
                        conn.lock().unwrap().push((target.clone(), auth.clone()));
                    }
                    let resp = if !ok {
                        n407.fetch_add(1, Ordering::SeqCst);
                        "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm=\"lvc\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string()
                    } else if method == "CONNECT" {
                        n200.fetch_add(1, Ordering::SeqCst);
                        // Tunnel "mo" roi doc ClientHello va dong: du de chung minh CONNECT da qua
                        // xac thuc, khong can TLS cuc bo.
                        let _ = sock
                            .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                            .await;
                        let mut hello = [0_u8; 4096];
                        let _ = tokio::time::timeout(Duration::from_secs(2), sock.read(&mut hello))
                            .await;
                        let _ = sock.shutdown().await;
                        return;
                    } else {
                        n200.fetch_add(1, Ordering::SeqCst);
                        let path = target
                            .split("://")
                            .nth(1)
                            .and_then(|s| s.find('/').map(|i| &s[i..]))
                            .unwrap_or("/");
                        let (ctype, body) = if path == "/song-song" {
                            ("text/html; charset=utf-8", SONG_SONG_HTML.to_string())
                        } else if path.starts_with("/sub/") {
                            ("text/plain", format!("sub {path}"))
                        } else {
                            ("text/html; charset=utf-8", "<html><head><title>qua proxy</title></head><body id=\"ok\">qua proxy</body></html>".to_string())
                        };
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    };
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        Self {
            addr,
            n407,
            n200,
            auth_seen,
            connect_seen,
            task,
        }
    }

    /// `LaunchOpts` headless di qua proxy nay voi user:pass.
    fn opts(&self, dir: &tempfile::TempDir, user: &str, pass: &str) -> LaunchOpts {
        LaunchOpts {
            proxy: lvc_core::parse_proxy(&format!("http://{user}:{pass}@{}", self.addr)).unwrap(),
            ..opts_headless(dir)
        }
    }
}

/// `Basic base64(user:pass)` nhu Chrome gui.
fn basic(user: &str, pass: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"))
    )
}

/// Review T13 I-1: 20 request cung luc qua proxy co auth — moi request deu bi Fetch tam dung; vong
/// `proxy_auth_loop` phai tra loi PIPELINE (dinh so lenh dang bay ≥ 2, khong bao gio ≥ 2 neu tuan tu)
/// va ca 20 deu xong nhanh (< 5 s, xa muc 20 × round-trip + 20 × 407/retry).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn proxy_auth_tra_loi_song_song_20_request() {
    let (user, pass) = ("tenND", "matkhauBM");
    let proxy = FakeProxy::start(basic(user, pass)).await;
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let s = d
        .launch(&proxy.opts(&dir, user, pass))
        .await
        .expect("launch");
    // Some ngay sau launch (proxy co auth); gia tri co the da > 0 vi Chrome tu gui vai request nen
    // luc khoi dong (deu vao proxy gia, khong ra mang ngoai).
    assert!(s.proxy_auth_peak_inflight().is_some());

    let t0 = Instant::now();
    d.goto(&s, "http://proxy-test.lvc.invalid/song-song")
        .await
        .expect("goto /song-song");
    let mut id = String::new();
    while t0.elapsed() < Duration::from_secs(10) {
        id = ops::eval_json(s.page(), "document.body.id").await.unwrap();
        if id == "done" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let elapsed = t0.elapsed();
    assert_eq!(id, "done", "20 fetch khong xong trong 10 s");
    let ok: String = ops::eval_json(s.page(), "document.body.dataset.ok")
        .await
        .unwrap();
    assert_eq!(ok, "20", "ca 20 fetch phai 200 qua proxy");
    assert!(elapsed < Duration::from_secs(5), "qua cham: {elapsed:?}");
    assert!(
        proxy.n200.load(Ordering::SeqCst) >= 21,
        "{}",
        proxy.n200.load(Ordering::SeqCst)
    );
    let peak = s.proxy_auth_peak_inflight().expect("proxy co auth → Some");
    eprintln!(
        "[song-song] 20 fetch xong sau {elapsed:?}; dinh Fetch.continue* dang bay = {peak}; proxy 200 = {}, 407 = {}",
        proxy.n200.load(Ordering::SeqCst),
        proxy.n407.load(Ordering::SeqCst)
    );
    assert!(
        peak >= 2,
        "Fetch.continue* phai bay song song, dinh = {peak}"
    );

    d.close(s).await.unwrap();
    proxy.task.abort();
}

/// Review T13 M-9: duong HTTPS = `CONNECT host:443` qua proxy HTTP. Proxy gia thay `CONNECT` khong
/// header (→ 407) roi `CONNECT` cung host voi `Proxy-Authorization` dung (→ tunnel 200); vi khong co
/// TLS phia sau, Chrome bao `Navigation{net_error: net::ERR_CONNECTION_CLOSED}` va KHONG phai
/// `is_proxy_down()` (tunnel da mo, chi lop TLS that bai) — khac han `ERR_TUNNEL_CONNECTION_FAILED`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn proxy_auth_connect_https() {
    let (user, pass) = ("tenND", "matkhauBM");
    let expected = basic(user, pass);
    let proxy = FakeProxy::start(expected.clone()).await;
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let s = d
        .launch(&proxy.opts(&dir, user, pass))
        .await
        .expect("launch");

    let err = d
        .goto(&s, "https://connect-test.lvc.invalid/ok?bi_mat=1")
        .await
        .expect_err("khong co TLS phia sau tunnel → phai loi");
    match &err {
        BrowserError::Navigation {
            url_host_path,
            net_error,
        } => {
            assert_eq!(url_host_path, "https://connect-test.lvc.invalid/ok");
            assert!(
                matches!(
                    net_error.as_str(),
                    "net::ERR_CONNECTION_CLOSED"
                        | "net::ERR_CONNECTION_RESET"
                        | "net::ERR_EMPTY_RESPONSE"
                ),
                "loi phai o lop TLS/ket noi sau tunnel, khong phai proxy: {net_error}"
            );
        }
        other => panic!("sai bien the: {other:?}"),
    }
    assert!(!err.is_proxy_down(), "{err:?}");

    let conns = proxy.connect_seen.lock().unwrap().clone();
    let host = "connect-test.lvc.invalid:443";
    let ours: Vec<&(String, Option<String>)> = conns.iter().filter(|(h, _)| h == host).collect();
    assert!(
        ours.len() >= 2,
        "phai thay CONNECT it nhat 2 lan (407 roi co header): {conns:?}"
    );
    assert_eq!(
        ours[0].1, None,
        "CONNECT dau tien chua co header: {conns:?}"
    );
    assert!(
        ours.iter()
            .any(|(_, a)| a.as_deref() == Some(expected.as_str())),
        "CONNECT sau phai mang dung Proxy-Authorization: {conns:?}"
    );

    d.close(s).await.unwrap();
    proxy.task.abort();
}

/// Proxy co user:pass: `goto` qua proxy gia thanh cong (trang 200 cua proxy), proxy DA thay 407 truoc
/// roi moi thay `Proxy-Authorization: Basic base64(user:pass)` — tuc la Fetch.authRequired duoc tra
/// loi `ProvideCredentials` tren phien browser. Dong lenh Chrome khong chua mat khau.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn proxy_auth_qua_fetch_tren_phien_browser() {
    let (user, pass) = ("tenND", "matkhauBM");
    let expected = basic(user, pass);
    let proxy = FakeProxy::start(expected.clone()).await;
    let dir = tempfile::tempdir().unwrap();
    let d = ChromiumDriver;
    let opts = proxy.opts(&dir, user, pass);
    assert!(opts.proxy.as_ref().unwrap().has_auth());
    let s = d.launch(&opts).await.expect("launch");

    // Dong lenh that cua Chrome (Win32_Process) khong duoc chua mat khau/user.
    let cmd = command_line(s.pid().expect("pid")).await;
    assert!(
        !cmd.contains(pass) && !cmd.contains(user),
        "lo thong tin proxy: {cmd}"
    );
    assert!(
        cmd.contains(&format!("--proxy-server=http://{}", proxy.addr)),
        "{cmd}"
    );

    d.goto(&s, "http://proxy-test.lvc.invalid/ok")
        .await
        .expect("goto qua proxy co auth");
    let id: String = ops::eval_json(s.page(), "document.body.id").await.unwrap();
    assert_eq!(id, "ok", "trang phai la trang 200 cua proxy gia");
    assert!(
        ops::url(s.page()).await.unwrap().ends_with("/ok"),
        "{}",
        ops::url(s.page()).await.unwrap()
    );

    assert!(
        proxy.n407.load(Ordering::SeqCst) >= 1,
        "proxy phai thay it nhat 1 request chua xac thuc (407)"
    );
    assert!(proxy.n200.load(Ordering::SeqCst) >= 1);
    let seen = proxy.auth_seen.lock().unwrap().clone();
    assert!(
        seen.iter().any(|a| a == &expected),
        "proxy phai thay dung Proxy-Authorization; thay: {seen:?}"
    );

    d.close(s).await.unwrap();
    proxy.task.abort();
}

/// P1 I-3 (residual): dong lenh THAT cua tien trinh Chrome (`Win32_Process.CommandLine`) khong co
/// `--enable-automation`, co du 5 `LAUNCH_ARGS` + `--lang=ja-JP` + `--user-data-dir` tuyet doi.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test lifecycle_smoke -- --ignored"]
async fn command_line_khong_co_enable_automation() {
    let dir = tempfile::tempdir().unwrap();
    let opts = opts_headless(&dir);
    let d = ChromiumDriver;
    let s = d.launch(&opts).await.expect("launch");
    let cmd = command_line(s.pid().expect("pid")).await;
    assert!(!cmd.is_empty(), "khong doc duoc dong lenh");
    assert!(!cmd.contains("--enable-automation"), "{cmd}");
    for flag in LAUNCH_ARGS {
        assert!(cmd.contains(flag), "thieu {flag}: {cmd}");
    }
    assert!(cmd.contains("--lang=ja-JP"), "{cmd}");
    let profile = std::path::absolute(&opts.profile_dir).unwrap();
    assert!(
        cmd.contains(&format!("--user-data-dir={}", profile.display()))
            || cmd.contains(&format!("--user-data-dir=\"{}\"", profile.display())),
        "{cmd}"
    );
    d.close(s).await.unwrap();
}

/// `Win32_Process.CommandLine` cua `pid` qua PowerShell (khong co crate WMI; test-only).
async fn command_line(pid: u32) -> String {
    let out = tokio::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("(Get-CimInstance Win32_Process -Filter \"ProcessId={pid}\").CommandLine"),
        ])
        .output()
        .await
        .expect("powershell");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `profile.exit_type` trong `Default/Preferences` ("Normal" = thoat sach; "Crashed" = dang chay/bi kill).
fn exit_type(profile: &std::path::Path) -> String {
    let raw = std::fs::read_to_string(profile.join("Default").join("Preferences"))
        .expect("doc Preferences");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("Preferences JSON");
    v["profile"]["exit_type"].as_str().unwrap_or("").to_string()
}

/// Cong loopback chac chan KHONG ai nghe: bind `127.0.0.1:0`, doc cong, tha listener.
async fn closed_port() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    drop(l);
    port
}

/// `tasklist /FI "PID eq <pid>"` co dong chua pid → con song.
async fn pid_alive(pid: u32) -> bool {
    let out = tokio::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
        .await
        .expect("tasklist");
    String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
}
