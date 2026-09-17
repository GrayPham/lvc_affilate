//! Chi cho test: doc nguon Python goc (`A:\ToolLVC\lvcseedance`, chi doc) de SO BYTE cac hang
//! chep nguyen van trong `labels.rs`. File vang -> test in ly do va bo qua (khong fail) — cung
//! cach P0.5 lam voi `SESSION_PROBE_JS`.

use std::path::PathBuf;

/// Goc repo Python: env `LVC_PY_ROOT` hoac mac dinh `A:\ToolLVC\lvcseedance`.
fn python_root() -> PathBuf {
    std::env::var_os("LVC_PY_ROOT")
        .map_or_else(|| PathBuf::from(r"A:\ToolLVC\lvcseedance"), PathBuf::from)
}

/// Doc `rel` (duong dan tuong doi trong repo Python) thanh chuoi; CRLF -> LF vi Python doc source
/// bang universal newlines nen literal `r"""..."""` luc chay chi chua `\n` (working copy Windows
/// checkout CRLF). `None` (kem ly do tren stderr) khi file khong doc duoc.
pub(crate) fn python_source(rel: &str) -> Option<String> {
    let path = python_root().join(rel);
    match std::fs::read(&path) {
        Ok(bytes) => Some(
            String::from_utf8(bytes)
                .unwrap_or_else(|e| panic!("{} khong phai UTF-8: {e}", path.display()))
                .replace("\r\n", "\n"),
        ),
        Err(e) => {
            eprintln!(
                "BO QUA so byte voi Python: khong doc duoc {} ({e}) — dat LVC_PY_ROOT neu repo o cho khac",
                path.display()
            );
            None
        }
    }
}

/// Lay literal chuoi Python gan cho `name` o DAU DONG (`NAME = ...`): ho tro `r"""..."""`,
/// `"..."`, `'...'`, va `re.compile(r"...", re.IGNORECASE)` (chi lay pattern). Khong xu ly escape
/// (moi hang ta chep deu la raw string hoac khong co escape).
pub(crate) fn py_literal(src: &str, name: &str) -> String {
    let key = format!("\n{name} = ");
    let start = src
        .find(&key)
        .unwrap_or_else(|| panic!("khong thay `{name} = ` o dau dong trong nguon Python"))
        + key.len();
    let rest = &src[start..];
    let rest = rest.strip_prefix("re.compile(").unwrap_or(rest);
    let rest = rest.strip_prefix('r').unwrap_or(rest);
    let (quote, body) = if let Some(b) = rest.strip_prefix("\"\"\"") {
        ("\"\"\"", b)
    } else if let Some(b) = rest.strip_prefix('"') {
        ("\"", b)
    } else if let Some(b) = rest.strip_prefix('\'') {
        ("'", b)
    } else {
        panic!(
            "`{name}` khong phai literal chuoi: {:?}",
            &rest[..rest.len().min(40)]
        );
    };
    let end = body
        .find(quote)
        .unwrap_or_else(|| panic!("`{name}`: khong thay dau dong {quote}"));
    body[..end].to_string()
}

/// Duong dan file Python chinh cua video worker (dung trong thong diep test).
pub(crate) const VIDEO_WORKER_UI: &str = r"src\video\video_worker_ui.py";
pub(crate) const MODES_PY: &str = r"src\utils\modes.py";

/// So sanh `rust` voi literal Python `name`; tra `true` neu khop. In chi tiet khi lech (dong dau
/// tien khac nhau) de sua nhanh.
pub(crate) fn same_bytes(src: &str, name: &str, rust: &str) -> bool {
    let py = py_literal(src, name);
    if py == rust {
        return true;
    }
    let mismatch = py
        .lines()
        .zip(rust.lines())
        .position(|(a, b)| a != b)
        .map_or_else(
            || {
                format!(
                    "(so dong khac: python {} / rust {})",
                    py.lines().count(),
                    rust.lines().count()
                )
            },
            |i| {
                format!(
                    "dong {} khac:\n  python: {:?}\n  rust:   {:?}",
                    i + 1,
                    py.lines().nth(i).unwrap_or(""),
                    rust.lines().nth(i).unwrap_or("")
                )
            },
        );
    eprintln!(
        "LECH `{name}` (python {} byte, rust {} byte) {mismatch}",
        py.len(),
        rust.len()
    );
    false
}
