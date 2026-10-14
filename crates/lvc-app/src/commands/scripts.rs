//! Lenh kich ban (P3c Task 1): CRUD kich ban/canh (`script_list/create/update/delete/get/
//! put_scenes/reset_scenes`) goi THANG `lvc_store::ScriptRepo` (giong `workflow_save/get/list/
//! delete` cua `workflow.rs` goi thang `WorkflowRepo` — khong can lop `Engine` chi de CRUD tho),
//! them hai lenh can DAG/quy tac nghiep vu (`script_run`/`script_import_txt`) goi qua
//! `lvc_engine::scripts::Engine::{run_script, import_txt_to_script}`. Xem quy uoc `xxx_impl`/`xxx`
//! o dau `commands/mod.rs`.
//!
//! `SCRIPT_NOT_FOUND` duoc kiem O DAY (tang lenh), TRUOC khi goi Engine — giong
//! `workflow_get_impl`/`_script_or_404` cua Python (`scripts_api.py`): moi lenh THAO TAC tren mot
//! kich ban cu the (tru `script_list`/`script_create`) deu kiem ton tai truoc.

use lvc_browser::BrowserDriver;
use lvc_engine::ScriptRunReport;
use lvc_store::{SceneIn, SceneRow, ScriptMeta, ScriptRepo, ScriptRow};
use serde::{Deserialize, Serialize};

use super::{Ctx, IpcError};
use crate::state::AppState;

/// Mot canh + trang thai task hien tai (join voi bang `tasks` qua `task_id`, giong
/// `scripts_store.py::list_scenes` — `task_status`) — `script_get` tra ve, UI (Task 2) hien thi
/// nut Chay/Chay lai/Dung theo `task_status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneDto {
    pub id: String,
    pub idx: i64,
    pub prompt: String,
    pub ratio: Option<String>,
    pub duration: Option<i64>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub logo: Option<String>,
    pub account: String,
    pub refs: Vec<String>,
    pub task_id: Option<String>,
    pub task_status: Option<String>,
    pub failure_code: Option<String>,
    pub local_path: Option<String>,
}

/// Ket qua `script_get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptDoc {
    pub meta: ScriptMeta,
    pub scenes: Vec<SceneDto>,
}

/// Ket qua `script_reset_scenes` — `scripts_api.py:206-215 reset_scenes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetReport {
    pub reset: Vec<String>,
    pub skipped: Vec<String>,
}

fn not_found(id: &str) -> IpcError {
    IpcError::new("SCRIPT_NOT_FOUND", format!("khong tim thay kich ban {id}"))
}

async fn require_script(repo: &ScriptRepo, id: &str) -> Result<ScriptRow, IpcError> {
    repo.get(id).await?.ok_or_else(|| not_found(id))
}

/// # Errors
/// `IpcError` ma `STORE` khi truy van that bai.
pub async fn script_list_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
) -> Result<Vec<ScriptMeta>, IpcError> {
    Ok(ScriptRepo::new(&ctx.db).list_with_counts().await?)
}

#[tauri::command]
pub async fn script_list(state: tauri::State<'_, AppState>) -> Result<Vec<ScriptMeta>, IpcError> {
    script_list_impl(&state).await
}

/// # Errors
/// `IpcError` ma `STORE` khi ghi/doc lai that bai.
pub async fn script_create_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    name: &str,
    dir: &str,
) -> Result<ScriptRow, IpcError> {
    Ok(ScriptRepo::new(&ctx.db).create(name, dir).await?)
}

#[tauri::command]
pub async fn script_create(
    state: tauri::State<'_, AppState>,
    name: String,
    dir: String,
) -> Result<ScriptRow, IpcError> {
    script_create_impl(&state, &name, &dir).await
}

/// `name`/`dir` la `None` = giu nguyen (xem [`ScriptRepo::update`]).
///
/// # Errors
/// `IpcError` ma `SCRIPT_NOT_FOUND` khi `id` khong ton tai; `STORE` khi ghi/doc lai that bai.
pub async fn script_update_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    name: Option<&str>,
    dir: Option<&str>,
) -> Result<ScriptRow, IpcError> {
    let repo = ScriptRepo::new(&ctx.db);
    require_script(&repo, id).await?;
    Ok(repo.update(id, name, dir).await?)
}

#[tauri::command]
pub async fn script_update(
    state: tauri::State<'_, AppState>,
    id: String,
    name: Option<String>,
    dir: Option<String>,
) -> Result<ScriptRow, IpcError> {
    script_update_impl(&state, &id, name.as_deref(), dir.as_deref()).await
}

/// Ruling R3: kich ban co canh dang chay (`queued`/`processing`) -> `SCENE_RUNNING` (tu choi CA
/// LUOT); xoa KHONG huy task dang co (`Engine::delete_script`).
///
/// # Errors
/// `IpcError` ma `SCRIPT_NOT_FOUND`/`SCENE_RUNNING`/`STORE`.
pub async fn script_delete_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<(), IpcError> {
    require_script(&ScriptRepo::new(&ctx.db), id).await?;
    ctx.engine.delete_script(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn script_delete(state: tauri::State<'_, AppState>, id: String) -> Result<(), IpcError> {
    script_delete_impl(&state, &id).await
}

/// # Errors
/// `IpcError` ma `SCRIPT_NOT_FOUND` khi `id` khong ton tai; `STORE` khi truy van that bai.
pub async fn script_get_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
) -> Result<ScriptDoc, IpcError> {
    let repo = ScriptRepo::new(&ctx.db);
    let meta = repo.meta_for(id).await?.ok_or_else(|| not_found(id))?;
    let scenes = repo.list_scenes(id).await?;
    let tasks = lvc_store::TaskRepo::new(&ctx.db);
    let mut dtos = Vec::with_capacity(scenes.len());
    for s in scenes {
        let (task_status, failure_code, local_path) = match s.task_id.as_deref() {
            Some(tid) => match tasks.get(tid).await? {
                Some(row) => (
                    Some(row.status.as_str().to_string()),
                    row.failure_code.map(|c| c.as_str().to_string()),
                    row.local_path,
                ),
                // task_id "mo coi" (task da bi xoa o noi khac) — coi nhu chua chay, giong
                // Python (`scripts_store.py:145-147` tu don `task_id=NULL` luc doc).
                None => (None, None, None),
            },
            None => (None, None, None),
        };
        dtos.push(SceneDto {
            id: s.id,
            idx: s.idx,
            prompt: s.prompt,
            ratio: s.ratio,
            duration: s.duration,
            model: s.model,
            mode: s.mode,
            logo: s.logo,
            account: s.account,
            refs: s.refs,
            task_id: s.task_id,
            task_status,
            failure_code,
            local_path,
        });
    }
    Ok(ScriptDoc { meta, scenes: dtos })
}

#[tauri::command]
pub async fn script_get(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<ScriptDoc, IpcError> {
    script_get_impl(&state, &id).await
}

/// `scripts_api.py:150-163 put_scenes`: qua tran (`TOO_MANY_SCENES`) kiem TRUOC khi cham DB,
/// dung CHUNG `Limits::batch_max_scenes` voi `run_start`/`workflow_validate` (`Engine::limits()`).
///
/// # Errors
/// `IpcError` ma `SCRIPT_NOT_FOUND`/`TOO_MANY_SCENES`/`SCENE_RUNNING`/`STORE`.
pub async fn script_put_scenes_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    scenes: Vec<SceneIn>,
) -> Result<Vec<SceneRow>, IpcError> {
    let repo = ScriptRepo::new(&ctx.db);
    require_script(&repo, id).await?;
    let max = ctx.engine.limits().batch_max_scenes;
    if scenes.len() > max {
        return Err(IpcError::new(
            "TOO_MANY_SCENES",
            format!("qua nhieu canh (toi da {max})"),
        ));
    }
    Ok(repo.replace_scenes(id, scenes).await?)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (Vec<SceneIn>) - bien gioi IPC"
)]
#[tauri::command]
pub async fn script_put_scenes(
    state: tauri::State<'_, AppState>,
    id: String,
    scenes: Vec<SceneIn>,
) -> Result<Vec<SceneRow>, IpcError> {
    script_put_scenes_impl(&state, &id, scenes).await
}

/// Ruling R1/R2: `scene_ids` rong = moi canh chua chay/da that bai; `style` = "Cố định style".
/// Loi tung canh (`SCENE_RUNNING`/`SCENE_EMPTY`/`SCENE_INVALID`) nam trong
/// `ScriptRunReport.errors`, KHONG lam ca lenh that bai — xem `Engine::run_script`.
///
/// # Errors
/// `IpcError` ma `SCRIPT_NOT_FOUND` khi `id` khong ton tai; `GRAPH_INVALID`/`STORE`/`ENGINE` tu
/// `Engine::run_graph` khi co loi HA TANG (khong phai loi tung canh).
pub async fn script_run_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    scene_ids: &[String],
    style: &str,
) -> Result<ScriptRunReport, IpcError> {
    let repo = ScriptRepo::new(&ctx.db);
    let row = require_script(&repo, id).await?;
    let defaults = ctx.settings.read().await.clone();
    // Engine Video + tham so cua CHINH kich ban nay (cot `video_kind`/`video_params`, migration
    // `0006`) — kich ban cu chua co thi `ScriptRepo` da tra mac dinh san.
    let video_params: serde_json::Value =
        serde_json::from_str(&row.video_params).unwrap_or(serde_json::Value::Null);
    Ok(ctx
        .engine
        .run_script(
            id,
            scene_ids,
            style,
            &defaults,
            &row.video_kind,
            &video_params,
        )
        .await?)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (Vec<String>/String) - bien gioi IPC"
)]
#[tauri::command]
pub async fn script_run(
    state: tauri::State<'_, AppState>,
    id: String,
    scene_ids: Vec<String>,
    style: String,
) -> Result<ScriptRunReport, IpcError> {
    script_run_impl(&state, &id, &scene_ids, &style).await
}

/// `scripts_api.py:206-215 reset_scenes`: dat lai TUNG canh ve "chua chay" (task cu GIU NGUYEN,
/// thanh video le); canh khong ton tai/dang chay -> `skipped` (khong loi ca lenh).
///
/// # Errors
/// `IpcError` ma `SCRIPT_NOT_FOUND` khi `id` khong ton tai; `STORE` khi truy van/ghi that bai.
pub async fn script_reset_scenes_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    scene_ids: &[String],
) -> Result<ResetReport, IpcError> {
    let repo = ScriptRepo::new(&ctx.db);
    require_script(&repo, id).await?;
    let mut reset = Vec::new();
    let mut skipped = Vec::new();
    for scene_id in scene_ids {
        if repo.clear_scene_task(scene_id).await? {
            reset.push(scene_id.clone());
        } else {
            skipped.push(scene_id.clone());
        }
    }
    Ok(ResetReport { reset, skipped })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (Vec<String>) - bien gioi IPC"
)]
#[tauri::command]
pub async fn script_reset_scenes(
    state: tauri::State<'_, AppState>,
    id: String,
    scene_ids: Vec<String>,
) -> Result<ResetReport, IpcError> {
    script_reset_scenes_impl(&state, &id, &scene_ids).await
}

/// Ruling R4: nhap TXT thay TOAN BO canh — tra ve so canh moi.
///
/// # Errors
/// `IpcError` ma `SCRIPT_NOT_FOUND` khi `id` khong ton tai; `SCENE_INVALID` khi cu phap TXT sai;
/// `SCENE_RUNNING` khi kich ban con canh dang chay; `STORE` khi DB loi.
pub async fn script_import_txt_impl<D: BrowserDriver + 'static>(
    ctx: &Ctx<D>,
    id: &str,
    text: &str,
) -> Result<usize, IpcError> {
    require_script(&ScriptRepo::new(&ctx.db), id).await?;
    Ok(ctx.engine.import_txt_to_script(id, text).await?)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Tauri command nhan tham so JSON da giai ma so huu (String) - bien gioi IPC"
)]
#[tauri::command]
pub async fn script_import_txt(
    state: tauri::State<'_, AppState>,
    id: String,
    text: String,
) -> Result<usize, IpcError> {
    script_import_txt_impl(&state, &id, &text).await
}

#[cfg(test)]
mod tests {
    use lvc_browser::FakeStep;
    use lvc_browser::dola::{ConversationId, VideoArtifact, poll::NOTE_RAW};

    use super::*;
    use crate::commands::test_support::ctx_with;

    fn scene_in(prompt: &str) -> SceneIn {
        SceneIn {
            id: None,
            prompt: prompt.to_string(),
            ..SceneIn::default()
        }
    }

    fn artifact(src: &std::path::Path) -> VideoArtifact {
        VideoArtifact {
            video_url: "https://v.example/x.mp4".into(),
            local_path: src.to_path_buf(),
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

    #[tokio::test]
    async fn script_crud_round_trip() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let created = script_create_impl(&ctx, "Demo", "").await.unwrap();
        assert_eq!(created.name, "Demo");

        let list = script_list_impl(&ctx).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, created.id);
        assert_eq!(list[0].total, 0);

        let updated = script_update_impl(&ctx, &created.id, Some("Demo 2"), None)
            .await
            .unwrap();
        assert_eq!(updated.name, "Demo 2");

        script_delete_impl(&ctx, &created.id).await.unwrap();
        let err = script_get_impl(&ctx, &created.id).await.unwrap_err();
        assert_eq!(err.code, "SCRIPT_NOT_FOUND");
    }

    #[tokio::test]
    async fn khong_ton_tai_bao_dung_ma_cho_moi_lenh_thao_tac() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        assert_eq!(
            script_update_impl(&ctx, "ghost", Some("x"), None)
                .await
                .unwrap_err()
                .code,
            "SCRIPT_NOT_FOUND"
        );
        assert_eq!(
            script_delete_impl(&ctx, "ghost").await.unwrap_err().code,
            "SCRIPT_NOT_FOUND"
        );
        assert_eq!(
            script_get_impl(&ctx, "ghost").await.unwrap_err().code,
            "SCRIPT_NOT_FOUND"
        );
        assert_eq!(
            script_put_scenes_impl(&ctx, "ghost", vec![])
                .await
                .unwrap_err()
                .code,
            "SCRIPT_NOT_FOUND"
        );
        assert_eq!(
            script_run_impl(&ctx, "ghost", &[], "")
                .await
                .unwrap_err()
                .code,
            "SCRIPT_NOT_FOUND"
        );
        assert_eq!(
            script_reset_scenes_impl(&ctx, "ghost", &[])
                .await
                .unwrap_err()
                .code,
            "SCRIPT_NOT_FOUND"
        );
        assert_eq!(
            script_import_txt_impl(&ctx, "ghost", "x")
                .await
                .unwrap_err()
                .code,
            "SCRIPT_NOT_FOUND"
        );
    }

    #[tokio::test]
    async fn put_scenes_qua_tran_bao_too_many_scenes() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let s = script_create_impl(&ctx, "Demo", "").await.unwrap();
        let max = ctx.engine.limits().batch_max_scenes;
        let many: Vec<SceneIn> = (0..=max).map(|i| scene_in(&format!("canh {i}"))).collect();
        let err = script_put_scenes_impl(&ctx, &s.id, many).await.unwrap_err();
        assert_eq!(err.code, "TOO_MANY_SCENES");
    }

    #[tokio::test]
    async fn script_get_tra_taskstatus_dung_sau_khi_chay() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();
        let (_dir, ctx, _driver) = ctx_with(happy_steps(1, &src), &["acc1"]).await;

        let s = script_create_impl(&ctx, "Demo", "").await.unwrap();
        script_put_scenes_impl(&ctx, &s.id, vec![scene_in("canh 1")])
            .await
            .unwrap();

        let report = script_run_impl(&ctx, &s.id, &[], "").await.unwrap();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.tasks.len(), 1);

        // Doi task xong (run_script chi cho toi khi dong task xuat hien — xem doc
        // `Engine::run_script`), giong `run_start_tao_run_va_3_task_run_list_thay`.
        let tasks = lvc_store::TaskRepo::new(&ctx.db);
        for _ in 0..200 {
            let row = tasks.get(&report.tasks[0].task_id).await.unwrap().unwrap();
            if row.status != lvc_core::TaskStatus::Queued
                && row.status != lvc_core::TaskStatus::Processing
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let doc = script_get_impl(&ctx, &s.id).await.unwrap();
        assert_eq!(doc.meta.total, 1);
        assert_eq!(doc.scenes.len(), 1);
        assert_eq!(doc.scenes[0].task_status.as_deref(), Some("completed"));
        assert!(doc.scenes[0].local_path.is_some());
    }

    #[tokio::test]
    async fn reset_scenes_tra_dung_reset_va_skipped() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &["acc1"]).await;
        let s = script_create_impl(&ctx, "Demo", "").await.unwrap();
        script_put_scenes_impl(&ctx, &s.id, vec![scene_in("canh 1")])
            .await
            .unwrap();

        let report = script_reset_scenes_impl(&ctx, &s.id, &["khong-ton-tai".to_string()])
            .await
            .unwrap();
        assert!(report.reset.is_empty());
        assert_eq!(report.skipped, vec!["khong-ton-tai".to_string()]);
    }

    #[tokio::test]
    async fn import_txt_impl_thay_toan_bo_canh() {
        let (_dir, ctx, _driver) = ctx_with(vec![], &[]).await;
        let s = script_create_impl(&ctx, "Demo", "").await.unwrap();
        script_put_scenes_impl(&ctx, &s.id, vec![scene_in("canh cu")])
            .await
            .unwrap();

        let n = script_import_txt_impl(&ctx, &s.id, "canh 1\ncanh 2\n")
            .await
            .unwrap();
        assert_eq!(n, 2);
        let doc = script_get_impl(&ctx, &s.id).await.unwrap();
        assert_eq!(doc.scenes.len(), 2);
    }
}
