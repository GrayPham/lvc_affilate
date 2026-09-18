//! Build script Tauri 2. `app/dist` bi `.gitignore` (san pham build cua `app/`, Task 11) nen tren
//! checkout moi thu muc nay khong ton tai — `tauri_build::build()` doc `frontendDist` tu
//! `tauri.conf.json` va se loi neu duong dan do khong co. Truoc T11 (React chua viet), ta tu tao
//! mot `index.html` gia (placeholder) de `cargo build -p lvc-app` xanh; T11 chay `pnpm -C app build`
//! roi ghi de bang san pham that (build.rs KHONG dong vao thu muc da co san — chi tao khi thieu).

use std::path::Path;

fn ensure_frontend_dist_placeholder() {
    // Tuong doi CWD cua build script = thu muc goc package (crates/lvc-app) — xem
    // `frontendDist` trong tauri.conf.json cung tinh tu day.
    let dist_dir = Path::new("../../app/dist");
    if dist_dir.exists() {
        return;
    }
    std::fs::create_dir_all(dist_dir).expect("tao app/dist that bai");
    std::fs::write(
        dist_dir.join("index.html"),
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>LVC Seedance</title></head>\
         <body>LVC Seedance</body></html>\n",
    )
    .expect("ghi placeholder app/dist/index.html that bai");
}

fn main() {
    ensure_frontend_dist_placeholder();
    tauri_build::build();
}
