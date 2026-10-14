//! THI NGHIEM MOT LAN (khong phai code san xuat): chay 5 "kich ban" affiliate qua node
//! `museai_video`, MOI kich ban sinh 3 video (dung chung 1 prompt, dung khuon "best-of-3") — sinh
//! THAT, TON CREDIT/HAN MUC tai khoan that. Ghi ket qua ra JSON de buoc sau (trich khung hinh +
//! xep hang) doc lai, khong phai parse log console.
//!
//! Chay: `cargo run -p lvc-engine --example museai_batch`
//! Ra: `%TEMP%/lvc-museai-batch/summary.json` — mang cac { template, prompt, videos: [{`scene_index`,
//! path, `size_bytes`, ok, error}] }.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use lvc_browser::chromium::ChromiumDriver;
use lvc_core::Limits;
use lvc_core::graph::{Edge, GRAPH_VERSION, Graph, Node, NodeData};
use lvc_engine::{Engine, EngineConfig, MuseaiAccountResolver, noop_hooks_factory};
use lvc_node_api::Registry;
use lvc_store::Db;
use serde::Serialize;
use serde_json::json;

struct FixedProfileResolver(PathBuf);

#[async_trait]
impl MuseaiAccountResolver for FixedProfileResolver {
    async fn profile_dir(&self, _credential_id: &str) -> Result<PathBuf, String> {
        Ok(self.0.clone())
    }
}

/// 5 kich ban affiliate san pham — moi kich ban la MOT nganh hang pho bien, prompt mo ta canh quay
/// san pham sinh dong, phu hop phong cach quang cao affiliate. Tieng Viet (da xac nhan `MuseAI`
/// hieu tot qua lan test truoc — video meo cam thanh cong voi prompt tieng Viet).
const TEMPLATES: &[(&str, &str)] = &[
    (
        "my-pham-serum",
        "Một lọ serum dưỡng da cao cấp đặt trên bàn đá cẩm thạch trắng, ánh sáng studio ấm áp \
         chiếu từ trên xuống, giọt serum vàng óng nhỏ xuống chậm rãi, nền mờ với hoa và lá xanh, \
         phong cách quảng cáo mỹ phẩm sang trọng, cận cảnh sản phẩm xoay nhẹ",
    ),
    (
        "tai-nghe-khong-day",
        "Một chiếc tai nghe không dây màu trắng hiện đại đặt trên nền tối phản chiếu, ánh đèn \
         neon xanh và tím chiếu quanh, sản phẩm xoay 360 độ khoe thiết kế, hiệu ứng ánh sáng công \
         nghệ cao, phong cách quảng cáo gadget cao cấp",
    ),
    (
        "may-xay-sinh-to",
        "Một chiếc máy xay sinh tố hiện đại trên quầy bếp sáng sủa, trái cây tươi dâu chuối việt \
         quất được đổ vào, máy hoạt động xay nhuyễn mượt mà, ánh nắng buổi sáng chiếu qua cửa sổ, \
         không khí nhà bếp ấm cúng gia đình",
    ),
    (
        "kinh-mat-thoi-trang",
        "Một người mẫu trẻ đeo kính mát thời trang đứng trên bãi biển lúc hoàng hôn, gió nhẹ thổi \
         tóc, ánh nắng vàng cam phản chiếu trên mắt kính, phong cách quảng cáo thời trang mùa hè, \
         cận cảnh chi tiết gọng kính",
    ),
    (
        "whey-protein",
        "Một hũ bột protein thể hình đặt cạnh bình lắc trong phòng gym hiện đại, ánh sáng mạnh mẽ \
         năng động, bột protein được múc và đổ vào bình lắc tạo hiệu ứng chuyển động chậm, không \
         khí thể thao năng lượng cao",
    ),
];

const VIDEOS_PER_TEMPLATE: usize = 3;

#[derive(Serialize)]
struct VideoResult {
    scene_index: u32,
    path: Option<String>,
    size_bytes: Option<u64>,
    ok: bool,
}

#[derive(Serialize)]
struct TemplateResult {
    template: String,
    prompt: String,
    run_id: String,
    videos: Vec<VideoResult>,
}

/// Thư mục dùng chung cho một lần chạy `run_template` — gom lại cho gọn chữ ký hàm (clippy
/// `too_many_arguments`).
struct TemplateCtx<'a> {
    accounts: &'a std::path::Path,
    downloads: &'a std::path::Path,
    logs: &'a std::path::Path,
    profile: &'a std::path::Path,
}

/// Chạy MỘT kịch bản: dựng engine riêng, sinh `VIDEOS_PER_TEMPLATE` video thật, đọc lại file đã
/// tải để báo cáo — tách khỏi `main` để không vượt giới hạn `clippy::too_many_lines`.
async fn run_template(ctx: &TemplateCtx<'_>, name: &str, prompt: &str) -> TemplateResult {
    let db = Db::open_memory().await.unwrap();
    let mut cfg = EngineConfig::new(ctx.accounts, ctx.downloads);
    cfg.logs_dir = ctx.logs.to_path_buf();
    let driver = Arc::new(ChromiumDriver);
    let (engine, _handle) = Engine::new(driver, db, Limits::default(), cfg, 1)
        .await
        .unwrap();
    let engine = Arc::new(engine);

    let registry = Registry::new()
        .with(node_prompt::node())
        .with(node_museai_video::node());
    engine.set_registry(Arc::new(registry));
    engine.set_museai_account_resolver(Arc::new(FixedProfileResolver(ctx.profile.to_path_buf())));

    let mut nodes = vec![Node {
        id: "v1".into(),
        kind: NodeData::new("museai_video", json!({ "account": "test" })),
        x: 300.0,
        y: 0.0,
    }];
    let mut edges = Vec::new();
    for i in 0..VIDEOS_PER_TEMPLATE {
        let pid = format!("p{i}");
        nodes.push(Node {
            id: pid.clone(),
            kind: NodeData::new("prompt", json!({ "text": prompt })),
            x: 0.0,
            y: f32::from(u16::try_from(i).unwrap_or(u16::MAX)) * 100.0,
        });
        edges.push(Edge {
            id: format!("e{i}"),
            from: pid,
            from_port: "out".into(),
            to: "v1".into(),
            to_port: "prompt".into(),
        });
    }
    let graph = Graph {
        version: GRAPH_VERSION,
        nodes,
        edges,
    };

    println!("dang chay {VIDEOS_PER_TEMPLATE} video that cho '{name}'...");
    let run_handle = match engine.run_graph(name, &graph, noop_hooks_factory()).await {
        Ok(h) => h,
        Err(e) => {
            println!("  run_graph that bai: {e}");
            return TemplateResult {
                template: name.to_string(),
                prompt: prompt.to_string(),
                run_id: String::new(),
                videos: Vec::new(),
            };
        }
    };
    let run_id = run_handle.run_id.clone();
    let _ = run_handle.join.await;

    let mut videos = Vec::new();
    let total_scenes = u32::try_from(VIDEOS_PER_TEMPLATE).unwrap_or(u32::MAX);
    for scene_index in 1..=total_scenes {
        let path = ctx.downloads.join(format!("{run_id}_v1_{scene_index}.mp4"));
        if path.exists() {
            let size = std::fs::metadata(&path).map_or(0, |m| m.len());
            println!("  canh {scene_index}: OK ({size} byte) {}", path.display());
            videos.push(VideoResult {
                scene_index,
                path: Some(path.to_string_lossy().into_owned()),
                size_bytes: Some(size),
                ok: true,
            });
        } else {
            println!("  canh {scene_index}: KHONG co file (that bai)");
            videos.push(VideoResult {
                scene_index,
                path: None,
                size_bytes: None,
                ok: false,
            });
        }
    }

    TemplateResult {
        template: name.to_string(),
        prompt: prompt.to_string(),
        run_id,
        videos,
    }
}

#[tokio::main]
async fn main() {
    let scratch = std::env::temp_dir().join("lvc-museai-batch");
    let accounts_dir = scratch.join("accounts");
    let downloads_dir = scratch.join("downloads");
    let logs_dir = scratch.join("logs");
    std::fs::create_dir_all(&accounts_dir).unwrap();
    std::fs::create_dir_all(&downloads_dir).unwrap();
    std::fs::create_dir_all(&logs_dir).unwrap();

    let profile_dir = std::env::temp_dir().join("lvc-museai-probe-profile");
    if !profile_dir.is_dir() {
        eprintln!(
            "khong thay profile {} — chay museai_probe truoc",
            profile_dir.display()
        );
        std::process::exit(2);
    }

    let mut all_results: Vec<TemplateResult> = Vec::new();

    for (idx, (name, prompt)) in TEMPLATES.iter().enumerate() {
        println!("\n=== Kich ban {}/{}: {name} ===", idx + 1, TEMPLATES.len());
        let ctx = TemplateCtx {
            accounts: &accounts_dir,
            downloads: &downloads_dir,
            logs: &logs_dir,
            profile: &profile_dir,
        };
        all_results.push(run_template(&ctx, name, prompt).await);

        let summary_path = scratch.join("summary.json");
        std::fs::write(
            &summary_path,
            serde_json::to_string_pretty(&all_results).unwrap(),
        )
        .unwrap();
        println!("(da ghi tam summary.json sau kich ban nay)");
    }

    let summary_path = scratch.join("summary.json");
    println!("\n=== XONG TAT CA. Summary: {} ===", summary_path.display());
    let total_ok: usize = all_results
        .iter()
        .flat_map(|t| &t.videos)
        .filter(|v| v.ok)
        .count();
    let total: usize = all_results.iter().map(|t| t.videos.len()).sum();
    println!("Thanh cong {total_ok}/{total} video.");
}
