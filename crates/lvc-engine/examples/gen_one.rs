//! Task 18 — THI NGHIEM LIVE: tao MOT video that bang `Engine` + `ChromiumDriver` (TON CREDIT
//! Dola — chi chay khi nguoi dung dong y). `main` mong: toan bo logic o `lvc_engine::cli`
//! (test bang `FakeDriver`, khong Chrome).
//!
//! Chay: `cargo run -p lvc-engine --example gen_one -- --account <ten> --prompt "..." [--mode fast]
//! [--duration 10] [--ratio 9:16] [--model seedance-2.0] [--ref a.png]... [--headed] [--db lvc.db]
//! [--accounts-dir accounts] [--downloads-dir downloads] [--logs-dir logs] [--threads 1]`
//! Bien moi truong: `LVC_CHROME` = duong dan chrome.exe; `RUST_LOG` = filter tracing (mac dinh
//! `info`; `chromiumoxide=warn` LUON duoc noi SAU directive nguoi dung — chinh sach log lvc-browser:
//! crate do in nguyen payload CDP gom cookie o muc debug/trace).
//! Ra: stderr = log; stdout = MOT JSON tong ket (khong prompt, khong URL ky, khong cookie).
//! Ma thoat: 0 = co video; 1 = tao video that bai (JSON co `failure_code`/`error`); 2 = tham so sai.

use std::sync::Arc;

use lvc_browser::chromium::ChromiumDriver;
use lvc_engine::cli::{Args, run};
use tracing_subscriber::EnvFilter;

fn init_tracing() {
    // Directive nguoi dung (RUST_LOG, mac dinh info) TRUOC, `chromiumoxide=warn` SAU -> thang.
    // `chromiumoxide::handler=error`: Chrome 151 gui event CDP moi ma chromiumoxide 0.9 chua
    // biet -> "WS Invalid message" o muc WARN, 309 dong/video (do live 2026-09-18), vo hai.
    const POLICY: &str = "chromiumoxide=warn,chromiumoxide::handler=error";
    let user = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let filter = EnvFilter::try_new(format!("{user},{POLICY}"))
        .unwrap_or_else(|_| EnvFilter::new(format!("info,{POLICY}")));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(false) // log hay bi chuyen huong vao file
        .init();
}

#[tokio::main]
async fn main() {
    let args = match Args::parse_from(std::env::args().skip(1)) {
        Ok(a) => a.with_env(),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    init_tracing();
    tracing::info!(?args, "gen_one");
    match run(&args, Arc::new(ChromiumDriver)).await {
        Ok(summary) => {
            println!("{}", summary.to_json());
            std::process::exit(i32::from(summary.error.is_some()));
        }
        Err(e) => {
            eprintln!("gen_one: {e}");
            std::process::exit(2);
        }
    }
}
