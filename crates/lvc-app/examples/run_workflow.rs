//! THÍ NGHIỆM LIVE: chạy NGUYÊN MỘT TEMPLATE qua `Engine::run_graph` + registry node thật
//! (`ChromiumDriver` — TỐN CREDIT Dola). Điền `out_dir` (node Lưu) + `account` (node dola_video) rồi
//! chạy DAG end-to-end; kiểm file lưu ra.
//!
//! Chạy: `cargo run -p lvc-app --example run_workflow -- <template.json> <account> <accounts_dir>
//! <out_dir> <db> <downloads_dir>`
//! stderr = log; stdout = tóm tắt.
//!
//! Biến môi trường tuỳ chọn cho workflow MuseAI:
//! - `LVC_MUSEAI_VAULT=1`: dùng resolver KHO TÀI KHOẢN thật (chia tải + chuyển dự phòng giữa các
//!   tài khoản MuseAI trong `<db>`), thay vì một profile cố định. Nên trỏ `<db>` vào BẢN SAO.
//! - `LVC_NODE_THREADS=N`: số node đồ thị chạy song song (mặc định của engine là 1).
#![allow(
    clippy::doc_markdown,
    reason = "doc tiếng Việt nhắc tên tham số dòng lệnh, không phải mã"
)]

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lvc_browser::chromium::ChromiumDriver;
use lvc_core::Limits;
use lvc_core::graph::Graph;
use lvc_engine::{Engine, EngineConfig, MuseaiAccountResolver, noop_hooks_factory};
use lvc_store::{AccountRepo, Db};
use tracing_subscriber::EnvFilter;

/// Resolver trỏ THẲNG tới một profile Chromium đã đăng nhập MuseAI (bỏ qua credential_id) — dùng để
/// chạy thử workflow MuseAI qua CLI. Đường dẫn lấy từ env `LVC_MUSEAI_PROFILE`.
struct FixedMuseaiProfile(PathBuf);

#[async_trait]
impl MuseaiAccountResolver for FixedMuseaiProfile {
    async fn profile_dir(&self, _credential_id: &str) -> Result<PathBuf, String> {
        Ok(self.0.clone())
    }
}

fn arg(i: usize, default: &str) -> String {
    std::env::args()
        .nth(i)
        .unwrap_or_else(|| default.to_string())
}

fn init_tracing() {
    let policy = "chromiumoxide=warn,chromiumoxide::handler=error";
    let user = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(format!("{user},{policy}")).unwrap())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}

/// Ảnh tham chiếu từ env `LVC_REFS` (danh sách đường dẫn ngăn cách `;`).
fn env_refs() -> Vec<String> {
    std::env::var("LVC_REFS")
        .ok()
        .map(|s| {
            s.split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Đọc template → graph, điền `out_dir` (node Lưu), `account` + `LVC_DURATION` (node dola_video), và
/// gán lần lượt `LVC_REFS` cho từng node `image_upload` (node 1 → ảnh 1, ...) để test luồng affiliate.
fn load_graph(template: &str, account: &str, out_dir: &str) -> Graph {
    let refs = env_refs();
    let duration_override: Option<u64> = std::env::var("LVC_DURATION")
        .ok()
        .and_then(|s| s.parse().ok());

    let raw = std::fs::read_to_string(template).expect("đọc template");
    let mut doc: serde_json::Value = serde_json::from_str(&raw).expect("parse template json");
    let mut graph_val = doc.get_mut("graph").cloned().unwrap_or(doc.clone());
    if let Some(nodes) = graph_val.get_mut("nodes").and_then(|n| n.as_array_mut()) {
        let mut img_idx = 0usize;
        for node in nodes.iter_mut() {
            let Some(kind) = node.get_mut("kind") else {
                continue;
            };
            match kind.get("kind").and_then(|k| k.as_str()) {
                Some("save") => kind["out_dir"] = serde_json::Value::String(out_dir.to_string()),
                Some("dola_video") => {
                    kind["account"] = serde_json::Value::String(account.to_string());
                    if let Some(d) = duration_override {
                        kind["duration"] = serde_json::Value::from(d);
                    }
                }
                Some("image_upload") if !refs.is_empty() => {
                    let p = refs[img_idx % refs.len()].clone();
                    img_idx += 1;
                    kind["paths"] = serde_json::Value::Array(vec![serde_json::Value::String(p)]);
                }
                _ => {}
            }
        }
    }
    serde_json::from_value(graph_val).expect("graph hợp lệ")
}

#[tokio::main]
async fn main() {
    let template = arg(1, "crates/lvc-app/templates/prompt-video-save.json");
    let account = arg(2, "luong01");
    let accounts_dir = PathBuf::from(arg(3, "accounts"));
    let out_dir = arg(4, "out");
    let db_path = PathBuf::from(arg(5, "lvc.db"));
    let downloads_dir = PathBuf::from(arg(6, "downloads"));

    init_tracing();
    let graph = load_graph(&template, &account, &out_dir);

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
    let (engine, _handle) = Engine::new(
        Arc::new(ChromiumDriver),
        db.clone(),
        Limits::default(),
        cfg,
        1,
    )
    .await
    .expect("Engine::new");
    let engine = Arc::new(engine);
    engine.set_registry(Arc::new(lvc_app_lib::registry::build_registry()));
    if let Some(n) = std::env::var("LVC_NODE_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        engine.apply_settings(&lvc_core::settings::AppSettings {
            node_threads: n,
            ..Default::default()
        });
    }

    // MuseAI: nối resolver profile cố định (env LVC_MUSEAI_PROFILE) để node museai_image/museai_video
    // mở đúng profile đã đăng nhập. Thiếu env = không chạy được node MuseAI.
    if std::env::var_os("LVC_MUSEAI_VAULT").is_some() {
        let vault = Arc::new(lvc_vault::Vault::new(&db, lvc_vault::default_box()));
        tracing::info!("MuseAI: dùng resolver kho tài khoản (chia tải)");
        engine.set_museai_account_resolver(Arc::new(
            lvc_app_lib::museai::VaultMuseaiAccountResolver::new(vault),
        ));
    } else if let Ok(prof) = std::env::var("LVC_MUSEAI_PROFILE") {
        tracing::info!(profile = %prof, "MuseAI profile cố định");
        engine.set_museai_account_resolver(Arc::new(FixedMuseaiProfile(PathBuf::from(prof))));
    }

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
            rd.filter_map(std::result::Result::ok)
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
