//! Smoke test cho `lvc_browser::dola::refs` tren fixture cuc bo `tests/fixtures/refs.html`
//! (KHONG goi dola.com; profile tam; anh la PNG 1x1 that ghi vao thu muc tam). Tat ca `#[ignore]`
//! vi can Chromium: `cargo test -p lvc-browser --test refs_smoke -- --ignored`.
//!
//! Fixture gia lap upload cua Dola: `input[type=file]` (composer Nhanh) hoac o soan
//! `[contenteditable=true]` (Pro/Agent, `?noinput=1`) nhan `paste`; sau do POST
//! `/alice/resource/prepare_upload` mot lan + `/upload/v1/<n>` moi anh + them mot `<img>` moi anh
//! (co delay de `attach_reference_images` phai poll). Listener paste doc `clipboardData.files` HAI
//! lan va ghi ca hai do dai (chung minh Proxy doc-mot-lan ADR-014: lan 1 = N, lan 2 = 0) va
//! "handler thu 2" chi upload khi con thay file. Query string: xem dau file fixture.

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::Fx;
use lvc_browser::BrowserError;
use lvc_browser::dola::DolaError;
use lvc_browser::dola::refs::{
    AttachWay, attach_reference_images, attach_reference_images_with_floor, paste_image_once,
};
use lvc_browser::ops;

/// PNG 1x1 RGBA hop le (70 byte; CRC da kiem bang Python `zlib.crc32`).
const PNG_1X1: [u8; 70] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0xf8, 0x5f,
    0x0f, 0x00, 0x02, 0x87, 0x01, 0x80, 0xeb, 0x47, 0xba, 0x92, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// `REFERENCE_UPLOAD_TIMEOUT` mac dinh (config.py:287) — duong thanh cong khong bao gio cho het.
const TIMEOUT: Duration = Duration::from_secs(90);
/// Duong het gio: timeout 1 s (in trong thong diep) + san 2 s (thay cho 10 s) -> cho dung 2 s.
const SHORT: Duration = Duration::from_secs(1);
const FLOOR: Duration = Duration::from_secs(2);

/// Ghi `n` file PNG 1x1 (`anh_1.png`, `anh_2.png`...) vao thu muc tam.
fn pngs(dir: &tempfile::TempDir, n: usize) -> Vec<PathBuf> {
    (1..=n)
        .map(|i| {
            let p = dir.path().join(format!("anh_{i}.png"));
            std::fs::write(&p, PNG_1X1).unwrap();
            p
        })
        .collect()
}

async fn js<T: serde::de::DeserializeOwned>(fx: &Fx, expr: &str) -> T {
    ops::eval_json(fx.page(), expr).await.expect("eval")
}

/// So `<img>` trong `#thumbs` (khong tinh `#logo` co san).
async fn thumbs(fx: &Fx) -> i64 {
    js(fx, "document.querySelectorAll('#thumbs img').length").await
}

async fn paste_ready(fx: &Fx) -> bool {
    js(
        fx,
        "document.documentElement.dataset.dolaPasteReady === '1'",
    )
    .await
}

fn count(log: &str, needle: &str) -> usize {
    log.lines().filter(|l| l.contains(needle)).count()
}

fn ref_upload_msg(e: &DolaError) -> &str {
    match e {
        DolaError::RefUpload(m) => m,
        other => panic!("mong RefUpload, duoc {other:?}"),
    }
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn attach_2_anh_qua_input() {
    let fx = Fx::open("refs.html").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 2);
    let t0 = Instant::now();
    let out = attach_reference_images(fx.page(), &paths, TIMEOUT)
        .await
        .unwrap();
    let dt = t0.elapsed();
    assert_eq!(out.way, Some(AttachWay::Input));
    assert_eq!(out.expected, 2);
    assert_eq!(out.tos, 2);
    assert_eq!(out.thumbs, 2, "{out:?}");
    assert!(!out.extra_uploads);
    // :879 ngu 800 ms sau khi du dieu kien; fixture delay 300 ms x 3 buoc (= 1700 ms, chua ke CDP)
    assert!(dt >= Duration::from_millis(1600), "{dt:?}");
    let log = fx.log().await;
    assert_eq!(count(&log, "change files=2"), 1, "{log}");
    assert_eq!(count(&log, "prepare status=200"), 1, "{log}");
    assert_eq!(count(&log, "upload /upload/v1/"), 2, "{log}");
    assert!(
        log.contains("thumb anh_1.png") && log.contains("thumb anh_2.png"),
        "{log}"
    );
    assert_eq!(count(&log, "paste "), 0, "khong di duong dan: {log}");
    // Chrome doan MIME theo duoi khi set_input_files; kich thuoc = 70 byte PNG that
    let files: Vec<String> = js(
        &fx,
        "[...document.getElementById('file-input').files].map(f => f.name + ':' + f.type + ':' + f.size)",
    )
    .await;
    assert_eq!(
        files,
        vec!["anh_1.png:image/png:70", "anh_2.png:image/png:70"]
    );
    assert_eq!(thumbs(&fx).await, 2);
    assert!(
        !paste_ready(&fx).await,
        "khong cai listener dan khi co input"
    );
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn paste_khi_khong_co_input() {
    let fx = Fx::open("refs.html?noinput=1").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 2);
    let t0 = Instant::now();
    let out = attach_reference_images(fx.page(), &paths, TIMEOUT)
        .await
        .unwrap();
    let dt = t0.elapsed();
    assert_eq!(out.way, Some(AttachWay::Paste));
    assert_eq!(out.expected, 2);
    assert_eq!(out.tos, 2, "ADR-014: 1 anh = 1 upload — {out:?}");
    assert_eq!(out.thumbs, 2, "{out:?}");
    assert!(!out.extra_uploads);
    // :864 700 ms sau moi lan dan (x2) + :879 800 ms (= 2200 ms, chua ke CDP)
    assert!(dt >= Duration::from_millis(2100), "{dt:?}");
    let log = fx.log().await;
    // Proxy doc-mot-lan: lan 1 thay 1 file (ten/MIME/70 byte), lan 2 thay 0 -> "handler thu 2" khong upload
    assert_eq!(
        count(&log, "paste files=1 files2=0 anh_1.png:image/png:70"),
        1,
        "{log}"
    );
    assert_eq!(
        count(&log, "paste files=1 files2=0 anh_2.png:image/png:70"),
        1,
        "{log}"
    );
    assert_eq!(count(&log, "prepare status=200"), 2, "{log}");
    assert_eq!(count(&log, "upload /upload/v1/"), 2, "{log}");
    assert!(paste_ready(&fx).await, "listener main world da cai");
    let last: String = js(
        &fx,
        "document.documentElement.dataset.dolaPasteResult || ''",
    )
    .await;
    assert!(
        last.starts_with("main acc=") && last.contains("consumed=true") && last.contains("dp=true"),
        "{last}"
    );
    fx.close().await;
}

/// `?preready=1`: trang da dat `window.__dolaPasteReady` ma KHONG co listener -> script main world
/// thoat som, `data-dola-paste-ready` khong thanh '1' (state `blocked`) -> fallback `PASTE_IMAGE_JS`
/// (`:782-786`): khong co Proxy -> ca 2 lan doc deu thay file -> 2 upload/anh (dinh thua, nhu
/// Python ghi chu).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn paste_fallback_khi_main_world_bi_chan() {
    let fx = Fx::open("refs.html?noinput=1&preready=1&delay=50").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 1);
    let r = paste_image_once(fx.page(), &paths[0]).await.unwrap();
    assert_eq!(r, "fallback");
    assert!(!paste_ready(&fx).await);
    let log = fx.log().await;
    assert_eq!(
        count(&log, "paste files=1 files2=1 anh_1.png:image/png:70"),
        1,
        "{log}"
    );
    // Toan bo luong: 1 anh -> tos=2 > expected=1 -> extra_uploads (canh bao, khong loi)
    let fx2 = Fx::open("refs.html?noinput=1&preready=1&delay=50").await;
    let out = attach_reference_images(fx2.page(), &paths, TIMEOUT)
        .await
        .unwrap();
    assert_eq!(out.way, Some(AttachWay::Paste));
    assert_eq!(out.tos, 2, "{out:?}");
    assert!(out.extra_uploads, "{out:?}");
    assert!(out.thumbs >= 1, "{out:?}");
    fx2.close().await;
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn paste_nobox_bao_ref_upload() {
    let fx = Fx::open("refs.html?noinput=1&nobox=1").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 2);
    let t0 = Instant::now();
    let err = attach_reference_images(fx.page(), &paths, TIMEOUT)
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "REF_UPLOAD: paste failed: nobox");
    assert!(t0.elapsed() < Duration::from_secs(5), "khong cho het gio");
    // Duong fallback khi khong co o soan: `:783`
    let fx2 = Fx::open("refs.html?noinput=1&nobox=1&preready=1").await;
    let err = paste_image_once(fx2.page(), &paths[0]).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        "REF_UPLOAD: no composer (textarea/contenteditable) to paste into"
    );
    fx2.close().await;
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn het_gio_khi_khong_upload() {
    let fx = Fx::open("refs.html?slow=1").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 2);
    let t0 = Instant::now();
    let err = attach_reference_images_with_floor(fx.page(), &paths, SHORT, FLOOR)
        .await
        .unwrap_err();
    let dt = t0.elapsed();
    assert_eq!(
        ref_upload_msg(&err),
        "attached 0/2 via input within 1s (prepare=0/2, tos=0/2, thumbs=+0)"
    );
    assert_eq!(
        err.to_string(),
        "REF_UPLOAD: attached 0/2 via input within 1s (prepare=0/2, tos=0/2, thumbs=+0)"
    );
    // deadline = max(1 s, 2 s) = 2 s; raise_if_logged_out them <1 s
    assert!(
        dt >= Duration::from_secs(2) && dt < Duration::from_secs(8),
        "{dt:?}"
    );
    let log = fx.log().await;
    assert_eq!(count(&log, "slow: khong upload"), 1, "{log}");
    fx.close().await;

    // Duong dan: `via paste`, dan xong (main world tra loi) nhung fixture khong upload
    let fx = Fx::open("refs.html?noinput=1&slow=1").await;
    let err = attach_reference_images_with_floor(fx.page(), &paths[..1], SHORT, FLOOR)
        .await
        .unwrap_err();
    assert_eq!(
        ref_upload_msg(&err),
        "attached 0/1 via paste within 1s (prepare=0/1, tos=0/1, thumbs=+0)"
    );
    assert!(fx.log().await.contains("paste files=1 files2=0"));
    fx.close().await;
}

/// `?fail=1`: upload tra 500 -> `count_2xx` KHONG dem (tos=0) du thumbnail van tang -> het gio,
/// `attached = min(thumbs, expected)`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn upload_500_khong_dem() {
    let fx = Fx::open("refs.html?fail=1&delay=50").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 2);
    let err = attach_reference_images_with_floor(fx.page(), &paths, SHORT, FLOOR)
        .await
        .unwrap_err();
    assert_eq!(
        ref_upload_msg(&err),
        "attached 2/2 via input within 1s (prepare=1/2, tos=0/2, thumbs=+2)"
    );
    let log = fx.log().await;
    assert_eq!(count(&log, "upload /upload/v1/loi status=500"), 2, "{log}");
    fx.close().await;
}

/// `?dup=1`: moi anh 2 upload (Dola gop/lap) -> dieu kien van dat, `tos > expected` -> canh bao
/// "dinh thua" (`:881`), khong loi.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn dup_upload_bao_dinh_thua() {
    let fx = Fx::open("refs.html?dup=1&delay=50").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 2);
    let out = attach_reference_images(fx.page(), &paths, TIMEOUT)
        .await
        .unwrap();
    assert_eq!(out.way, Some(AttachWay::Input));
    assert_eq!(out.tos, 4, "{out:?}");
    assert!(out.extra_uploads, "{out:?}");
    assert_eq!(out.thumbs, 2, "{out:?}");
    fx.close().await;
}

/// `#logo` co san: `thumbs` = tang them so voi luc bat dau (`thumbs0`), khong phai tong `<img>`.
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn thumbs_tru_so_img_co_san() {
    let fx = Fx::open("refs.html?logos=3&delay=50").await;
    let n0: i64 = js(&fx, "document.querySelectorAll('img').length").await;
    assert_eq!(n0, 3);
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 1);
    let out = attach_reference_images(fx.page(), &paths, TIMEOUT)
        .await
        .unwrap();
    assert_eq!(out.thumbs, 1, "{out:?}");
    let n1: i64 = js(&fx, "document.querySelectorAll('img').length").await;
    assert_eq!(n1, 4);
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn rong_khong_lam_gi() {
    let fx = Fx::open("refs.html").await;
    let t0 = Instant::now();
    let out = attach_reference_images(fx.page(), &[], TIMEOUT)
        .await
        .unwrap();
    assert_eq!(out.way, None);
    assert_eq!(out.expected, 0);
    assert_eq!(out.tos, 0);
    assert!(!out.extra_uploads);
    assert!(t0.elapsed() < Duration::from_millis(500));
    assert_eq!(fx.log().await, "", "khong dung toi trang");
    fx.close().await;
}

#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn file_khong_ton_tai_bao_io() {
    let dir = tempfile::tempdir().unwrap();
    let missing = vec![dir.path().join("khong_co.png")];
    // input: set_file_input kiem truoc khi goi CDP
    let fx = Fx::open("refs.html").await;
    let err = attach_reference_images(fx.page(), &missing, TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, DolaError::Browser(BrowserError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound),
        "{err:?}"
    );
    fx.close().await;
    // paste: tokio::fs::read
    let fx = Fx::open("refs.html?noinput=1").await;
    let err = attach_reference_images(fx.page(), &missing, TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, DolaError::Browser(BrowserError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound),
        "{err:?}"
    );
    assert!(err.to_string().contains("khong_co.png"), "{err}");
    fx.close().await;
}

/// `paste_image_once` goi 2 lan: lan 2 khong cai lai listener (idempotent), moi lan 1 ket qua
/// `main ...` moi (`PASTE_TRIGGER_JS` xoa ket qua cu truoc khi postMessage).
#[tokio::test]
#[ignore = "can Chromium tren may; chay: cargo test -p lvc-browser --test refs_smoke -- --ignored"]
async fn paste_image_once_idempotent() {
    let fx = Fx::open("refs.html?noinput=1&slow=1").await;
    let dir = tempfile::tempdir().unwrap();
    let paths = pngs(&dir, 2);
    let r1 = paste_image_once(fx.page(), &paths[0]).await.unwrap();
    assert!(r1.starts_with("main acc="), "{r1}");
    assert!(r1.contains("consumed=true"), "{r1}");
    let installs: i64 = js(&fx, "window.__lvc_msg_listeners || 0").await;
    let r2 = paste_image_once(fx.page(), &paths[1]).await.unwrap();
    assert!(r2.starts_with("main acc="), "{r2}");
    let installs2: i64 = js(&fx, "window.__lvc_msg_listeners || 0").await;
    assert_eq!(installs, 1, "cai dung 1 listener message");
    assert_eq!(installs2, 1, "lan 2 khong cai them");
    let log = fx.log().await;
    assert_eq!(count(&log, "paste files=1 files2=0"), 2, "{log}");
    fx.close().await;
}
