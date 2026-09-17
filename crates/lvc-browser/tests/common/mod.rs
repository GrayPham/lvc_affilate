//! Helper dung chung cho smoke test (`ops_smoke.rs`, `session_smoke.rs`): server HTTP ti hon phuc
//! vu `tests/fixtures/` tren `127.0.0.1:0` + mo Chromium headless voi profile tam. KHONG bao gio
//! goi dola.com.
//!
//! Vi sao HTTP chu khong `file://` (do 2026-09-18 tren Chromium 1234): `fetch()` tu origin `file://`
//! bi CORS chan ("Failed to fetch") va `Network.responseReceived` KHONG bao gio ban, nen
//! `count_responses`/`RECENT_IDS_JS` khong test duoc; dung mot transport cho ca bo (gan Dola https hon).
#![allow(dead_code)] // moi file test chi dung mot phan helper

use std::path::PathBuf;

use lvc_browser::chromium::{ChromiumDriver, ChromiumSession};
use lvc_browser::{BrowserDriver, LaunchOpts, ops};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// `LVC_CHROME` (tuy chon) ghi de binary; mac dinh resolver chon Chromium Playwright rev cao nhat.
pub fn opts_headless(dir: &tempfile::TempDir) -> LaunchOpts {
    LaunchOpts {
        profile_dir: dir.path().join("p"),
        headless: true,
        executable_override: std::env::var_os("LVC_CHROME").map(PathBuf::from),
        ..LaunchOpts::default()
    }
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Body gia lap `POST /im/chain/recent_conv` cua Dola (xem `RECENT_IDS_JS`): 3 cell — id qua
/// `conversation.conversation_id`, id qua `id`, va mot id KHONG phai so (phai bi loc).
pub const RECENT_CONV_BODY: &str = r#"{"downlink_body":{"pull_recent_conv_chain_downlink_body":{"cells":[{"conversation":{"conversation_id":"3841796000000000689"}},{"id":12345},{"id":"abc"}]}}}"#;

/// Tra (status, content-type, body) cho duong dan HTTP: endpoint gia lap `upload`/`recent_conv` cua
/// Dola tra 200 JSON, `/upload/v1/loi` tra 500 (de chac `count_2xx` KHONG dem non-2xx), con lai doc
/// file trong `tests/fixtures/` (cam `..`).
pub fn route(path: &str) -> (&'static str, &'static str, Vec<u8>) {
    match path {
        "/alice/resource/prepare_upload" | "/upload/v1/x" => {
            ("200 OK", "application/json", b"{\"ok\":true}".to_vec())
        }
        "/im/chain/recent_conv" => (
            "200 OK",
            "application/json",
            RECENT_CONV_BODY.as_bytes().to_vec(),
        ),
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
/// tra `http://127.0.0.1:<port>` + task accept (abort trong `Fx::close`). Doc toi het header
/// (`\r\n\r\n`); body POST (neu co) bi bo qua — du cho fixture.
pub async fn serve_fixtures() -> (String, tokio::task::JoinHandle<()>) {
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

/// Chromium headless + server fixture + trang da mo `<base>/<path>`.
pub struct Fx {
    pub d: ChromiumDriver,
    pub s: ChromiumSession,
    pub base: String,
    server: tokio::task::JoinHandle<()>,
    _dir: tempfile::TempDir,
}

impl Fx {
    /// `path` vd `"composer.html"` hoac `"chat.html?login=1"`.
    pub async fn open(path: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (base, server) = serve_fixtures().await;
        let d = ChromiumDriver;
        let s = d.launch(&opts_headless(&dir)).await.expect("launch");
        d.goto(&s, &format!("{base}/{path}"))
            .await
            .expect("goto fixture");
        Self {
            d,
            s,
            base,
            server,
            _dir: dir,
        }
    }

    pub fn page(&self) -> &chromiumoxide::Page {
        self.s.page()
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.base)
    }

    /// Noi dung `#log` cua fixture (moi su kien duoc ghi vao day).
    pub async fn log(&self) -> String {
        ops::eval_json(self.page(), "document.getElementById('log').textContent")
            .await
            .expect("doc #log")
    }

    /// Gia tri `#prompt` (textarea) cua fixture.
    pub async fn textarea(&self) -> String {
        ops::eval_json(self.page(), "document.getElementById('prompt').value")
            .await
            .expect("doc textarea")
    }

    pub async fn close(self) {
        self.d.close(self.s).await.expect("close");
        self.server.abort();
    }
}
