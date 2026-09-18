//! Lenh task (Task 9): liet ke/huy/tiep tuc mot task, mo thu muc chua file video. [`TaskDto`]
//! KHONG mang `prompt` day du (co the dai, UI danh sach khong can) — chi `prompt_preview` (80 ky tu
//! dau). Xem quy uoc `xxx_impl`/`xxx` o dau `commands/mod.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lvc_browser::BrowserDriver;
use lvc_browser::dola::NoopHooks;
use lvc_store::{TaskRepo, TaskRow};
use serde::{Deserialize, Serialize};
use tauri_plugin_shell::ShellExt;

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

/// Ket qua `task_resume` — chi bao lai `task_id` da nhan (giong `RunInfo` cua `run_start`); lenh
/// tra ve NGAY, khong doi video xong (xem doc [`task_resume_impl`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResumeInfo {
    pub task_id: String,
}

/// Than logic `task_resume`: spawn `Engine::resume` chay NEN voi `NoopHooks` (engine da tu phat
/// `TaskStage`/`TaskDone`/`TaskFailed` qua `EngineEvents`, UI theo doi qua do) roi tra ve NGAY —
/// KHONG doi video xong (co the mat vai phut). Loi resume (vd thieu `conversation_id`) khong tra ve
/// tu ham nay — no la `TaskFailed` phia UI nhan qua kenh su kien (xem doc `Engine::resume`).
pub fn task_resume_impl<D: BrowserDriver + 'static>(ctx: &Ctx<D>, id: &str) -> TaskResumeInfo {
    let engine = Arc::clone(&ctx.engine);
    let task_id = id.to_string();
    let spawn_id = task_id.clone();
    tokio::spawn(async move {
        let _ = engine.resume(&spawn_id, &NoopHooks).await;
    });
    TaskResumeInfo { task_id }
}

#[allow(
    clippy::unused_async,
    reason = "giu async dong bo voi moi lenh IPC khac (quy uoc brief Task 9); than logic dong bo"
)]
#[tauri::command]
pub async fn task_resume(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<TaskResumeInfo, IpcError> {
    Ok(task_resume_impl(&state, &id))
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
    use lvc_core::{FailureCode, TaskStatus};

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
