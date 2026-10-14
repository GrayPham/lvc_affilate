//! THI NGHIEM SONG MOT LAN (khong phai code san xuat): sinh MOT video `MuseAI` THAT qua luong day
//! du `Engine::run_graph` (do thi `prompt -> museai_video`, dung khuon `gen_one.rs` cua Dola) —
//! dung cookie that da nap san vao profile cua `lvc-browser/examples/museai_probe.rs`
//! (`%TEMP%/lvc-museai-probe-profile`). Resolver tai khoan o day CO Y bo qua Vault (tra thang
//! `profile_dir` co dinh) — chi de kiem thu, san xuat dung `VaultMuseaiAccountResolver` (lvc-app).
//!
//! TON CREDIT/HAN MUC tai khoan `MuseAI` THAT cua nguoi dung — CHI chay khi nguoi dung dong y.
//! Chay: `cargo run -p lvc-engine --example museai_gen_one -- "<prompt>"`

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lvc_browser::chromium::ChromiumDriver;
use lvc_core::Limits;
use lvc_core::graph::{Edge, GRAPH_VERSION, Graph, Node, NodeData};
use lvc_engine::{Engine, EngineConfig, MuseaiAccountResolver, noop_hooks_factory};
use lvc_node_api::Registry;
use lvc_store::Db;
use serde_json::json;

struct FixedProfileResolver(PathBuf);

#[async_trait]
impl MuseaiAccountResolver for FixedProfileResolver {
    async fn profile_dir(&self, _credential_id: &str) -> Result<PathBuf, String> {
        Ok(self.0.clone())
    }
}

#[tokio::main]
async fn main() {
    let prompt = std::env::args().nth(1).unwrap_or_else(|| {
        "mot con meo mau cam dang choi voi cuon len, hoat hinh 3D de thuong".to_string()
    });

    let scratch = std::env::temp_dir().join("lvc-museai-gen-one");
    let accounts_dir = scratch.join("accounts");
    let downloads_dir = scratch.join("downloads");
    let logs_dir = scratch.join("logs");
    std::fs::create_dir_all(&accounts_dir).unwrap();
    std::fs::create_dir_all(&downloads_dir).unwrap();
    std::fs::create_dir_all(&logs_dir).unwrap();

    let db = Db::open_memory().await.unwrap();
    let mut cfg = EngineConfig::new(&accounts_dir, &downloads_dir);
    cfg.logs_dir = logs_dir;

    let driver = Arc::new(ChromiumDriver);
    let (engine, _handle) = Engine::new(driver, db, Limits::default(), cfg, 1)
        .await
        .unwrap();
    let engine = Arc::new(engine);

    let registry = Registry::new()
        .with(node_prompt::node())
        .with(node_museai_video::node());
    engine.set_registry(Arc::new(registry));

    let profile_dir = std::env::temp_dir().join("lvc-museai-probe-profile");
    if !profile_dir.is_dir() {
        eprintln!(
            "khong thay profile {} — chay museai_probe truoc de dang nhap/nap cookie",
            profile_dir.display()
        );
        std::process::exit(2);
    }
    engine.set_museai_account_resolver(Arc::new(FixedProfileResolver(profile_dir)));

    let graph = Graph {
        version: GRAPH_VERSION,
        nodes: vec![
            Node {
                id: "p1".into(),
                kind: NodeData::new("prompt", json!({ "text": prompt })),
                x: 0.0,
                y: 0.0,
            },
            Node {
                id: "v1".into(),
                kind: NodeData::new("museai_video", json!({ "account": "test" })),
                x: 300.0,
                y: 0.0,
            },
        ],
        edges: vec![Edge {
            id: "e1".into(),
            from: "p1".into(),
            from_port: "out".into(),
            to: "v1".into(),
            to_port: "prompt".into(),
        }],
    };

    println!("dang chay workflow (TON CREDIT/HAN MUC MuseAI THAT)...");
    let run_handle = engine
        .run_graph("wf-museai-test", &graph, noop_hooks_factory())
        .await
        .expect("run_graph that bai");
    let run_id = run_handle.run_id.clone();
    let _ = run_handle.join.await;
    println!(
        "run {run_id} hoan tat. Kiem file video tai {}",
        downloads_dir.display()
    );

    let expected = downloads_dir.join(format!("{run_id}_v1_1.mp4"));
    if expected.exists() {
        let size = std::fs::metadata(&expected).map_or(0, |m| m.len());
        println!("THANH CONG: video tai {} ({size} byte)", expected.display());
    } else {
        println!("KHONG thay file video mong doi: {}", expected.display());
        if let Ok(entries) = std::fs::read_dir(&downloads_dir) {
            for e in entries.flatten() {
                println!("  co trong downloads_dir: {}", e.path().display());
            }
        }
    }
}
