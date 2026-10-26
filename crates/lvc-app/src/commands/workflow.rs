//! Lenh workflow/run (Task 9): luu/doc/xoa workflow (bang `workflows`), bat dau/huy mot lan chay
//! do thi (`Engine::run_graph`/`cancel_run`, Task 7), liet ke run gan day (bang `runs`). Xem quy
//! uoc `xxx_impl`/`xxx` o dau `commands/mod.rs`.

use std::sync::Arc;

use lvc_browser::BrowserDriver;
use lvc_core::Limits;
use lvc_core::graph::{Graph, NodeManifest, migrate};
use lvc_engine::{EngineError, RunHandle, noop_hooks_factory};
use lvc_node_api::Registry;
use lvc_store::{NodeStateRepo, NodeStateRow, RunRepo, RunRow, WorkflowMeta, WorkflowRepo};
use serde::{Deserialize, Serialize};
use tracing::warn;
use uuid::Uuid;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Ket qua `workflow_validate` khi do thi hop le — `warnings` de danh cho canh bao khong-loi trong
/// tuong lai (vd node chua cau hinh tai khoan); P3 hien luon rong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    pub video_total: u32,
    pub warnings: Vec<String>,
}

/// Ket qua `run_start`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunInfo {
    pub run_id: String,
}

/// Ket qua `workflow_get` — `WorkflowRepo::get` chi tra `WorkflowRow.json` dang CHUOI THO; UI (Task
/// 11) can `Graph` da parse san de dua vao React Flow ngay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDoc {
    pub meta: WorkflowMeta,
    pub graph: Graph,
}

/// Than logic `workflow_validate` — THUAN (khong dung `Ctx`/DB/engine truc tiep, chi nhan `limits`
/// tu ben ngoai): `lvc_core::graph::plan` (gom ca `validate`). `limits` do lenh Tauri (duoi)
/// truyen vao tu `Engine::limits()` — CUNG mot nguon voi `run_start` (`Engine::run_graph` tu goi
/// `plan(graph, &self.limits)` ben trong), tranh hai noi tu hardcode `Limits::default()` rieng roi
/// troi nhau ve sau (fix-round 1, M8).
///
/// # Errors
/// `IpcError` ma `GRAPH_INVALID`, `details` = `Vec<GraphError>` da serialize (dung LAI cung mot
/// anh xa voi moi loi `EngineError::Graph` khac trong crate — `IpcError::from`, fix-round 1 M8),
/// khi do thi khong hop le hoac loi lap ke hoach (vd cu phap TXT trong node Prompt).
pub fn workflow_validate_impl(
    graph: &Graph,
    registry: &Registry,
    limits: &Limits,
) -> Result<PlanSummary, IpcError> {
    match registry.prepare(&migrate(graph.clone()), limits) {
        Ok(prepared) => Ok(PlanSummary {
            video_total: prepared.video_total,
            warnings: Vec::new(),
        }),
        Err(errs) => Err(IpcError::from(EngineError::Graph(errs))),
    }
}

#[allow(
    clippy::unused_async,
    reason = "giu async dong bo voi moi lenh IPC khac (quy uoc brief Task 9); than logic thuan"
)]
#[tauri::command]
pub async fn workflow_validate(
    state: tauri::State<'_, AppState>,
    graph: Graph,
) -> Result<PlanSummary, IpcError> {
    workflow_validate_impl(&graph, &state.engine.registry(), state.engine.limits())
}

/// P4a: manifest MOI node da cai (sap theo `kind`) — UI dung dung palette/cong/form tu day.
#[allow(
    clippy::unused_async,
    reason = "giu async dong bo voi moi lenh IPC khac (quy uoc brief Task 9)"
)]
#[tauri::command]
pub async fn node_catalog(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<NodeManifest>, IpcError> {
    Ok(state.engine.registry().manifests())
}

/// Than logic `workflow_save` — `id = None` sinh `uuid v4` moi (workflow moi tao trong UI).
///
/// # Errors
/// `IpcError` ma `STORE` khi ghi bang `workflows` that bai.
pub async fn workflow_save_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: Option<String>,
    name: &str,
    graph: &Graph,
) -> Result<WorkflowMeta, IpcError> {
    let id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
    // serde_json::to_string tren Graph (kieu dong bo, khong chua so NaN/vo cuc) khong the that bai
    // trong thuc te — van map loi thay vi unwrap de khong panic neu gia dinh nay sai trong tuong lai.
    let json = serde_json::to_string(&migrate(graph.clone()))
        .map_err(|e| IpcError::new("GRAPH_ENCODE", format!("khong ma hoa duoc do thi: {e}")))?;
    let updated_at = lvc_engine::now_unix();
    WorkflowRepo::new(&ctx.db)
        .upsert(&id, name, &json, updated_at)
        .await?;
    Ok(WorkflowMeta {
        id,
        name: name.to_string(),
        updated_at,
        node_count: u32::try_from(graph.nodes.len()).unwrap_or(0),
    })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (String/Graph) - bien gioi IPC, khong the doi sang tham chieu"
)]
#[tauri::command]
pub async fn workflow_save(
    state: tauri::State<'_, AppState>,
    id: Option<String>,
    name: String,
    graph: Graph,
) -> Result<WorkflowMeta, IpcError> {
    workflow_save_impl(&state, id, &name, &graph).await
}

/// # Errors
/// `IpcError` ma `STORE` khi truy van that bai.
pub async fn workflow_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
) -> Result<Vec<WorkflowMeta>, IpcError> {
    Ok(WorkflowRepo::new(&ctx.db).list_meta().await?)
}

#[tauri::command]
pub async fn workflow_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<WorkflowMeta>, IpcError> {
    workflow_list_impl(&state).await
}

/// # Errors
/// `IpcError` ma `WORKFLOW_NOT_FOUND` khi khong co dong khop `id`; `WORKFLOW_CORRUPT` khi cot
/// `json` khong con la `Graph` hop le (vd bi sua tay); `STORE` khi truy van that bai.
pub async fn workflow_get_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<WorkflowDoc, IpcError> {
    let row = WorkflowRepo::new(&ctx.db).get(id).await?.ok_or_else(|| {
        IpcError::new(
            "WORKFLOW_NOT_FOUND",
            format!("khong tim thay workflow {id}"),
        )
    })?;
    let graph: Graph = serde_json::from_str(&row.json)
        .map_err(|e| IpcError::new("WORKFLOW_CORRUPT", format!("do thi da luu bi hong: {e}")))?;
    // P4a: workflow luu tu P3 (v1, kind `video`) -> dinh dang hien tai.
    let graph = migrate(graph);
    Ok(WorkflowDoc {
        meta: WorkflowMeta {
            id: row.id,
            name: row.name,
            updated_at: row.updated_at,
            node_count: u32::try_from(graph.nodes.len()).unwrap_or(0),
        },
        graph,
    })
}

#[tauri::command]
pub async fn workflow_get(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<WorkflowDoc, IpcError> {
    workflow_get_impl(&state, &id).await
}

/// # Errors
/// `IpcError` ma `STORE` khi ghi bang `workflows` that bai.
pub async fn workflow_delete_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<bool, IpcError> {
    // Xoá luôn dòng nguồn gốc (nếu có): còn sót thì "Cập nhật workflow" sẽ báo bản mới cho một
    // workflow đã không còn tồn tại. Xoá TRƯỚC, vì bỏ sót một dòng trong bảng phụ còn nhẹ hơn là
    // xoá được dòng phụ nhưng workflow chính vẫn nằm đó.
    lvc_store::WorkflowOriginRepo::new(&ctx.db)
        .remove(id)
        .await?;
    Ok(WorkflowRepo::new(&ctx.db).delete(id).await?)
}

#[tauri::command]
pub async fn workflow_delete(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<bool, IpcError> {
    workflow_delete_impl(&state, &id).await
}

/// Than logic `run_start` — validate LAI qua chinh `Engine::run_graph` (khong goi `plan` rieng o
/// day: `run_graph` da goi `plan` ben trong va tra `EngineError::Graph` neu khong hop le, tranh
/// logic kiem tra kep lech nhau). `hooks_factory` = [`noop_hooks_factory`] — engine da tu phat
/// `TaskStage`/`NodeState`/`RunDone` qua `EngineEvents` (bridge Task 8), UI khong can hooks rieng.
///
/// Fix-round 1 (I1): `ctx.runs` CHI giu `CancellationToken` (huy nhanh, khong doc lai duoc trang
/// thai tu do) — `Engine` da tu quan ly `RunHandle`/`JoinHandle` that su qua `run_tokens` rieng
/// (`dag.rs`), giu ca hai ban se trung lap ma khong them gia tri. Spawn MOT task NEN "reaper" doi
/// `join` roi don map — chay tren CA HAI duong ket thuc (hoan tat tu nhien LAN bi huy), khong chi
/// khi ai do goi `run_cancel`; panic cua than vong chay (hiem — da bat trong o `run_graph_task`
/// nen day chi la phong thu kep) duoc log `warn` thay vi nuot lang le.
///
/// # Errors
/// `IpcError` ma `GRAPH_INVALID` (kem `details`) khi do thi khong hop le.
pub async fn run_start_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    workflow_id: &str,
    graph: &Graph,
) -> Result<RunInfo, IpcError> {
    // I1 (fix-round 1, P3c Task 1): `..` bo qua `tasks_ready` (moi, `lvc_engine::scripts::
    // run_script` dung, `run_start`/UI workflow thuong khong can).
    let RunHandle {
        run_id,
        token,
        join,
        ..
    } = ctx
        .engine
        .run_graph(workflow_id, graph, noop_hooks_factory())
        .await?;
    ctx.runs.insert(run_id.clone(), token);

    let runs = Arc::clone(&ctx.runs);
    let reaper_id = run_id.clone();
    tokio::spawn(async move {
        if let Err(e) = join.await {
            warn!(run_id = %reaper_id, error = %e, "run task ket thuc bat thuong (panic) - van don khoi map");
        }
        runs.remove(&reaper_id);
    });

    Ok(RunInfo { run_id })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (String/Graph) - bien gioi IPC, khong the doi sang tham chieu"
)]
#[tauri::command]
pub async fn run_start(
    license: tauri::State<'_, super::license::LicenseState>,
    state: tauri::State<'_, AppState>,
    workflow_id: String,
    graph: Graph,
) -> Result<RunInfo, IpcError> {
    license.require_licensed()?;
    run_start_impl(&state, &workflow_id, &graph).await
}

/// Than logic `run_cancel` — boc thang `Engine::cancel_run` (dong bo, xem doc do). Huy chi bao
/// token; chua chac run da THUC SU dung ngay khi ham nay tra ve (buoc dang chay chay tiep den diem
/// kiem huy ke tiep). Fix-round 1 (I1): KHONG con dong gi vao `ctx.runs` o day nua — task "reaper"
/// spawn tu `run_start_impl` da tu don map tren CA HAI duong ket thuc (hoan tat tu nhien lan bi
/// huy), nen `run_cancel` khong can biet gi ve map ca.
#[must_use = "gia tri tra ve la co biet duoc run_id nay hay khong (false = khong biet)"]
pub fn run_cancel_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>, run_id: &str) -> bool {
    ctx.engine.cancel_run(run_id)
}

#[allow(
    clippy::unused_async,
    reason = "giu async dong bo voi moi lenh IPC khac (quy uoc brief Task 9); than logic dong bo"
)]
#[tauri::command]
pub async fn run_cancel(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<bool, IpcError> {
    Ok(run_cancel_impl(&state, &run_id))
}

/// # Errors
/// `IpcError` ma `STORE` khi truy van that bai.
pub async fn run_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    limit: i64,
) -> Result<Vec<RunRow>, IpcError> {
    Ok(RunRepo::new(&ctx.db).list_recent(limit).await?)
}

#[tauri::command]
pub async fn run_list(
    state: tauri::State<'_, AppState>,
    limit: i64,
) -> Result<Vec<RunRow>, IpcError> {
    run_list_impl(&state, limit).await
}

/// Run-robustness B: trang thai MUC NODE cua mot run (bang `node_states`) — UI khoi phuc lai
/// `nodes` (video da xong / node loi + ly do) khi mo lai app/tab.
///
/// # Errors
/// `IpcError` ma `STORE` khi truy van that bai.
pub async fn run_node_states_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    run_id: &str,
) -> Result<Vec<NodeStateRow>, IpcError> {
    Ok(NodeStateRepo::new(&ctx.db).list_for_run(run_id).await?)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (String) - bien gioi IPC"
)]
#[tauri::command]
pub async fn run_node_states(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<Vec<NodeStateRow>, IpcError> {
    run_node_states_impl(&state, &run_id).await
}

/// Phần C: chạy lại CHỈ node lỗi + node phía sau (tự ghép), tái dùng output node đã xong (KHÔNG
/// tốn credit lại). Đọc `node_states` của run cũ → `failed`; `rerun = descendants(failed)`; seed
/// output các node đã xong NGOÀI `rerun` (và còn trong graph) → `run_graph_seeded` (run MỚI).
///
/// # Errors
/// `IpcError` mã `NO_FAILED_NODES` khi run cũ không có node lỗi; `STORE`/`GRAPH_INVALID` khác.
pub async fn run_retry_failed_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    run_id: &str,
    graph: &Graph,
) -> Result<RunInfo, IpcError> {
    use std::collections::{BTreeSet, HashMap, HashSet};

    let ns = NodeStateRepo::new(&ctx.db);
    let states = ns.list_for_run(run_id).await?;
    let failed: BTreeSet<String> = states
        .iter()
        .filter(|s| s.state == "failed")
        .map(|s| s.node_id.clone())
        .collect();
    if failed.is_empty() {
        return Err(IpcError::new(
            "NO_FAILED_NODES",
            "không có node lỗi để chạy lại",
        ));
    }
    let g = migrate(graph.clone());
    let rerun = lvc_core::graph::descendants(&g, &failed);
    let in_graph: HashSet<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();

    // Seed output node da xong (ngoai rerun, con trong graph). Node seed hong JSON -> bo (se chay lai).
    let mut seed: HashMap<String, lvc_node_api::Outputs> = HashMap::new();
    for (node_id, json) in ns.outputs_for_run(run_id).await? {
        if rerun.contains(&node_id) || !in_graph.contains(node_id.as_str()) {
            continue;
        }
        if let Ok(outputs) = serde_json::from_str::<lvc_node_api::Outputs>(&json) {
            seed.insert(node_id, outputs);
        }
    }
    let skip: HashSet<String> = seed.keys().cloned().collect();

    // Run moi cung workflow voi run cu (de re-attach/lich su nhat quan).
    let workflow_id = RunRepo::new(&ctx.db)
        .get(run_id)
        .await?
        .map(|r| r.workflow_id)
        .unwrap_or_default();

    let RunHandle {
        run_id: new_run_id,
        token,
        join,
        ..
    } = ctx
        .engine
        .run_graph_seeded(&workflow_id, &g, noop_hooks_factory(), seed, skip)
        .await?;
    ctx.runs.insert(new_run_id.clone(), token);
    let runs = Arc::clone(&ctx.runs);
    let reaper_id = new_run_id.clone();
    tokio::spawn(async move {
        if let Err(e) = join.await {
            warn!(run_id = %reaper_id, error = %e, "run retry ket thuc bat thuong (panic)");
        }
        runs.remove(&reaper_id);
    });
    Ok(RunInfo { run_id: new_run_id })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (String/Graph) - bien gioi IPC"
)]
#[tauri::command]
pub async fn run_retry_failed(
    license: tauri::State<'_, super::license::LicenseState>,
    state: tauri::State<'_, AppState>,
    run_id: String,
    graph: Graph,
) -> Result<RunInfo, IpcError> {
    license.require_licensed()?;
    run_retry_failed_impl(&state, &run_id, &graph).await
}

/// Lấy đường dẫn media (ảnh/video) ĐẦU TIÊN trong một `Outputs` — "đại diện" preview của node.
fn first_media_path(outputs: &lvc_node_api::Outputs) -> Option<String> {
    outputs.values().flatten().find_map(|v| match v {
        lvc_node_api::Value::Image(a) | lvc_node_api::Value::Video(a) => {
            Some(a.path.to_string_lossy().into_owned())
        }
        lvc_node_api::Value::Text(_) | lvc_node_api::Value::Audio(_) => None,
    })
}

/// Run-robustness B: map `node_id -> duong dan output` de khoi phuc preview anh/video khi mo lai
/// app. UU TIEN duong dan THAT da luu trong `node_states.outputs` (JSON `Outputs`) — ben voi MOI
/// quy uoc ten file (`MuseAI` luu ten server rieng `media-generation-...`, KHONG theo
/// `{run}_{node}_{scene}`, nen quet dia theo ten se bo sot). Fallback quet dia cho node chua co
/// outputs luu (run cu truoc migration 0013).
///
/// # Errors
/// `IpcError` ma `STORE` khi doc `node_states` that bai.
pub async fn run_node_outputs_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    run_id: &str,
) -> Result<std::collections::HashMap<String, String>, IpcError> {
    use std::collections::HashMap;
    let ns = NodeStateRepo::new(&ctx.db);
    let mut map: HashMap<String, String> = HashMap::new();
    for (node_id, json) in ns.outputs_for_run(run_id).await? {
        if let Ok(outputs) = serde_json::from_str::<lvc_node_api::Outputs>(&json) {
            if let Some(path) = first_media_path(&outputs) {
                map.insert(node_id, path);
            }
        }
    }
    // Fallback: node da xong nhung khong co outputs luu -> quet dia theo ten tat dinh.
    let missing: Vec<String> = ns
        .list_for_run(run_id)
        .await?
        .into_iter()
        .map(|r| r.node_id)
        .filter(|n| !map.contains_key(n))
        .collect();
    if !missing.is_empty() {
        for (n, p) in ctx.engine.node_outputs(run_id, &missing) {
            map.entry(n).or_insert(p);
        }
    }
    Ok(map)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (String) - bien gioi IPC"
)]
#[tauri::command]
pub async fn run_node_outputs(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<std::collections::HashMap<String, String>, IpcError> {
    run_node_outputs_impl(&state, &run_id).await
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeStep;
    use lvc_browser::dola::ConversationId;
    use lvc_browser::dola::poll::{NOTE_RAW, VideoArtifact};
    use lvc_core::graph::{Edge, GraphError, Node, NodeData};
    use lvc_core::{LogoMode, Mode, Model, Ratio};
    use lvc_store::RunStatus;

    use super::*;
    use crate::commands::test_support::ctx_with;

    fn node(id: &str, kind: NodeData) -> Node {
        Node {
            id: id.to_string(),
            kind,
            x: 0.0,
            y: 0.0,
        }
    }

    fn edge(id: &str, from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
        Edge {
            id: id.to_string(),
            from: from.to_string(),
            from_port: from_port.to_string(),
            to: to.to_string(),
            to_port: to_port.to_string(),
        }
    }

    fn video_kind() -> NodeData {
        NodeData::new(
            "dola_video",
            serde_json::json!({ "mode": Mode::Fast, "model": Model::Seedance20, "ratio": Ratio::R9x16, "duration": 10, "logo": LogoMode::Off, "account": String::new() }),
        )
    }

    /// Do thi 3 canh Prompt -> Tao video -> Luu (dung o nhieu test: `validate`/`run_start`).
    fn three_scene_graph(out_dir: &std::path::Path) -> Graph {
        Graph {
            version: 2,
            nodes: vec![
                node(
                    "pr",
                    NodeData::new(
                        "prompt",
                        serde_json::json!({ "text": "canh 1\ncanh 2\ncanh 3".to_string() }),
                    ),
                ),
                node("vid", video_kind()),
                node(
                    "sv",
                    NodeData::new(
                        "save",
                        serde_json::json!({ "out_dir": out_dir.display().to_string() }),
                    ),
                ),
            ],
            edges: vec![
                edge("e1", "pr", "out", "vid", "prompt"),
                edge("e2", "vid", "out", "sv", "video"),
            ],
        }
    }

    fn artifact(local: &std::path::Path) -> VideoArtifact {
        VideoArtifact {
            video_url: "https://v.example/x.mp4".into(),
            local_path: local.to_path_buf(),
            conversation_id: ConversationId("c".into()),
            account: String::new(),
            watermarked: false,
            bytes: 5,
            note: NOTE_RAW,
        }
    }

    fn happy_steps(n: usize, src: &std::path::Path) -> Vec<FakeStep> {
        let mut v = Vec::new();
        for i in 0..n {
            v.push(FakeStep::Launch(Ok(())));
            v.push(FakeStep::Submit(Ok(ConversationId(format!("c{i}")))));
            v.push(FakeStep::Poll(Ok(artifact(src))));
        }
        v
    }

    #[test]
    fn workflow_validate_do_thi_canh_treo_bao_loi_co_cau_truc() {
        let g = Graph {
            version: 2,
            nodes: vec![node(
                "pr",
                NodeData::new(
                    "prompt",
                    serde_json::json!({ "text": "canh 1".to_string() }),
                ),
            )],
            edges: vec![edge("e1", "pr", "out", "khong-ton-tai", "prompt")],
        };
        let err =
            workflow_validate_impl(&g, &crate::registry::build_registry(), &Limits::default())
                .unwrap_err();
        assert_eq!(err.code, "GRAPH_INVALID");
        let details = err.details.expect("phai co details");
        let errs: Vec<GraphError> = serde_json::from_value(details).unwrap();
        assert!(
            errs.contains(&GraphError::UnknownNode {
                id: "khong-ton-tai".to_string()
            }),
            "{errs:?}"
        );
    }

    #[test]
    fn workflow_validate_do_thi_3_canh_video_total_3() {
        let dir = tempfile::tempdir().unwrap();
        let g = three_scene_graph(&dir.path().join("out"));
        let summary =
            workflow_validate_impl(&g, &crate::registry::build_registry(), &Limits::default())
                .unwrap();
        assert_eq!(summary.video_total, 3);
        assert!(summary.warnings.is_empty());
    }

    #[tokio::test]
    async fn workflow_save_get_list_delete_round_trip() {
        let (dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let g = three_scene_graph(&dir.path().join("out"));

        let meta = workflow_save_impl(&ctx, None, "Demo", &g).await.unwrap();
        assert_eq!(meta.name, "Demo");
        assert!(!meta.id.is_empty());

        let doc = workflow_get_impl(&ctx, &meta.id).await.unwrap();
        assert_eq!(doc.meta.id, meta.id);
        assert_eq!(doc.graph, g);

        let list = workflow_list_impl(&ctx).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, meta.id);

        // Luu lai voi CUNG id (sua ten) -> upsert, khong tao dong moi.
        let meta2 = workflow_save_impl(&ctx, Some(meta.id.clone()), "Demo 2", &g)
            .await
            .unwrap();
        assert_eq!(meta2.id, meta.id);
        let list2 = workflow_list_impl(&ctx).await.unwrap();
        assert_eq!(list2.len(), 1, "upsert khong tao them dong");
        assert_eq!(list2[0].name, "Demo 2");

        let deleted = workflow_delete_impl(&ctx, &meta.id).await.unwrap();
        assert!(deleted);
        assert!(workflow_get_impl(&ctx, &meta.id).await.is_err());
        let deleted_again = workflow_delete_impl(&ctx, &meta.id).await.unwrap();
        assert!(!deleted_again, "id da xoa -> false");
    }

    #[tokio::test]
    async fn workflow_get_id_khong_ton_tai_bao_loi_rieng() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = workflow_get_impl(&ctx, "ghost").await.unwrap_err();
        assert_eq!(err.code, "WORKFLOW_NOT_FOUND");
    }

    #[tokio::test]
    async fn run_start_tao_run_va_3_task_run_list_thay() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();

        let (dir, ctx, _driver) = ctx_with(happy_steps(3, &src), &["acc1", "acc2", "acc3"]).await;
        let g = three_scene_graph(&dir.path().join("out"));

        let info = run_start_impl(&ctx, "wf1", &g).await.unwrap();
        assert!(!info.run_id.is_empty());
        assert!(
            ctx.runs.contains_key(&info.run_id),
            "run_start phai chen RunHandle vao map"
        );

        // Doi run chay xong (poll join truc tiep khong the vi da chen vao map - doi qua run_list).
        for _ in 0..200 {
            let rows = run_list_impl(&ctx, 10).await.unwrap();
            if rows
                .iter()
                .any(|r| r.id == info.run_id && r.status != RunStatus::Running)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let rows = run_list_impl(&ctx, 10).await.unwrap();
        let row = rows
            .iter()
            .find(|r| r.id == info.run_id)
            .expect("run phai co trong run_list");
        assert_eq!(row.status, RunStatus::Completed, "{row:?}");

        let tasks = lvc_store::TaskRepo::new(&ctx.db)
            .list_by_run(&info.run_id)
            .await
            .unwrap();
        assert_eq!(tasks.len(), 3);

        // I1 (fix-round 1): run hoan tat TU NHIEN (khong ai goi run_cancel) van phai duoc "reaper"
        // don khoi ctx.runs — doi ngan vi task nen co the chay tre mot nhip sau khi run_list da
        // thay trang thai cuoi (ca hai deu phu thuoc `join` nhung reaper con phai duoc lich lai).
        for _ in 0..200 {
            if !ctx.runs.contains_key(&info.run_id) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            !ctx.runs.contains_key(&info.run_id),
            "reaper phai don run da hoan tat tu nhien khoi map"
        );
    }

    /// Run-robustness B: sau khi run xong, `run_node_states` tra dong `node_states` (trang thai muc
    /// node) + `run_node_outputs` quet dia tra map node->file cho node co output.
    #[tokio::test]
    async fn run_node_states_va_outputs_sau_khi_chay() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();

        let (dir, ctx, _driver) = ctx_with(happy_steps(3, &src), &["acc1", "acc2", "acc3"]).await;
        let g = three_scene_graph(&dir.path().join("out"));
        let info = run_start_impl(&ctx, "wf1", &g).await.unwrap();

        for _ in 0..200 {
            let rows = run_list_impl(&ctx, 10).await.unwrap();
            if rows
                .iter()
                .any(|r| r.id == info.run_id && r.status != RunStatus::Running)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let states = run_node_states_impl(&ctx, &info.run_id).await.unwrap();
        let mut node_ids: Vec<String> = states.iter().map(|s| s.node_id.clone()).collect();
        node_ids.sort();
        assert_eq!(node_ids, vec!["pr", "sv", "vid"]);
        assert!(states.iter().all(|s| s.state == "done"), "{states:?}");

        // Preview khoi phuc UU TIEN duong dan THAT trong node_states.outputs (ben voi moi ten file,
        // ke ca ten server MuseAI). Node video da done -> co duong dan output de restore preview.
        let outs = run_node_outputs_impl(&ctx, &info.run_id).await.unwrap();
        assert!(
            outs.get("vid").is_some_and(|p| !p.is_empty()),
            "vid phai co duong dan output de khoi phuc preview: {outs:?}"
        );
    }

    /// Phần C: chạy lại node lỗi — chạy thật 1 lần (pr/vid/sv done + lưu Outputs), giả lập `vid`
    /// lỗi, rồi `run_retry_failed` → run MỚI hoàn tất, pr được SEED (không chạy lại), vid+sv chạy
    /// lại tái dùng output pr.
    #[tokio::test]
    async fn run_retry_failed_chay_lai_node_loi_ra_run_hoan_tat() {
        async fn wait_done<D: BrowserDriver + 'static>(ctx: &Ctx<D>, run_id: &str) {
            for _ in 0..300 {
                let rows = run_list_impl(ctx, 20).await.unwrap();
                if rows
                    .iter()
                    .any(|r| r.id == run_id && r.status != RunStatus::Running)
                {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            panic!("run {run_id} khong xong");
        }

        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();
        // Chi lan chay DAU dung driver (3 canh video). Retry re-chay CHI node `sv` (Luu — khong
        // dung driver), con `pr`/`vid` bi SKIP (seed lai) -> retry KHONG goi driver, khong ton
        // credit lai cho video, va khong dinh phai cache phien giua 2 run.
        let (dir, ctx, driver) = ctx_with(happy_steps(3, &src), &["acc1", "acc2", "acc3"]).await;
        let g = three_scene_graph(&dir.path().join("out"));

        let info = run_start_impl(&ctx, "wf1", &g).await.unwrap();
        wait_done(&ctx, &info.run_id).await;
        let calls_sau_run_dau = driver.calls.lock().unwrap().len();

        // Gia lap `sv` (Luu) loi o lan chay truoc (pr/vid van done + co Outputs da luu).
        let ns = lvc_store::NodeStateRepo::new(&ctx.db);
        ns.upsert(
            &info.run_id,
            "sv",
            "failed",
            (1, 1, 0, 1),
            Some("gia lap loi"),
            9.0,
        )
        .await
        .unwrap();

        let retry = run_retry_failed_impl(&ctx, &info.run_id, &g).await.unwrap();
        assert_ne!(retry.run_id, info.run_id, "retry la run moi");
        wait_done(&ctx, &retry.run_id).await;

        let row = run_list_impl(&ctx, 20)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == retry.run_id)
            .expect("run retry phai co trong run_list");
        assert_eq!(row.status, RunStatus::Completed, "{row:?}");

        // `vid` (video, dat tien) bi SKIP khi retry -> driver KHONG duoc goi them (khong ton credit).
        assert_eq!(
            driver.calls.lock().unwrap().len(),
            calls_sau_run_dau,
            "retry KHONG duoc goi driver lai (vid phai skip)"
        );

        let states = ns.list_for_run(&retry.run_id).await.unwrap();
        for n in ["pr", "vid", "sv"] {
            assert!(
                states.iter().any(|s| s.node_id == n && s.state == "done"),
                "{n} phai done: {states:?}"
            );
        }
    }

    /// Phần C (final-review Critical 1): CHAINED retry — retry → vẫn lỗi → retry lại. Node đắt
    /// (`vid`) đã xong phải được seed+skip ở CẢ HAI lần retry, driver KHÔNG được gọi lại (không tốn
    /// credit lại) dù qua nhiều lần retry. Bắt lỗi "skip không ghi `set_outputs` cho run mới".
    #[tokio::test]
    async fn run_retry_failed_chained_khong_re_chay_node_dat() {
        async fn wait_done<D: BrowserDriver + 'static>(ctx: &Ctx<D>, run_id: &str) {
            for _ in 0..300 {
                let rows = run_list_impl(ctx, 30).await.unwrap();
                if rows
                    .iter()
                    .any(|r| r.id == run_id && r.status != RunStatus::Running)
                {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            panic!("run {run_id} khong xong");
        }

        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();
        // CHI lan chay dau duoc phep dung driver (3 canh). Neu retry re-chay vid -> het step/sai.
        let (dir, ctx, driver) = ctx_with(happy_steps(3, &src), &["acc1", "acc2", "acc3"]).await;
        let g = three_scene_graph(&dir.path().join("out"));
        let ns = lvc_store::NodeStateRepo::new(&ctx.db);

        let r1 = run_start_impl(&ctx, "wf1", &g).await.unwrap();
        wait_done(&ctx, &r1.run_id).await;
        let calls = driver.calls.lock().unwrap().len();
        assert!(calls > 0, "lan dau phai chay vid");

        // Retry lan 1: gia lap sv loi.
        ns.upsert(&r1.run_id, "sv", "failed", (1, 1, 0, 1), Some("x"), 9.0)
            .await
            .unwrap();
        let r2 = run_retry_failed_impl(&ctx, &r1.run_id, &g).await.unwrap();
        wait_done(&ctx, &r2.run_id).await;

        // Retry lan 2: sv lai loi trong run2 -> retry tiep.
        ns.upsert(&r2.run_id, "sv", "failed", (1, 1, 0, 1), Some("x"), 9.0)
            .await
            .unwrap();
        let r3 = run_retry_failed_impl(&ctx, &r2.run_id, &g).await.unwrap();
        wait_done(&ctx, &r3.run_id).await;

        // vid (dat tien) KHONG bao gio chay lai qua 2 lan retry -> driver.calls khong doi.
        assert_eq!(
            driver.calls.lock().unwrap().len(),
            calls,
            "retry nhieu lan KHONG duoc re-chay node da xong (vid)"
        );
        assert_eq!(
            run_list_impl(&ctx, 30)
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.id == r3.run_id)
                .unwrap()
                .status,
            RunStatus::Completed
        );
    }

    /// Phần C: không có node lỗi -> lệnh trả lỗi rõ ràng (phòng thủ; UI không hiện nút thì không tới).
    #[tokio::test]
    async fn run_retry_failed_khong_co_node_loi_bao_loi() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let rr = RunRepo::new(&ctx.db);
        rr.create("r1", "wf1", RunStatus::Completed, 1.0)
            .await
            .unwrap();
        let g = three_scene_graph(std::path::Path::new("out"));
        let err = run_retry_failed_impl(&ctx, "r1", &g).await.unwrap_err();
        assert_eq!(err.code, "NO_FAILED_NODES");
    }

    #[tokio::test]
    async fn run_start_do_thi_khong_hop_le_tra_graph_invalid() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let g = Graph {
            version: 2,
            nodes: vec![],
            edges: vec![],
        };
        let err = run_start_impl(&ctx, "wf1", &g).await.unwrap_err();
        assert_eq!(err.code, "GRAPH_INVALID");
        assert!(ctx.runs.is_empty(), "khong tao run khi do thi khong hop le");
    }

    #[tokio::test]
    async fn run_cancel_tra_true_roi_run_ket_thuc_cancelled() {
        let steps = vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(ConversationId("c1".into()))),
            FakeStep::PollUntilCancelled,
        ];
        let (dir, ctx, _driver) = ctx_with(steps, &["acc1", "acc2", "acc3"]).await;
        let g = three_scene_graph(&dir.path().join("out"));

        let mut rx = ctx.engine.events().subscribe();
        let info = run_start_impl(&ctx, "wf1", &g).await.unwrap();

        // Doi canh dau vao "generating" (dang poll) truoc khi huy - tranh huy qua som luc con o
        // limiter/lease (dan lai flakiness giong lvc-engine::dag test, xem doc do).
        loop {
            let ev = lvc_engine::events::next_event(&mut rx).await.unwrap();
            if let lvc_engine::EngineEvent::TaskStage { stage, .. } = &ev {
                if stage == "generating" {
                    break;
                }
            }
        }

        let cancelled = run_cancel_impl(&ctx, &info.run_id);
        assert!(cancelled);

        for _ in 0..200 {
            let rows = run_list_impl(&ctx, 10).await.unwrap();
            if rows
                .iter()
                .any(|r| r.id == info.run_id && r.status != RunStatus::Running)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let rows = run_list_impl(&ctx, 10).await.unwrap();
        let row = rows.iter().find(|r| r.id == info.run_id).unwrap();
        assert_eq!(row.status, RunStatus::Cancelled, "{row:?}");

        // I1 (fix-round 1): `run_cancel_impl` KHONG con dong gi vao map — cho "reaper" (spawn tu
        // `run_start_impl`) don sau khi `join` xong, doi ngan giong test hoan tat tu nhien o tren.
        for _ in 0..200 {
            if !ctx.runs.contains_key(&info.run_id) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(
            !ctx.runs.contains_key(&info.run_id),
            "reaper phai don run da bi huy khoi map"
        );
    }

    #[tokio::test]
    async fn run_cancel_run_id_khong_biet_tra_false() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        assert!(!run_cancel_impl(&ctx, "khong-ton-tai"));
    }
}
