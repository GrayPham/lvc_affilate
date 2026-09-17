//! Anh tham chieu (Task 8, P2) — port `attach_reference_images` / `paste_image_once` cua
//! `video_worker_ui.py` (HEAD 3d07c19):
//!
//! | Rust | Python |
//! |---|---|
//! | [`mime_by_suffix`] | `MIME_BY_SUFFIX` `:709` (+ `.get(suffix.lower(), ...)` `:770`) |
//! | [`paste_image_once`] | `paste_image_once` `:767-787` |
//! | [`attach_reference_images`] / [`attach_reference_images_with_floor`] | `attach_reference_images` `:836-892` |
//! | [`AttachWay`] | `way = "input"` `:857` / `"paste"` `:859` |
//! | [`AttachOutcome`] | dong `print` `:882` (`tos`, `thumbs +N`, canh bao dinh thua) |
//! | [`upload_failed_msg`] | `ReferenceUploadError(...)` `:888-890` |
//!
//! # Composer Nhanh vs Pro/Agent
//! Composer Nhanh co `input[type=file]` -> [`ops::set_file_input`] TAT CA file mot lan (`:856`,
//! `set_input_files`). Pro/Agent (プロ / Pro Max) khong co input -> dan tung file qua
//! `ClipboardEvent` (`:861`, 700 ms giua 2 lan).
//!
//! # MAIN world (khac Python — ruling Task 8 #2)
//! Python chay o isolated world (patchright) nen phai tiem `<script>` (`PASTE_INJECT_JS`) de
//! `PASTE_MAIN_WORLD_SCRIPT` chay o main world. chromiumoxide `Runtime.evaluate` DA o main world
//! -> [`paste_image_once`] `eval` thang `PASTE_MAIN_WORLD_SCRIPT` (idempotent qua
//! `window.__dolaPasteReady` + `data-dola-paste-ready`), KHONG dung the `<script>` (CSP khong ap
//! len `DevTools`). Proxy `clipboardData` doc-mot-lan (ADR-014) giu nguyen van trong `labels.rs` (so
//! byte voi Python). Neu sau khi eval ma `data-dola-paste-ready != '1'` (state `blocked`, vd trang
//! da dat `window.__dolaPasteReady` ma khong co listener) -> roi ve `PASTE_IMAGE_JS` nhu Python
//! `:783-785` (co the dinh 2 lan).
//!
//! # Hoan tat upload
//! Bo dem `Network.responseReceived` ([`ops::count_responses`]) dang ky TRUOC moi hanh dong (Python
//! `page.on("response")` `:851`); xong khi `prepare(/alice/resource/prepare_upload 2xx) >= 1`
//! (Dot 6 BUG-001: Dola gop prepare theo lo) `&& tos(/upload/v1/ 2xx) >= N && thumbs(<img>) >= N`
//! (`:876-878`), roi ngu 800 ms va dem lai `tos` (`:879-880`, canh bao "dinh thua" khi `tos > N`).
//! Het `max(floor, timeout)` (`:874`, floor 10 s) -> [`session::raise_if_logged_out`] ->
//! [`DolaError::RefUpload`] voi thong diep y nguyen Python.
//!
//! Gioi han so luong/kich thuoc anh KHONG kiem o day: Python kiem `REFERENCE_IMAGE_MAX_COUNT` o bien
//! API (`server.py:1867`, `media.py:184`, `scenes.py:89`) — Rust: `lvc_core::Limits::
//! reference_image_max_count` trong `scene.rs`.
//!
//! Log qua `tracing`; KHONG BAO GIO log noi dung file/base64 (chi duong dan + so byte).

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use chromiumoxide::Page;
use serde::Serialize;

use super::error::DolaError;
use super::labels::{
    COUNT_IMG_JS, PASTE_IMAGE_JS, PASTE_MAIN_WORLD_SCRIPT, PASTE_RESULT_JS, PASTE_TRIGGER_JS,
};
use super::session;
use crate::BrowserError;
use crate::ops;

// ---------------------------------------------------------------------------------------------
// Hang — moi so la literal cua Python (dong ghi ben canh)
// ---------------------------------------------------------------------------------------------

/// Chuoi con URL cua `prepare_upload` (`:847`, `:850`).
pub const PREPARE_UPLOAD_PART: &str = "/alice/resource/prepare_upload";
/// Chuoi con URL cua upload TOS (`:847`, `:851`).
pub const UPLOAD_PART: &str = "/upload/v1/";
/// `page.locator('input[type="file"]').first` `:854-856`.
const FILE_INPUT_SEL: &str = r#"input[type="file"]"#;
/// `!!document.querySelector('input[type="file"]')` — `.first.count() > 0` `:854`.
const HAS_FILE_INPUT_JS: &str = r#"!!document.querySelector('input[type="file"]')"#;
/// `data-dola-paste-ready === '1'` — `PASTE_INJECT_JS` `:755`.
const PASTE_READY_JS: &str = "document.documentElement.dataset.dolaPasteReady === '1'";
/// `wait_for_timeout(700)` sau moi lan dan `:864`.
const PASTE_GAP: Duration = Duration::from_millis(700);
/// `max(10, int(config.REFERENCE_UPLOAD_TIMEOUT))` `:874` — san 10 s.
const DEADLINE_FLOOR: Duration = Duration::from_secs(10);
/// `wait_for_timeout(800)` sau khi du dieu kien `:879`.
const SETTLE_OK: Duration = Duration::from_millis(800);
/// `wait_for_timeout(250)` moi vong `:884`.
const POLL: Duration = Duration::from_millis(250);
/// `for _ in range(20)` `:774` x `wait_for_timeout(100)` `:780` doc `PASTE_RESULT_JS`.
const PASTE_RESULT_POLLS: u32 = 20;
const PASTE_RESULT_POLL: Duration = Duration::from_millis(100);

// ---------------------------------------------------------------------------------------------
// Kieu ket qua
// ---------------------------------------------------------------------------------------------

/// Duong dinh anh: `input[type=file]` (composer Nhanh, `:856`) hay dan `ClipboardEvent`
/// (Pro/Agent, `:859-864`). `Display` = chuoi `way` Python (`"input"`/`"paste"`) dung trong
/// thong diep loi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachWay {
    Input,
    Paste,
}

impl fmt::Display for AttachWay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Input => "input",
            Self::Paste => "paste",
        })
    }
}

/// Ket qua thanh cong cua [`attach_reference_images`] — cac so Python in o `:882` de Task 9 log
/// (ruling Task 8 #4). `way == None` <=> `expected == 0` (Python `return 0` `:846` TRUOC khi chon
/// duong `:855`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachOutcome {
    pub way: Option<AttachWay>,
    /// So anh yeu cau = `len(image_paths)` (Python tra `expected` `:883`).
    pub expected: usize,
    /// So phan hoi 2xx `/upload/v1/` sau khi ngu 800 ms (`:880`).
    pub tos: usize,
    /// So `<img>` tang them so voi truoc khi dinh (`:875`).
    pub thumbs: usize,
    /// `tos > expected` (`:881`): Dola nhan nhieu upload hon so anh ("dinh thua") — canh bao, khong loi.
    pub extra_uploads: bool,
}

impl AttachOutcome {
    /// `if not image_paths: return 0` `:845-846`.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            way: None,
            expected: 0,
            tos: 0,
            thumbs: 0,
            extra_uploads: false,
        }
    }
}

/// Tham so cho `PASTE_TRIGGER_JS`/`PASTE_IMAGE_JS` — dict `arg` `:770-771`
/// (`{"b64", "name", "mime"}`). `Debug` CHE `b64` (chi in do dai).
#[derive(Clone, Serialize)]
struct PasteArg {
    b64: String,
    name: String,
    mime: &'static str,
}

impl PasteArg {
    /// `base64.b64encode(p.read_bytes())`, `p.name`, `MIME_BY_SUFFIX.get(p.suffix.lower(), ...)`.
    /// Khong co ten file (`..`/goc) -> `""` (Python `Path("..").name == ".."`, nhung doc bytes
    /// cua thu muc da loi truoc do).
    fn new(path: &Path, bytes: &[u8]) -> Self {
        Self {
            b64: B64.encode(bytes),
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            mime: mime_by_suffix(path),
        }
    }
}

impl fmt::Debug for PasteArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasteArg")
            .field("b64_len", &self.b64.len())
            .field("name", &self.name)
            .field("mime", &self.mime)
            .finish()
    }
}

/// Phan loai chuoi `PASTE_RESULT_JS` (`:775-779`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PasteResult {
    /// Rong — main world chua tra loi, doc tiep.
    Pending,
    /// `main ...` / `nobox` / `error...` — main world da tra loi, tra ngay (ke ca loi).
    Final,
    /// Chuoi khac — Python `break` -> fallback.
    Other,
}

// ---------------------------------------------------------------------------------------------
// Ham thuan
// ---------------------------------------------------------------------------------------------

/// `MIME_BY_SUFFIX.get(p.suffix.lower(), default)` `:709`, `:770`: `.jpg`/`.jpeg` -> `image/jpeg`,
/// `.png` -> `image/png`, `.webp` -> `image/webp`; con lai `application/octet-stream` (ruling Task 8
/// #3 — Python mac dinh `image/png`). `Path::extension` == `Path.suffix` khong dau cham (`a.` ->
/// `""`, `.png` -> khong co, `a.tar.gz` -> `gz`).
#[must_use]
pub fn mime_by_suffix(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

/// `:775-779`: `r.startswith("main ") or r == "nobox" or r.startswith("error")` -> `Final`.
fn classify_paste_result(r: &str) -> PasteResult {
    if r.is_empty() {
        PasteResult::Pending
    } else if r.starts_with("main ") || r == "nobox" || r.starts_with("error") {
        PasteResult::Final
    } else {
        PasteResult::Other
    }
}

/// `:862` `r == "nobox" or r.startswith("error")`.
fn paste_failed(r: &str) -> bool {
    r == "nobox" || r.starts_with("error")
}

/// `:876` `prepare >= 1 and tos >= expected and thumbs >= expected`.
fn uploads_done(expected: usize, prepare: usize, tos: usize, thumbs: i64) -> bool {
    prepare >= 1 && tos >= expected && thumbs >= i64::try_from(expected).unwrap_or(i64::MAX)
}

/// `:889` `min(thumbs, expected) if thumbs > 0 else 0`.
fn attached_count(thumbs: i64, expected: usize) -> usize {
    usize::try_from(thumbs).map_or(0, |t| t.min(expected))
}

/// Thong diep `ReferenceUploadError` `:888-890` (chua tien to `REF_UPLOAD: ` — [`DolaError::RefUpload`]
/// them): `attached {k}/{N} via {way} within {int(T)}s (prepare={p}/{N}, tos={t}/{N}, thumbs=+{h})`.
/// `T` = timeout CAU HINH (khong phai san 10 s), cat phan le giay nhu `int()`; `h` co the am
/// (Python in `+-1`).
#[must_use]
pub fn upload_failed_msg(
    expected: usize,
    way: AttachWay,
    timeout: Duration,
    prepare: usize,
    tos: usize,
    thumbs: i64,
) -> String {
    let attached = attached_count(thumbs, expected);
    let secs = timeout.as_secs();
    format!(
        "attached {attached}/{expected} via {way} within {secs}s \
         (prepare={prepare}/{expected}, tos={tos}/{expected}, thumbs=+{thumbs})"
    )
}

// ---------------------------------------------------------------------------------------------
// CDP
// ---------------------------------------------------------------------------------------------

/// `int(await page.evaluate(COUNT_IMG_JS) or 0)` `:853`, `:875`.
async fn count_img(page: &Page) -> Result<i64, BrowserError> {
    let v: Option<i64> = ops::eval_fn(page, COUNT_IMG_JS).await?;
    Ok(v.unwrap_or(0))
}

/// Cai listener main world (thay `PASTE_INJECT_JS` `:754-758`): da `ready` thi thoi; chua thi
/// `eval` `PASTE_MAIN_WORLD_SCRIPT` thang (MAIN world) roi kiem lai `data-dola-paste-ready`.
/// Tra `"ready"`/`"blocked"` nhu Python `state`. Ngoai le JS khi cai -> `blocked` (warn), loi CDP
/// -> `Err`.
async fn install_paste_listener(page: &Page) -> Result<&'static str, BrowserError> {
    if ops::eval_json::<bool>(page, PASTE_READY_JS).await? {
        return Ok("ready");
    }
    match ops::eval_json::<serde_json::Value>(page, PASTE_MAIN_WORLD_SCRIPT).await {
        Ok(_) => {}
        Err(BrowserError::Js(e)) => {
            tracing::warn!("khong cai duoc listener dan anh (main world): {e}");
            return Ok("blocked");
        }
        Err(e) => return Err(e),
    }
    Ok(if ops::eval_json::<bool>(page, PASTE_READY_JS).await? {
        "ready"
    } else {
        "blocked"
    })
}

/// Port `paste_image_once` `:767-787`: dan MOT anh vao o soan sao cho chi 1 upload (ADR-014).
/// Doc file (`tokio::fs::read`), base64, `name`/`mime` -> cai listener main world -> `PASTE_TRIGGER_JS`
/// -> doc `PASTE_RESULT_JS` 20 x 100 ms. Tra chuoi ket qua Python (brief ghi `()` — giu chuoi de
/// log `acc=/consumed=`): bat dau `main ` khi di duong main world (ke ca `nobox`/`error...` —
/// caller quyet, `:862`), `"fallback"` khi phai dung `PASTE_IMAGE_JS` (co the dinh 2 lan, warn).
///
/// # Errors
/// `Browser(Io)` khi khong doc duoc file; `RefUpload("no composer (textarea/contenteditable) to
/// paste into")` khi fallback khong thay o soan (`:783`); loi CDP/JS khac.
pub async fn paste_image_once(page: &Page, path: &Path) -> Result<String, DolaError> {
    let bytes = tokio::fs::read(path).await.map_err(|e| {
        BrowserError::Io(std::io::Error::new(
            e.kind(),
            format!("khong doc duoc anh tham chieu {}: {e}", path.display()),
        ))
    })?;
    tracing::debug!(path = %path.display(), bytes = bytes.len(), "dan anh tham chieu");
    let arg = PasteArg::new(path, &bytes);
    drop(bytes);
    let state = install_paste_listener(page).await?;
    if state == "ready" {
        ops::eval_with_arg::<serde_json::Value, _>(page, PASTE_TRIGGER_JS, &arg).await?;
        for _ in 0..PASTE_RESULT_POLLS {
            let r: String = ops::eval_fn(page, PASTE_RESULT_JS).await?;
            match classify_paste_result(&r) {
                PasteResult::Final => return Ok(r),
                PasteResult::Other => break,
                PasteResult::Pending => tokio::time::sleep(PASTE_RESULT_POLL).await,
            }
        }
    }
    // :782-786 — duong cu (Python: isolated world; o day van MAIN world nhung khong co Proxy doc-mot-lan)
    let ok: bool = ops::eval_with_arg(page, PASTE_IMAGE_JS, &arg).await?;
    if !ok {
        return Err(DolaError::RefUpload(
            "no composer (textarea/contenteditable) to paste into".into(),
        ));
    }
    tracing::warn!(
        state,
        "paste fallback (PASTE_IMAGE_JS, khong co Proxy doc-mot-lan) — Dola co the dinh 2 lan"
    );
    Ok("fallback".into())
}

/// Port `attach_reference_images` `:836-892` voi san deadline 10 s (`:874`). Xem doc module.
///
/// # Errors
/// - `RefUpload("paste failed: nobox"|"paste failed: error ...")` (`:863`);
/// - het gio: `LoggedOut { revoked: true }` neu Dola tu dang xuat (`:887`), khong thi
///   `RefUpload(`[`upload_failed_msg`]`)` (`:888`);
/// - `Browser(...)`: file khong ton tai (`set_file_input`/doc file), loi CDP/JS.
pub async fn attach_reference_images(
    page: &Page,
    paths: &[PathBuf],
    timeout: Duration,
) -> Result<AttachOutcome, DolaError> {
    attach_reference_images_with_floor(page, paths, timeout, DEADLINE_FLOOR).await
}

/// Nhu [`attach_reference_images`] nhung san deadline `floor` tuy y (`max(floor, timeout)`) — cho
/// test duong het gio khong phai cho 10 s. `timeout` van la so in trong thong diep loi (`within Ts`).
///
/// # Errors
/// Nhu [`attach_reference_images`].
pub async fn attach_reference_images_with_floor(
    page: &Page,
    paths: &[PathBuf],
    timeout: Duration,
    floor: Duration,
) -> Result<AttachOutcome, DolaError> {
    if paths.is_empty() {
        return Ok(AttachOutcome::empty());
    }
    let expected = paths.len();
    // :851 — dang ky nghe response TRUOC moi hanh dong; drop -> remove_listener (:892)
    let counter = ops::count_responses(page).await?;
    let thumbs0 = count_img(page).await?;
    let has_input: bool = ops::eval_json(page, HAS_FILE_INPUT_JS).await?;
    let way = if has_input {
        // :856 — set_input_files tat ca mot lan
        ops::set_file_input(page, FILE_INPUT_SEL, paths).await?;
        AttachWay::Input
    } else {
        // :859-864 — dan tung anh, 700 ms giua 2 lan
        for p in paths {
            let r = paste_image_once(page, p).await?;
            if paste_failed(&r) {
                return Err(DolaError::RefUpload(format!("paste failed: {r}")));
            }
            tracing::debug!(path = %p.display(), result = %r, "da dan");
            tokio::time::sleep(PASTE_GAP).await;
        }
        AttachWay::Paste
    };
    tracing::info!(expected, %way, thumbs0, "dinh anh tham chieu — cho upload");
    // :874 — max(10, T)
    let deadline = tokio::time::Instant::now() + timeout.max(floor);
    let (mut prepare, mut tos, mut thumbs) = (0_usize, 0_usize, 0_i64);
    while tokio::time::Instant::now() < deadline {
        prepare = counter.count_2xx(PREPARE_UPLOAD_PART);
        tos = counter.count_2xx(UPLOAD_PART);
        thumbs = count_img(page).await? - thumbs0;
        if uploads_done(expected, prepare, tos, thumbs) {
            tokio::time::sleep(SETTLE_OK).await;
            // :880 — dem lai sau 800 ms (upload thua den muon)
            tos = counter.count_2xx(UPLOAD_PART);
            let extra_uploads = tos > expected;
            if extra_uploads {
                tracing::warn!(
                    tos,
                    expected,
                    "Dola nhận {tos} upload cho {expected} ảnh (đính thừa)"
                );
            }
            tracing::info!(expected, %way, tos, thumbs, "Reference images uploaded");
            return Ok(AttachOutcome {
                way: Some(way),
                expected,
                tos,
                thumbs: usize::try_from(thumbs).unwrap_or(0),
                extra_uploads,
            });
        }
        tokio::time::sleep(POLL).await;
    }
    // :887 — dung ma: Dola tu dang xuat giua chung -> LOGGED_OUT (revoked) thay vi REF_UPLOAD
    session::raise_if_logged_out(page, "", "khi upload ảnh tham chiếu").await?;
    Err(DolaError::RefUpload(upload_failed_msg(
        expected, way, timeout, prepare, tos, thumbs,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `video_worker_ui.py:709` `MIME_BY_SUFFIX` (4 duoi, `suffix.lower()`); ngoai bang ->
    /// `application/octet-stream` (ruling Task 8 #3; Python `:770` mac dinh `image/png`). Oracle
    /// Python 2026-09-18: `Path(n).suffix.lower()` cho tung ten ben duoi.
    #[test]
    fn mime_by_suffix_theo_python() {
        let cases = [
            ("a.jpg", "image/jpeg"),
            ("a.JPG", "image/jpeg"),
            ("a.jpeg", "image/jpeg"),
            ("a.png", "image/png"),
            ("a.PnG", "image/png"),
            ("a.webp", "image/webp"),
            ("a.gif", "application/octet-stream"),
            ("a", "application/octet-stream"),
            ("a.", "application/octet-stream"),
            (".png", "application/octet-stream"),
            ("a.tar.gz", "application/octet-stream"),
            ("a.png.bak", "application/octet-stream"),
            (r"C:\anh\b.WEBP", "image/webp"),
        ];
        for (name, want) in cases {
            assert_eq!(mime_by_suffix(Path::new(name)), want, "{name}");
        }
    }

    /// `:888-890` `ReferenceUploadError(f"attached {min(thumbs, expected) if thumbs > 0 else 0}/{expected}
    /// via {way} within {int(T)}s (prepare={p}/{N}, tos={t}/{N}, thumbs=+{h})")` — oracle Python
    /// 2026-09-18 (ke ca `thumbs=+-1` khi am va `min(5, 2)`).
    #[test]
    fn thong_diep_that_bai_theo_python() {
        let t = Duration::from_secs(90);
        assert_eq!(
            upload_failed_msg(2, AttachWay::Input, t, 0, 0, 0),
            "attached 0/2 via input within 90s (prepare=0/2, tos=0/2, thumbs=+0)"
        );
        assert_eq!(
            upload_failed_msg(2, AttachWay::Paste, t, 1, 1, 1),
            "attached 1/2 via paste within 90s (prepare=1/2, tos=1/2, thumbs=+1)"
        );
        assert_eq!(
            upload_failed_msg(2, AttachWay::Input, t, 1, 0, 5),
            "attached 2/2 via input within 90s (prepare=1/2, tos=0/2, thumbs=+5)"
        );
        assert_eq!(
            upload_failed_msg(2, AttachWay::Input, t, 1, 2, -1),
            "attached 0/2 via input within 90s (prepare=1/2, tos=2/2, thumbs=+-1)"
        );
        assert_eq!(
            upload_failed_msg(1, AttachWay::Paste, t, 0, 0, 0),
            "attached 0/1 via paste within 90s (prepare=0/1, tos=0/1, thumbs=+0)"
        );
        // `int(config.REFERENCE_UPLOAD_TIMEOUT)`: cat phan le giay
        assert_eq!(
            upload_failed_msg(1, AttachWay::Input, Duration::from_millis(1999), 0, 0, 0),
            "attached 0/1 via input within 1s (prepare=0/1, tos=0/1, thumbs=+0)"
        );
        // Bao trong `DolaError::RefUpload` -> tien to `REF_UPLOAD: ` nhu `ReferenceUploadError`
        let e = DolaError::RefUpload(upload_failed_msg(2, AttachWay::Input, t, 0, 0, 0));
        assert_eq!(
            e.to_string(),
            "REF_UPLOAD: attached 0/2 via input within 90s (prepare=0/2, tos=0/2, thumbs=+0)"
        );
    }

    /// `way = "input"` `:857` / `"paste"` `:859`.
    #[test]
    fn attach_way_display() {
        assert_eq!(AttachWay::Input.to_string(), "input");
        assert_eq!(AttachWay::Paste.to_string(), "paste");
    }

    /// `:876-878` dieu kien xong: `prepare >= 1 and tos >= expected and thumbs >= expected`
    /// (Dot 6 BUG-001: prepare gop lo -> chi can >= 1).
    #[test]
    fn dieu_kien_xong_theo_python() {
        assert!(uploads_done(2, 1, 2, 2));
        assert!(uploads_done(2, 1, 5, 9));
        assert!(uploads_done(6, 2, 10, 24), "log 8000 154bc94c");
        assert!(!uploads_done(2, 0, 2, 2), "prepare=0");
        assert!(!uploads_done(2, 1, 1, 2), "tos thieu");
        assert!(!uploads_done(2, 1, 2, 1), "thumbs thieu");
        assert!(!uploads_done(2, 1, 2, -1), "thumbs am");
    }

    /// `:864` 700 ms giua 2 lan dan; `:874` san `max(10, T)`; `:879` +800 ms; `:884` poll 250 ms;
    /// `:774-780` 20 x 100 ms doc ket qua dan.
    #[test]
    fn hang_thoi_gian_khop_python() {
        assert_eq!(PASTE_GAP, Duration::from_millis(700));
        assert_eq!(DEADLINE_FLOOR, Duration::from_secs(10));
        assert_eq!(SETTLE_OK, Duration::from_millis(800));
        assert_eq!(POLL, Duration::from_millis(250));
        assert_eq!(PASTE_RESULT_POLLS, 20);
        assert_eq!(PASTE_RESULT_POLL, Duration::from_millis(100));
        assert_eq!(PREPARE_UPLOAD_PART, "/alice/resource/prepare_upload");
        assert_eq!(UPLOAD_PART, "/upload/v1/");
    }

    /// `paste_image_once` `:769-771`: `name = Path(path).name`, `mime` theo duoi, `b64` = base64
    /// chuan (`base64.b64encode`) cua NOI DUNG file. `Debug` KHONG in b64 (chinh sach log).
    #[test]
    fn paste_arg_tu_bytes() {
        let arg = PasteArg::new(Path::new(r"C:\anh\Mot Anh.JPG"), b"\x89PNG\r\n\x1a\n");
        assert_eq!(arg.name, "Mot Anh.JPG");
        assert_eq!(arg.mime, "image/jpeg");
        assert_eq!(arg.b64, "iVBORw0KGgo=");
        let dbg = format!("{arg:?}");
        assert!(!dbg.contains("iVBOR"), "lo b64: {dbg}");
        assert!(
            dbg.contains("Mot Anh.JPG") && dbg.contains("b64_len: 12"),
            "{dbg}"
        );
        // Serialize dung 3 khoa nhu dict Python `{"b64", "name", "mime"}`
        let v = serde_json::to_value(&arg).unwrap();
        assert_eq!(v["b64"], "iVBORw0KGgo=");
        assert_eq!(v["name"], "Mot Anh.JPG");
        assert_eq!(v["mime"], "image/jpeg");
        assert_eq!(v.as_object().unwrap().len(), 3);
    }

    /// `paste_image_once` `:775-778`: chuoi tra ve tu main world quyet dinh — `main ...`/`nobox`/
    /// `error...` -> tra ngay (khong fallback); chuoi la -> fallback; rong -> doi tiep.
    #[test]
    fn phan_loai_ket_qua_dan() {
        assert_eq!(
            classify_paste_result("main acc=2 consumed=true dp=true"),
            PasteResult::Final
        );
        assert_eq!(classify_paste_result("nobox"), PasteResult::Final);
        assert_eq!(classify_paste_result("error TypeError"), PasteResult::Final);
        assert_eq!(classify_paste_result(""), PasteResult::Pending);
        assert_eq!(classify_paste_result("la"), PasteResult::Other);
        assert_eq!(
            classify_paste_result("main"),
            PasteResult::Other,
            "can 'main ' co dau cach"
        );
    }

    /// `:862` chi `nobox` va `error...` la loi dan; `main ...`/`fallback` di tiep.
    #[test]
    fn paste_that_bai_theo_python() {
        assert!(paste_failed("nobox"));
        assert!(paste_failed("error TypeError: x"));
        assert!(!paste_failed("main acc=1 consumed=true dp=false"));
        assert!(!paste_failed("fallback"));
    }

    /// `:883` `min(thumbs, expected) if thumbs > 0 else 0`.
    #[test]
    fn attached_count_theo_python() {
        assert_eq!(attached_count(0, 2), 0);
        assert_eq!(attached_count(-3, 2), 0);
        assert_eq!(attached_count(1, 2), 1);
        assert_eq!(attached_count(2, 2), 2);
        assert_eq!(attached_count(7, 2), 2);
    }

    #[test]
    fn outcome_rong_khi_khong_co_anh() {
        let o = AttachOutcome::empty();
        assert_eq!(o.expected, 0);
        assert_eq!(o.way, None);
        assert!(!o.extra_uploads);
    }
}
