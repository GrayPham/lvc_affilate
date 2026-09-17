//! Helper dung chung cho smoke test (`ops_smoke.rs`, `session_smoke.rs`, `compose_smoke.rs`,
//! `refs_smoke.rs`, `submit_smoke.rs`): server HTTP ti hon phuc vu `tests/fixtures/` tren
//! `127.0.0.1:0` + mo Chromium headless voi profile tam. KHONG bao gio goi dola.com.
//!
//! Vi sao HTTP chu khong `file://` (do 2026-09-18 tren Chromium 1234): `fetch()` tu origin `file://`
//! bi CORS chan ("Failed to fetch") va `Network.responseReceived` KHONG bao gio ban, nen
//! `count_responses`/`RECENT_IDS_JS` khong test duoc; dung mot transport cho ca bo (gan Dola https hon).
#![allow(dead_code)] // moi file test chi dung mot phan helper

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Task 9: tu lan goi `recent_conv` THU HAI tro di (cung mot server) — co them chat MOI
/// `777000000000000001` dung dau (moi nhat truoc), de test nhanh "URL roi vao chat cu ->
/// `recent_conv_ids` tim chat moi" cua `after_submit` (`video_worker_ui.py:2578-2587`).
pub const RECENT_CONV_BODY_LATER: &str = r#"{"downlink_body":{"pull_recent_conv_chain_downlink_body":{"cells":[{"conversation":{"conversation_id":"777000000000000001"}},{"conversation":{"conversation_id":"3841796000000000689"}},{"id":12345}]}}}"#;

/// Id MOI ma `RECENT_CONV_BODY_LATER` them vao.
pub const RECENT_CONV_NEW_ID: &str = "777000000000000001";

/// Tra (status, content-type, body) cho duong dan HTTP: endpoint gia lap `upload`/`recent_conv` cua
/// Dola tra 200 JSON (`recent_call` = so lan `/im/chain/recent_conv` DA duoc goi truoc do tren server
/// nay: 0 -> `RECENT_CONV_BODY`, >= 1 -> `RECENT_CONV_BODY_LATER`), `/upload/v1/loi` tra 500 (de
/// chac `count_2xx` KHONG dem non-2xx), `/chat` va `/chat/<so>` -> `chat_submit.html` (URL hinh dang
/// Dola cho `dola::submit`), con lai doc file trong `tests/fixtures/` (cam `..`).
pub fn route(path: &str, recent_call: usize) -> (&'static str, &'static str, Vec<u8>) {
    match path {
        "/alice/resource/prepare_upload" | "/upload/v1/x" => {
            ("200 OK", "application/json", b"{\"ok\":true}".to_vec())
        }
        "/im/chain/recent_conv" => (
            "200 OK",
            "application/json",
            if recent_call == 0 {
                RECENT_CONV_BODY
            } else {
                RECENT_CONV_BODY_LATER
            }
            .as_bytes()
            .to_vec(),
        ),
        // Task 9: trang chat gia lap o /chat va /chat/<id> (pushState/goto sau khi gui).
        p if p == "/chat"
            || p.strip_prefix("/chat/")
                .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())) =>
        {
            match std::fs::read(fixtures_dir().join("chat_submit.html")) {
                Ok(b) => ("200 OK", "text/html; charset=utf-8", b),
                Err(_) => ("404 Not Found", "text/plain", b"khong co".to_vec()),
            }
        }
        // Task 7: fixture compose.html gui /chat/completion (qua hook duration) - chi can 200.
        "/chat/completion" => ("200 OK", "application/json", b"{}".to_vec()),
        "/upload/v1/loi" => (
            "500 Internal Server Error",
            "application/json",
            b"{}".to_vec(),
        ),
        // Task 8: fixture refs.html goi /upload/v1/<n> mot lan cho moi anh (n tang dan).
        p if p.starts_with("/upload/v1/") => {
            ("200 OK", "application/json", b"{\"ok\":true}".to_vec())
        }
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
    let recent_calls = Arc::new(AtomicUsize::new(0));
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let recent_calls = Arc::clone(&recent_calls);
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
                let recent_call = if path == "/im/chain/recent_conv" {
                    recent_calls.fetch_add(1, Ordering::SeqCst)
                } else {
                    0
                };
                let (status, ctype, body) = route(&path, recent_call);
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
    /// `path` vd `"composer.html"`, `"chat.html?login=1"` hoac `"chat?captcha=1"` (route `/chat`).
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
