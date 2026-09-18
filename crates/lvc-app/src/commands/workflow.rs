//! Lenh workflow/run (Task 9): luu/doc/xoa workflow (bang `workflows`), bat dau/huy mot lan chay
//! do thi (`Engine::run_graph`/`cancel_run`, Task 7), liet ke run gan day (bang `runs`). Xem quy
//! uoc `xxx_impl`/`xxx` o dau `commands/mod.rs`.

use std::sync::Arc;

use lvc_browser::BrowserDriver;
use lvc_core::Limits;
use lvc_core::graph::{Graph, plan};
use lvc_engine::{EngineError, RunHandle, noop_hooks_factory};
use lvc_store::{RunRepo, RunRow, WorkflowMeta, WorkflowRepo};
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
pub fn workflow_validate_impl(graph: &Graph, limits: &Limits) -> Result<PlanSummary, IpcError> {
    match plan(graph, limits) {
        Ok(run_plan) => Ok(PlanSummary {
            video_total: run_plan.video_total,
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
    workflow_validate_impl(&graph, state.engine.limits())
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
    let json = serde_json::to_string(graph)
        .map_err(|e| IpcError::new("GRAPH_ENCODE", format!("khong ma hoa duoc do thi: {e}")))?;
    let updated_at = lvc_engine::now_unix();
    WorkflowRepo::new(&ctx.db)
        .upsert(&id, name, &json, updated_at)
        .await?;
    Ok(WorkflowMeta {
        id,
        name: name.to_string(),
        updated_at,
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
    Ok(WorkflowDoc {
        meta: WorkflowMeta {
            id: row.id,
            name: row.name,
            updated_at: row.updated_at,
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
    state: tauri::State<'_, AppState>,
    workflow_id: String,
    graph: Graph,
) -> Result<RunInfo, IpcError> {
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

#[cfg(test)]
mod tests {
    use lvc_browser::FakeStep;
    use lvc_browser::dola::ConversationId;
    use lvc_browser::dola::poll::{NOTE_RAW, VideoArtifact};
    use lvc_core::graph::{Edge, GraphError, Node, NodeKind};
    use lvc_core::{LogoMode, Mode, Model, Ratio};
    use lvc_store::RunStatus;

    use super::*;
    use crate::commands::test_support::ctx_with;

    fn node(id: &str, kind: NodeKind) -> Node {
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

    fn video_kind() -> NodeKind {
        NodeKind::Video {
            mode: Mode::Fast,
            model: Model::Seedance20,
            ratio: Ratio::R9x16,
            duration: 10,
            logo: LogoMode::Off,
            account: String::new(),
        }
    }

    /// Do thi 3 canh Prompt -> Tao video -> Luu (dung o nhieu test: `validate`/`run_start`).
    fn three_scene_graph(out_dir: &std::path::Path) -> Graph {
        Graph {
            version: 1,
            nodes: vec![
                node(
                    "pr",
                    NodeKind::Prompt {
                        text: "canh 1\ncanh 2\ncanh 3".to_string(),
                    },
                ),
                node("vid", video_kind()),
                node(
                    "sv",
                    NodeKind::Save {
                        out_dir: out_dir.display().to_string(),
                    },
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
            version: 1,
            nodes: vec![node(
                "pr",
                NodeKind::Prompt {
                    text: "canh 1".to_string(),
                },
            )],
            edges: vec![edge("e1", "pr", "out", "khong-ton-tai", "prompt")],
        };
        let err = workflow_validate_impl(&g, &Limits::default()).unwrap_err();
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
        let summary = workflow_validate_impl(&g, &Limits::default()).unwrap();
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

    #[tokio::test]
    async fn run_start_do_thi_khong_hop_le_tra_graph_invalid() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let g = Graph {
            version: 1,
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
