//! Lenh task (Task 9): liet ke/huy/tiep tuc mot task, mo thu muc chua file video. [`TaskDto`]
//! KHONG mang `prompt` day du (co the dai, UI danh sach khong can) — chi `prompt_preview` (80 ky tu
//! dau). Xem quy uoc `xxx_impl`/`xxx` o dau `commands/mod.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lvc_browser::BrowserDriver;
use lvc_browser::dola::NoopHooks;
use lvc_core::TaskStatus;
use lvc_store::{TaskRepo, TaskRow};
use serde::{Deserialize, Serialize};
use tauri_plugin_shell::ShellExt;
use tracing::warn;

use super::{Ctx, IpcError};
use crate::state::AppState;

/// So ky tu toi da cua `prompt_preview` (xem doc module).
const PROMPT_PREVIEW_MAX_CHARS: usize = 80;
/// `TaskRepo::list_recent`/`list_by_run` kep tren; `task_list_impl` dung LAI cung khoang de hai
/// nhanh (co/khong `run_id`) nhat quan.
const TASK_LIST_LIMIT_MIN: i64 = 1;
const TASK_LIST_LIMIT_MAX: i64 = 200;

/// DTO cho danh sach task — KHONG co `prompt` day du (xem doc module).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    pub id: String,
    pub prompt_preview: String,
    pub status: String,
    pub failure_code: Option<String>,
    pub error: Option<String>,
    pub account: Option<String>,
    pub mode: String,
    pub duration: i64,
    pub ratio: String,
    pub model: String,
    pub local_path: Option<String>,
    pub scene_index: Option<i64>,
    pub run_id: String,
    pub node_id: String,
    pub stage: String,
    pub created_at: f64,
    pub started_at: Option<f64>,
    pub finished_at: Option<f64>,
    /// `true` khi `TaskRow::conversation_id` co mat va khac rong — frontend dung de quyet dinh co
    /// bat nut "Tiep tuc" (`task_resume`) hay khong, KHONG can biet chinh id (khong ly do de lo).
    pub has_conversation: bool,
}

impl TaskDto {
    #[must_use]
    pub fn from_row(row: &TaskRow) -> Self {
        Self {
            id: row.id.clone(),
            prompt_preview: row.prompt.chars().take(PROMPT_PREVIEW_MAX_CHARS).collect(),
            status: row.status.as_str().to_string(),
            failure_code: row.failure_code.map(|c| c.as_str().to_string()),
            error: row.error.clone(),
            account: row.account.clone(),
            mode: row.mode.clone(),
            duration: row.duration,
            ratio: row.ratio.clone(),
            model: row.model.clone(),
            local_path: row.local_path.clone(),
            scene_index: row.scene_index,
            run_id: row.run_id.clone(),
            node_id: row.node_id.clone(),
            stage: row.stage.clone(),
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
            has_conversation: row
                .conversation_id
                .as_deref()
                .is_some_and(|s| !s.is_empty()),
        }
    }
}

/// Than logic `task_list`. `run_id = None` -> `TaskRepo::list_recent` (moi task gan day, khong
/// phan biet run); `run_id = Some` -> `TaskRepo::list_by_run` (khong co `limit` rieng trong repo —
/// cat sau khi doc, cac run P3 chi vai chuc task nen khong dang lo hieu nang).
///
/// **Thu tu KHAC nhau giua hai nhanh (M9, fix-round 1)** — Task 11 KHONG duoc gia dinh "moi nhat
/// truoc" cho ca hai: nhanh `run_id = None` (`list_recent`) sap `created_at` GIAM DAN (moi nhat
/// truoc); nhanh `run_id = Some` (`list_by_run`) sap `scene_index` roi `created_at` TANG DAN (canh
/// 1 truoc), va `rows.truncate(limit)` (cat sau khi doc TOAN BO task cua run — repo chua co `LIMIT`
/// rieng cho truy van nay) giu lai N canh DAU (chi so nho nhat), KHONG phai N canh moi nhat.
///
/// # Errors
/// `IpcError` ma `STORE` khi truy van that bai.
pub async fn task_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<TaskDto>, IpcError> {
    let repo = TaskRepo::new(&ctx.db);
    let limit = limit.clamp(TASK_LIST_LIMIT_MIN, TASK_LIST_LIMIT_MAX);
    let rows = match run_id {
        Some(rid) => {
            let mut rows = repo.list_by_run(rid).await?;
            let take = usize::try_from(limit).unwrap_or(usize::MAX);
            rows.truncate(take);
            rows
        }
        None => repo.list_recent(limit).await?,
    };
    Ok(rows.iter().map(TaskDto::from_row).collect())
}

#[tauri::command]
pub async fn task_list(
    state: tauri::State<'_, AppState>,
    run_id: Option<String>,
    limit: i64,
) -> Result<Vec<TaskDto>, IpcError> {
    task_list_impl(&state, run_id.as_deref(), limit).await
}

/// Than logic `task_cancel` — boc thang `Engine::cancel` (dong bo, R4). `false` = khong biet `id`
/// (chua chay/da xong).
#[must_use = "false = khong biet id nay (chua chay/da xong)"]
pub fn task_cancel_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>, id: &str) -> bool {
    ctx.engine.cancel(id)
}

#[allow(
    clippy::unused_async,
    reason = "giu async dong bo voi moi lenh IPC khac (quy uoc brief Task 9); than logic dong bo"
)]
#[tauri::command]
pub async fn task_cancel(state: tauri::State<'_, AppState>, id: String) -> Result<bool, IpcError> {
    Ok(task_cancel_impl(&state, &id))
}

/// Ket qua `task_resume` khi CHAP NHAN duoc yeu cau — chi bao lai `task_id` da nhan (giong
/// `RunInfo` cua `run_start`); lenh tra ve NGAY sau khi kiem tra qua, khong doi video xong (xem
/// doc [`task_resume_impl`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResumeInfo {
    pub task_id: String,
}

/// Than logic `task_resume`. Fix-round 1 (I2): `id` la tham so NGUOI DUNG UI go vao/lay tu danh
/// sach CU (co the da het thoi han — task da xong, hoac dang `processing` trong MOT run KHAC dang
/// chay that) nen KHONG duoc tin tuong mu quang truoc khi cham vao `Engine::resume`: goi `resume`
/// tren mot task dang `processing` se chia se token huy voi job dang chay that qua
/// `CancelRegistry::register_guarded` (get-or-insert), roi `CancelGuard::drop` cua LAN RESUME NAY
/// se XOA MAT token do khi no ket thuc — huy nham job that con dang chay. Vi vay kiem tra o day,
/// TRUOC khi spawn:
/// 1. Doc dong task — khong co -> `TASK_NOT_FOUND`.
/// 2. `status` phai la `Failed` hoac `Queued` (khac -> `Processing`/`Completed` -> `NOT_RESUMABLE`,
///    thong diep neu ro trang thai hien tai).
/// 3. Phai co `conversation_id` (khac rong) -> `NOT_RESUMABLE` khi khong (chua tung gui gi thi
///    khong co gi de "tiep tuc" — `workflow_validate`/`run_start` la duong dung cho truong hop nay).
///
/// Qua duoc ca hai thi MOI spawn `Engine::resume` chay NEN voi `NoopHooks` (engine da tu phat
/// `TaskStage`/`TaskDone`/`TaskFailed` qua `EngineEvents`, UI theo doi qua do) roi tra ve NGAY —
/// KHONG doi video xong (co the mat vai phut). Loi CUA CHINH LAN RESUME (vd `NotResumable` do
/// `account` cung thieu, loi trinh duyet/Dola...) khong tra ve tu ham nay nua — no la `TaskFailed`
/// phia UI nhan qua kenh su kien VA duoc log `warn` o day (KHONG `let _ =` nuot lang le — id sai/
/// task het thoi han la loi nguoi dung co the xay ra thuong xuyen tu UI, can vet log de tra cuu).
///
/// # Errors
/// `IpcError` ma `TASK_NOT_FOUND` (khong co dong khop `id`); `NOT_RESUMABLE` (trang thai khong
/// phai `Failed`/`Queued`, hoac thieu `conversation_id`); `STORE` khi truy van that bai.
pub async fn task_resume_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<TaskResumeInfo, IpcError> {
    let row = TaskRepo::new(&ctx.db)
        .get(id)
        .await?
        .ok_or_else(|| IpcError::new("TASK_NOT_FOUND", format!("khong tim thay task {id}")))?;

    if row.status != TaskStatus::Failed && row.status != TaskStatus::Queued {
        return Err(IpcError::new(
            "NOT_RESUMABLE",
            format!(
                "task {id} dang o trang thai {} - khong the tiep tuc",
                row.status.as_str()
            ),
        ));
    }
    if !TaskDto::from_row(&row).has_conversation {
        return Err(IpcError::new(
            "NOT_RESUMABLE",
            format!("task {id} khong co conversation_id - khong the mo lai hoi thoai"),
        ));
    }

    let engine = Arc::clone(&ctx.engine);
    let task_id = id.to_string();
    let spawn_id = task_id.clone();
    tokio::spawn(async move {
        if let Err(e) = engine.resume(&spawn_id, &NoopHooks).await {
            warn!(task_id = %spawn_id, error = %e, "task_resume: resume nen that bai");
        }
    });
    Ok(TaskResumeInfo { task_id })
}

#[tauri::command]
pub async fn task_resume(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<TaskResumeInfo, IpcError> {
    task_resume_impl(&state, &id).await
}

/// Than logic `task_open_file` — THUAN, tach khoi phan Tauri thuc su mo he dieu hanh (`ShellExt`,
/// can `AppHandle`) de test duoc khong can runtime Tauri (xem doc module dau `commands/mod.rs`).
/// Tra ve THU MUC CHUA file (khong phai chinh file — mo Explorer/Finder tai thu muc do), rong sau
/// `trim`/thieu -> loi.
///
/// # Errors
/// `IpcError` ma `NO_FILE` khi task chua co `local_path` (chua tao video xong).
pub fn task_dir_impl(local_path: Option<&str>) -> Result<PathBuf, IpcError> {
    let raw = local_path
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| IpcError::new("NO_FILE", "task chua co file video de mo"))?;
    let parent = Path::new(raw)
        .parent()
        .filter(|p| !p.as_os_str().is_empty());
    Ok(parent.map_or_else(|| PathBuf::from("."), Path::to_path_buf))
}

/// Lop Tauri MONG (khong test duoc o day — can `AppHandle` that): doc dong task, suy ra thu muc
/// chua file qua [`task_dir_impl`], roi `tauri-plugin-shell` mo thu muc do bang trinh quan ly file
/// mac dinh cua he dieu hanh.
///
/// # Errors
/// `IpcError` ma `TASK_NOT_FOUND`/`NO_FILE`/`SHELL_OPEN_FAILED`.
#[allow(
    deprecated,
    reason = "tauri-plugin-shell::Shell::open van dung tot (khuyen nghi doi sang tauri-plugin-opener nhung ngoai pham vi Task 9 - chua co dependency/plugin do trong workspace)"
)]
#[tauri::command]
pub async fn task_open_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    let row = TaskRepo::new(&state.db)
        .get(&id)
        .await?
        .ok_or_else(|| IpcError::new("TASK_NOT_FOUND", format!("khong tim thay task {id}")))?;
    let dir = task_dir_impl(row.local_path.as_deref())?;
    app.shell()
        .open(dir.display().to_string(), None)
        .map_err(|e| IpcError::new("SHELL_OPEN_FAILED", e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use lvc_core::FailureCode;

    use super::*;
    use crate::commands::test_support::ctx_with;

    fn row(id: &str, prompt: &str, run_id: &str) -> TaskRow {
        TaskRow {
            id: id.to_string(),
            prompt: prompt.to_string(),
            ratio: "9:16".to_string(),
            duration: 10,
            model: "seedance-2.0".to_string(),
            mode: "fast".to_string(),
            logo: "off".to_string(),
            status: TaskStatus::Completed,
            failure_code: None,
            error: None,
            account: Some("acc1".to_string()),
            account_pref: None,
            conversation_id: Some("conv1".to_string()),
            video_url: Some("https://v.example/x.mp4".to_string()),
            local_path: Some(r"C:\out\a.mp4".to_string()),
            reference_images: vec![],
            batch_id: None,
            batch_name: None,
            scene_index: Some(1),
            retry_count: 0,
            retry_at: 0.0,
            created_at: 1_700_000_000.0,
            updated_at: 1_700_000_000.0,
            run_id: run_id.to_string(),
            node_id: "vid".to_string(),
            stage: "done".to_string(),
            started_at: Some(1_700_000_000.0),
            finished_at: Some(1_700_000_010.0),
        }
    }

    #[test]
    fn from_row_prompt_preview_cat_80_ky_tu_khong_lo_prompt_day_du() {
        let long_prompt = "a".repeat(200);
        let r = row("t1", &long_prompt, "run1");
        let dto = TaskDto::from_row(&r);
        assert_eq!(dto.prompt_preview.chars().count(), 80);
        let v = serde_json::to_value(&dto).unwrap();
        assert!(v.get("prompt").is_none(), "{v:?}");
        assert!(v.get("promptPreview").is_some(), "{v:?}");
    }

    #[test]
    fn from_row_has_conversation_dung_theo_conversation_id() {
        let mut r = row("t1", "canh 1", "run1");
        assert!(TaskDto::from_row(&r).has_conversation);
        r.conversation_id = None;
        assert!(!TaskDto::from_row(&r).has_conversation);
        r.conversation_id = Some(String::new());
        assert!(!TaskDto::from_row(&r).has_conversation);
    }

    #[test]
    fn from_row_failure_code_dung_wire_string() {
        let mut r = row("t1", "canh 1", "run1");
        r.failure_code = Some(FailureCode::ProxyDown);
        let dto = TaskDto::from_row(&r);
        assert_eq!(dto.failure_code.as_deref(), Some("PROXY_DOWN"));
    }

    #[tokio::test]
    async fn task_list_theo_run_tra_3_dto_khong_prompt_day_du() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"x").unwrap();

        let (dir, ctx, _driver) = ctx_with(
            {
                let mut v = Vec::new();
                for i in 0..3 {
                    v.push(lvc_browser::FakeStep::Launch(Ok(())));
                    v.push(lvc_browser::FakeStep::Submit(Ok(
                        lvc_browser::dola::ConversationId(format!("c{i}")),
                    )));
                    v.push(lvc_browser::FakeStep::Poll(Ok(
                        lvc_browser::dola::poll::VideoArtifact {
                            video_url: "https://v.example/x.mp4".into(),
                            local_path: src.clone(),
                            conversation_id: lvc_browser::dola::ConversationId("c".into()),
                            account: String::new(),
                            watermarked: false,
                            bytes: 5,
                            note: lvc_browser::dola::poll::NOTE_RAW,
                        },
                    )));
                }
                v
            },
            &["acc1", "acc2", "acc3"],
        )
        .await;

        let out_dir = dir.path().join("out");
        let g = lvc_core::graph::Graph {
            version: 1,
            nodes: vec![
                lvc_core::graph::Node {
                    id: "pr".to_string(),
                    kind: lvc_core::graph::NodeKind::Prompt {
                        text: "canh 1\ncanh 2\ncanh 3".to_string(),
                    },
                    x: 0.0,
                    y: 0.0,
                },
                lvc_core::graph::Node {
                    id: "vid".to_string(),
                    kind: lvc_core::graph::NodeKind::Video {
                        mode: lvc_core::Mode::Fast,
                        model: lvc_core::Model::Seedance20,
                        ratio: lvc_core::Ratio::R9x16,
                        duration: 10,
                        logo: lvc_core::LogoMode::Off,
                        account: String::new(),
                    },
                    x: 0.0,
                    y: 0.0,
                },
                lvc_core::graph::Node {
                    id: "sv".to_string(),
                    kind: lvc_core::graph::NodeKind::Save {
                        out_dir: out_dir.display().to_string(),
                    },
                    x: 0.0,
                    y: 0.0,
                },
            ],
            edges: vec![
                lvc_core::graph::Edge {
                    id: "e1".to_string(),
                    from: "pr".to_string(),
                    from_port: "out".to_string(),
                    to: "vid".to_string(),
                    to_port: "prompt".to_string(),
                },
                lvc_core::graph::Edge {
                    id: "e2".to_string(),
                    from: "vid".to_string(),
                    from_port: "out".to_string(),
                    to: "sv".to_string(),
                    to_port: "video".to_string(),
                },
            ],
        };

        let handle = ctx
            .engine
            .run_graph("wf1", &g, lvc_engine::noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();
        handle.join.await.unwrap();

        let dtos = task_list_impl(&ctx, Some(&run_id), 10).await.unwrap();
        assert_eq!(dtos.len(), 3);
        for dto in &dtos {
            assert!(dto.prompt_preview.chars().count() <= 80);
            let v = serde_json::to_value(dto).unwrap();
            assert!(v.get("prompt").is_none(), "{v:?}");
        }
    }

    #[tokio::test]
    async fn task_cancel_id_khong_biet_tra_false() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        assert!(!task_cancel_impl(&ctx, "khong-ton-tai"));
    }

    /// 9:16, 10s, seedance-2.0, fast — canh toi thieu de tao task qua `TaskRepo::create` (giong
    /// `bootstrap::tests::scene`, nhung dung rieng o day vi `pub(crate)` cua `lvc_core::Scene`
    /// khong lo ra ngoai crate `lvc-core`).
    fn scene() -> lvc_core::Scene {
        lvc_core::Scene::normalize(
            &lvc_core::RawScene {
                prompt: Some("con meo di bo".into()),
                ..lvc_core::RawScene::default()
            },
            &lvc_core::Limits::default(),
            None,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn task_resume_id_khong_ton_tai_tra_task_not_found() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let err = task_resume_impl(&ctx, "ghost").await.unwrap_err();
        assert_eq!(err.code, "TASK_NOT_FOUND");
    }

    /// I2 (fix-round 1): task dang `processing` (trong MOT run khac dang chay that) phai bi chan
    /// TRUOC khi spawn — resume no se chia se/xoa token huy cua job that dang chay (xem doc
    /// `task_resume_impl`).
    #[tokio::test]
    async fn task_resume_task_dang_processing_tra_not_resumable() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let tasks = TaskRepo::new(&ctx.db);
        tasks.create("t1", &scene(), None).await.unwrap();
        tasks
            .set_status("t1", TaskStatus::Processing, None, None)
            .await
            .unwrap();
        tasks.set_conversation("t1", "conv1").await.unwrap();

        let err = task_resume_impl(&ctx, "t1").await.unwrap_err();
        assert_eq!(err.code, "NOT_RESUMABLE");
    }

    #[tokio::test]
    async fn task_resume_failed_khong_co_conversation_tra_not_resumable() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let tasks = TaskRepo::new(&ctx.db);
        tasks.create("t1", &scene(), None).await.unwrap();
        tasks
            .set_status("t1", TaskStatus::Failed, None, None)
            .await
            .unwrap();

        let err = task_resume_impl(&ctx, "t1").await.unwrap_err();
        assert_eq!(err.code, "NOT_RESUMABLE");
    }

    /// I2 (fix-round 1): task `Failed` + co `conversation_id`/`account` -> chap nhan, spawn
    /// `Engine::resume` chay nen (`FakeStep::Launch` + `FakeStep::Poll`, khop dung kich ban
    /// `resume.rs` dung — KHONG `Submit`, resume khong gui lai prompt) roi dong task chuyen
    /// `Completed`.
    #[tokio::test]
    async fn task_resume_failed_voi_conversation_spawn_thanh_cong_row_chuyen_completed() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"x").unwrap();

        let (_dir, ctx, _driver) = ctx_with(
            vec![
                lvc_browser::FakeStep::Launch(Ok(())),
                lvc_browser::FakeStep::Poll(Ok(lvc_browser::dola::poll::VideoArtifact {
                    video_url: "https://v.example/x.mp4".into(),
                    local_path: src.clone(),
                    conversation_id: lvc_browser::dola::ConversationId("999".into()),
                    account: String::new(),
                    watermarked: false,
                    bytes: 5,
                    note: lvc_browser::dola::poll::NOTE_RAW,
                })),
            ],
            &["a"],
        )
        .await;

        let tasks = TaskRepo::new(&ctx.db);
        tasks.create("t1", &scene(), None).await.unwrap();
        tasks
            .set_status("t1", TaskStatus::Failed, None, None)
            .await
            .unwrap();
        tasks.set_account("t1", "a").await.unwrap();
        tasks.set_conversation("t1", "999").await.unwrap();

        let info = task_resume_impl(&ctx, "t1").await.unwrap();
        assert_eq!(info.task_id, "t1");

        // Cho task chuyen trang thai (spawn nen) — poll voi timeout ngan.
        for _ in 0..500 {
            let row = tasks.get("t1").await.unwrap().unwrap();
            if row.status == TaskStatus::Completed {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let row = tasks.get("t1").await.unwrap().unwrap();
        assert_eq!(row.status, TaskStatus::Completed, "{row:?}");
    }

    #[test]
    fn task_dir_impl_khong_co_local_path_bao_no_file() {
        let err = task_dir_impl(None).unwrap_err();
        assert_eq!(err.code, "NO_FILE");
        let err2 = task_dir_impl(Some("   ")).unwrap_err();
        assert_eq!(err2.code, "NO_FILE");
    }

    #[test]
    fn task_dir_impl_tra_ve_thu_muc_cha() {
        let dir = task_dir_impl(Some(r"C:\out\a.mp4")).unwrap();
        assert_eq!(dir, PathBuf::from(r"C:\out"));
    }
}
