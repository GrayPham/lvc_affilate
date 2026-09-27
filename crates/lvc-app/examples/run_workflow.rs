//! THÍ NGHIỆM LIVE: chạy NGUYÊN MỘT TEMPLATE qua `Engine::run_graph` + registry node thật
//! (`ChromiumDriver` — TỐN CREDIT Dola). Điền `out_dir` (node Lưu) + `account` (node dola_video) rồi
//! chạy DAG end-to-end; kiểm file lưu ra.
//!
//! Chạy: cargo run -p lvc-app --example run_workflow -- <template.json> <account> <accounts_dir> \
//!       <out_dir> <db> <downloads_dir>
//! stderr = log; stdout = tóm tắt.

use std::path::PathBuf;
use std::sync::Arc;

use lvc_browser::chromium::ChromiumDriver;
use lvc_core::Limits;
use lvc_core::graph::Graph;
use lvc_engine::{Engine, EngineConfig, noop_hooks_factory};
use lvc_store::{AccountRepo, Db};
use tracing_subscriber::EnvFilter;

fn arg(i: usize, default: &str) -> String {
    std::env::args().nth(i).unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() {
    let template = arg(1, "crates/lvc-app/templates/prompt-video-save.json");
    let account = arg(2, "luong01");
    let accounts_dir = PathBuf::from(arg(3, "accounts"));
    let out_dir = arg(4, "out");
    let db_path = PathBuf::from(arg(5, "lvc.db"));
    let downloads_dir = PathBuf::from(arg(6, "downloads"));

    let policy = "chromiumoxide=warn,chromiumoxide::handler=error";
    let user = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(format!("{user},{policy}")).unwrap())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    // Đọc template -> lấy graph -> điền out_dir (node Lưu) + account (node dola_video).
    let raw = std::fs::read_to_string(&template).expect("đọc template");
    let mut doc: serde_json::Value = serde_json::from_str(&raw).expect("parse template json");
    let graph_val = doc.get_mut("graph").cloned().unwrap_or(doc.clone());
    let mut graph_val = graph_val;
    if let Some(nodes) = graph_val.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        for node in nodes.iter_mut() {
            let Some(kind) = node.get_mut("kind") else { continue };
            match kind.get("kind").and_then(|k| k.as_str()) {
                Some("save") => {
                    kind["out_dir"] = serde_json::Value::String(out_dir.clone());
                }
                Some("dola_video") => {
                    kind["account"] = serde_json::Value::String(account.clone());
                }
                _ => {}
            }
        }
    }
    let graph: Graph = serde_json::from_value(graph_val).expect("graph hợp lệ");

    std::fs::create_dir_all(&out_dir).ok();
    std::fs::create_dir_all(&downloads_dir).ok();

    let db = Db::open(&db_path).await.expect("mở db");
    let repo = AccountRepo::new(&db);
    if repo.get(&account).await.expect("get account").is_none() {
        repo.upsert(&account).await.expect("upsert account");
        repo.set_browser(&account, "chromium")
            .await
            .expect("set_browser");
    }

    let cfg = EngineConfig::new(&accounts_dir, &downloads_dir);
    let (engine, _handle) = Engine::new(Arc::new(ChromiumDriver), db.clone(), Limits::default(), cfg, 1)
        .await
        .expect("Engine::new");
    let engine = Arc::new(engine);
    engine.set_registry(Arc::new(lvc_app_lib::registry::build_registry()));

    tracing::info!(template = %template, account = %account, "chạy template qua run_graph");
    let handle = engine
        .run_graph("wf-live-test", &graph, noop_hooks_factory())
        .await
        .expect("run_graph");
    let run_id = handle.run_id.clone();
    handle.join.await.expect("run join");

    // Kết quả: liệt kê file trong out_dir.
    let files: Vec<String> = std::fs::read_dir(&out_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path().display().to_string())
                .collect()
        })
        .unwrap_or_default();
    println!("run_id={run_id}");
    println!("out_dir={out_dir}");
    println!("saved_files={}", files.len());
    for f in &files {
        println!("  {f}");
    }
    std::process::exit(i32::from(files.is_empty()));
}
