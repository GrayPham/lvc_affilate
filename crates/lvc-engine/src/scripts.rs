//! P3c Task 1 — bien cac canh da chon cua MOT kich ban thanh MOT lan chay DAG (R1/R2), nhap TXT
//! thay toan bo canh (R4), xoa kich ban co canh dang chay bi tu choi (R3). Dung LAI
//! `Engine::run_graph` (`dag.rs`, Task 7) — khong dung bo thuc thi rieng cho kich ban.
//!
//! Port `A:\ToolLVC\lvcseedance\src\core\scripts_api.py:167-215` (`run_scenes`/`reset_scenes`) +
//! `scripts_store.py` (schema/CRUD, da port sang `lvc_store::scripts`).
//!
//! `ScriptRepo` duoc dung TREN CHO (`ScriptRepo::new(&self.db)`) trong tung ham thay vi them
//! truong moi vao `Engine` — `Engine::db` da la truong `pub(crate)` co san (them boi Task 10 cho
//! `import_legacy`, xem `generate.rs`), khong can them mot repo thuong tru chi de doc/ghi 2 bang
//! it dung.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lvc_browser::BrowserDriver;
use lvc_core::graph::{Edge, Graph, Node, NodeKind};
use lvc_core::{AppSettings, LogoMode, RawScene, Scene, format_txt, parse_txt};
use lvc_store::{ReplaceScenesError, SceneIn, SceneRow, ScriptRepo};
use serde::{Deserialize, Serialize};

use crate::dag::noop_hooks_factory;
use crate::error::EngineError;
use crate::generate::Engine;

/// So lan doi toi da khi doi cac dong `TaskRow` cua mot lan chay duoc tao (xem doc
/// [`Engine::run_script`]) — `POLL_INTERVAL * POLL_MAX_TRIES` = 2s, giong nguong cho trong cac
/// test `run_start`/`run_cancel` co san (`crates/lvc-app/src/commands/workflow.rs`).
const POLL_MAX_TRIES: u32 = 200;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Loi rieng cua [`Engine::import_txt_to_script`]/[`Engine::delete_script`] — `Engine::run_script`
/// KHONG dung kieu nay (chu ky cong khai cua no la `Result<_, EngineError>`, xem task-1-brief.md;
/// loi tung canh la DU LIEU trong `ScriptRunReport.errors`, khong phai nhanh `Err`).
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error(transparent)]
    Store(#[from] lvc_store::StoreError),
    /// `parse_txt` (lvc-core) that bai cu phap — TU CHOI CA LUOT nhap (khong nhap mot phan).
    #[error("canh khong hop le: {0}")]
    SceneInvalid(String),
    /// [`ReplaceScenesError::SceneRunning`] — canh dang chay bi bo khoi TXT/mang moi.
    #[error("canh dang chay: {0:?}")]
    SceneRunning(Vec<String>),
}

impl From<ReplaceScenesError> for ScriptError {
    fn from(e: ReplaceScenesError) -> Self {
        match e {
            ReplaceScenesError::Store(se) => Self::Store(se),
            ReplaceScenesError::SceneRunning(ids) => Self::SceneRunning(ids),
        }
    }
}

/// Mot canh da duoc gan `task_id` sau [`Engine::run_script`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRunTask {
    pub scene_id: String,
    pub task_id: String,
}

/// Mot canh KHONG duoc chay — `code` la mot trong `SCENE_RUNNING`/`SCENE_EMPTY`/`SCENE_INVALID`
/// (xem [`Engine::run_script`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRunError {
    pub scene_id: String,
    pub code: String,
}

/// Ket qua [`Engine::run_script`]. `run_id` = `None` khi KHONG co canh nao hop le de chay (moi
/// canh duoc chon deu roi vao `errors`) — khong tao dong `runs` nao trong truong hop do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRunReport {
    pub run_id: Option<String>,
    pub tasks: Vec<ScriptRunTask>,
    pub errors: Vec<ScriptRunError>,
}

const SCENE_RUNNING: &str = "SCENE_RUNNING";
const SCENE_EMPTY: &str = "SCENE_EMPTY";
const SCENE_INVALID: &str = "SCENE_INVALID";

/// `value` rong/khoang trang -> `default`; nguoc lai gia tri da trim.
fn non_empty_or(value: Option<&str>, default: &str) -> String {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
        .to_string()
}

/// Xay `RawScene` tu mot dong `scenes` + mac dinh `AppSettings` (mode/model/ratio/duration —
/// logo/account KHONG co trong `AppSettings`, dung mac dinh cua chinh `Scene::normalize`/`""`) —
/// xem doc [`Engine::run_script`] ve ly do khong hardcode "9:16"/"seedance-2.0"/... nhu ban Python
/// (`scripts_store.py::_normalize`).
fn scene_row_to_raw(scene: &SceneRow, prompt: String, defaults: &AppSettings) -> RawScene {
    RawScene {
        prompt: Some(prompt),
        ratio: Some(non_empty_or(
            scene.ratio.as_deref(),
            defaults.default_ratio.as_str(),
        )),
        duration: Some(match scene.duration {
            Some(d) if d > 0 => d.to_string(),
            _ => defaults.default_duration.to_string(),
        }),
        model: Some(non_empty_or(
            scene.model.as_deref(),
            defaults.default_model.as_str(),
        )),
        mode: Some(non_empty_or(
            scene.mode.as_deref(),
            defaults.default_mode.as_str(),
        )),
        logo: scene.logo.clone(),
        reference_images: Some(scene.refs.clone()),
        account: Some(scene.account.clone()),
    }
}

/// `outputs.safe_folder_name` (Python `src/utils/outputs.py:23-27`): bo ky tu cam tren Windows,
/// gop khoang trang lien tiep, bo dau cham/khoang trang dau-cuoi, toi da 60 ky tu.
fn safe_folder_name(name: &str) -> String {
    let filtered: String = name
        .chars()
        .filter(|c| {
            !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()
        })
        .collect();
    let collapsed = filtered.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches([' ', '.']);
    let truncated: String = trimmed.chars().take(60).collect();
    truncated.trim_end_matches([' ', '.']).to_string()
}

/// `dir` kich ban (da trim) neu khong rong; nguoc lai `<downloads_dir>/<ten da lam an toan>`
/// (rong sau khi lam an toan -> dung `script_id` — ten kich ban KHONG BAO GIO rong that su, xem
/// [`ScriptRepo::create`], day chi la phong thu). Ruling R1 + thiet ke Task 1.
fn effective_out_dir(dir: &str, name: &str, script_id: &str, downloads_dir: &Path) -> PathBuf {
    let trimmed = dir.trim();
    if !trimmed.is_empty() {
        return PathBuf::from(trimmed);
    }
    let sanitized = safe_folder_name(name);
    let sub = if sanitized.is_empty() {
        script_id.to_string()
    } else {
        sanitized
    };
    downloads_dir.join(sub)
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Chay cac canh da chon cua MOT kich ban (ruling R1/R2 — plan P3c). `scene_ids` rong = MOI
    /// canh CHUA chay hoac da that bai (`task_id IS NULL` hoac trang thai `Failed`); khac rong =
    /// dung DUNG cac id do, theo thu tu `idx` cua kich ban (KHONG theo thu tu `scene_ids` truyen
    /// vao — giong `scripts_api.py:171-176 run_scenes`).
    ///
    /// Voi MOI canh duoc chon: dang `queued`/`processing` -> loi `SCENE_RUNNING`; prompt rong (sau
    /// trim) -> loi `SCENE_EMPTY`; `style` khong rong va CHUA co trong prompt (khong phan biet hoa
    /// thuong) -> noi `"\nStyle: {style}"` (`scripts_api.py:189-193`); cuoi cung chuan hoa qua
    /// [`Scene::normalize`] (mac dinh tu `defaults`, xem [`scene_row_to_raw`]) — loi chuan hoa ->
    /// `SCENE_INVALID`. Canh loi bi LOAI KHOI lan chay (khong dung ca lo, giong Python: moi canh
    /// doc lap).
    ///
    /// Canh con lai (hop le) duoc `format_txt` thanh MOT node Prompt -> node Tao video (mac dinh
    /// mode/model/ratio/duration tu `defaults`) -> node Luu (`dir` kich ban, rong -> thu muc con
    /// cua `downloads_dir` dat theo ten kich ban) roi chay qua [`Engine::run_graph`] (tai su dung
    /// DAG, su kien, huy — R1). `run_graph` tra ve NGAY (chay nen — xem doc `dag.rs`); ham nay CHO
    /// (toi da ~2s, poll ngan) den khi cac dong `TaskRow` cua lan chay xuat hien roi anh xa
    /// `scene_index` (1-based, dung thu tu voi cac canh hop le) sang tung canh — KHONG cho ca lan
    /// chay hoan tat (video co the mat vai phut/canh): "Dung tat ca" (`run_cancel`, Task 2) can
    /// `run_id` con dung khi video con dang tao.
    ///
    /// # Errors
    /// `EngineError::Graph`/`EngineError::Store` tu `run_graph`/doc DB — loi tung canh KHONG nam
    /// o day, xem `ScriptRunReport.errors`.
    #[allow(
        clippy::too_many_lines,
        reason = "than ham gom du 4 buoc tuan tu (loc canh muc tieu, chuan hoa tung canh, dung \
                  Graph, doi + anh xa task) - tach nho se roi tham so qua nhieu giua cac ham con \
                  hon la lam ro hon (giong ly do Engine::attempt/run_mode da chap nhan)"
    )]
    pub async fn run_script(
        self: &Arc<Self>,
        script_id: &str,
        scene_ids: &[String],
        style: &str,
        defaults: &AppSettings,
    ) -> Result<ScriptRunReport, EngineError> {
        let repo = ScriptRepo::new(&self.db);
        let all_scenes = repo.list_scenes(script_id).await?;
        let statuses = repo.scene_task_statuses(script_id).await?;

        let wanted: Option<HashSet<&str>> =
            (!scene_ids.is_empty()).then(|| scene_ids.iter().map(String::as_str).collect());

        let targets: Vec<&SceneRow> = all_scenes
            .iter()
            .filter(|s| match &wanted {
                Some(w) => w.contains(s.id.as_str()),
                None => !matches!(
                    statuses.get(&s.id),
                    Some(
                        lvc_core::TaskStatus::Completed
                            | lvc_core::TaskStatus::Queued
                            | lvc_core::TaskStatus::Processing
                    )
                ),
            })
            .collect();

        let style_trim = style.trim();
        let mut errors = Vec::new();
        let mut valid: Vec<(&SceneRow, Scene)> = Vec::new();

        for scene in targets {
            if matches!(
                statuses.get(&scene.id),
                Some(lvc_core::TaskStatus::Queued | lvc_core::TaskStatus::Processing)
            ) {
                errors.push(ScriptRunError {
                    scene_id: scene.id.clone(),
                    code: SCENE_RUNNING.to_string(),
                });
                continue;
            }
            let base_prompt = scene.prompt.trim();
            if base_prompt.is_empty() {
                errors.push(ScriptRunError {
                    scene_id: scene.id.clone(),
                    code: SCENE_EMPTY.to_string(),
                });
                continue;
            }
            let mut prompt = base_prompt.to_string();
            // scripts_api.py:189-193: ban Python noi "\nStyle: <style>" TRUC TIEP vao prompt roi
            // tao task ngay (khong qua TXT). O day prompt con phai di qua format_txt/parse_txt
            // (R1 — Prompt node dung MOT khoi TXT, moi dong la MOT canh) TRUOC khi thanh task:
            // mot ky tu \n THAT trong prompt se bi .lines() cua parse_txt hieu nham la RANH GIOI
            // giua hai canh, tach canh nay thanh hai VideoJob rieng (mot canh "ma" chi chua dong
            // "Style: …"). Dung dau cach thay \n — giu nguyen y nghia (nhan "Style: …" ngay sau
            // prompt, kiem trung khong phan biet hoa/thuong nhu Python) ma van an toan qua TXT.
            if !style_trim.is_empty() && !prompt.to_lowercase().contains(&style_trim.to_lowercase())
            {
                prompt = format!("{prompt} Style: {style_trim}");
            }
            let raw = scene_row_to_raw(scene, prompt, defaults);
            #[allow(
                clippy::cast_sign_loss,
                reason = "scene.idx luon duong (1-based, ScriptRepo::replace_scenes tu danh)"
            )]
            let line = usize::try_from(scene.idx).ok();
            match Scene::normalize(&raw, &self.limits, line) {
                Ok(s) => valid.push((scene, s)),
                Err(_) => errors.push(ScriptRunError {
                    scene_id: scene.id.clone(),
                    code: SCENE_INVALID.to_string(),
                }),
            }
        }

        if valid.is_empty() {
            return Ok(ScriptRunReport {
                run_id: None,
                tasks: Vec::new(),
                errors,
            });
        }

        let scenes_only: Vec<Scene> = valid.iter().map(|(_, s)| s.clone()).collect();
        let text = format_txt(&scenes_only);

        let script_row = repo.get(script_id).await?;
        let dir = script_row
            .as_ref()
            .map_or_else(String::new, |s| s.dir.clone());
        let name = script_row
            .as_ref()
            .map_or_else(|| script_id.to_string(), |s| s.name.clone());
        let out_dir = effective_out_dir(&dir, &name, script_id, &self.cfg.downloads_dir);

        let graph = Graph {
            version: 1,
            nodes: vec![
                Node {
                    id: "prompt".to_string(),
                    kind: NodeKind::Prompt { text },
                    x: 80.0,
                    y: 80.0,
                },
                Node {
                    id: "video".to_string(),
                    kind: NodeKind::Video {
                        mode: defaults.default_mode,
                        model: defaults.default_model,
                        ratio: defaults.default_ratio,
                        duration: defaults.default_duration,
                        logo: LogoMode::default(),
                        account: String::new(),
                    },
                    x: 380.0,
                    y: 80.0,
                },
                Node {
                    id: "save".to_string(),
                    kind: NodeKind::Save {
                        out_dir: out_dir.display().to_string(),
                    },
                    x: 680.0,
                    y: 80.0,
                },
            ],
            edges: vec![
                Edge {
                    id: "e1".to_string(),
                    from: "prompt".to_string(),
                    from_port: "out".to_string(),
                    to: "video".to_string(),
                    to_port: "prompt".to_string(),
                },
                Edge {
                    id: "e2".to_string(),
                    from: "video".to_string(),
                    from_port: "out".to_string(),
                    to: "save".to_string(),
                    to_port: "video".to_string(),
                },
            ],
        };

        let handle = self
            .run_graph(script_id, &graph, noop_hooks_factory())
            .await?;
        let run_id = handle.run_id.clone();

        // run_graph() chi tao dong NEN (spawn) roi tra ve NGAY — cho toi khi cac dong TaskRow
        // xuat hien (tao rat nhanh, KHONG doi video tao xong) de anh xa scene_id -> task_id; xem
        // doc ham ve ly do khong `handle.join.await` (se khoa "Dung tat ca" cho toi khi xong het).
        let expected = valid.len();
        let mut rows = self.tasks.list_by_run(&run_id).await?;
        for _ in 0..POLL_MAX_TRIES {
            if rows.len() >= expected {
                break;
            }
            tokio::time::sleep(POLL_INTERVAL).await;
            rows = self.tasks.list_by_run(&run_id).await?;
        }

        let mut tasks = Vec::with_capacity(rows.len().min(expected));
        for ((scene, _), row) in valid.iter().zip(rows.iter()) {
            repo.set_scene_task(&scene.id, &row.id).await?;
            tasks.push(ScriptRunTask {
                scene_id: scene.id.clone(),
                task_id: row.id.clone(),
            });
        }

        Ok(ScriptRunReport {
            run_id: Some(run_id),
            tasks,
            errors,
        })
    }

    /// Nhap TXT thay TOAN BO canh cua kich ban (ruling R4): `parse_txt` (lvc-core, gioi han qua
    /// `self.limits`) roi `ScriptRepo::replace_scenes` — canh dang chay bi bo khoi TXT moi ->
    /// TU CHOI CA LUOT (khong nhap mot phan, xem [`lvc_store::ReplaceScenesError`]). Canh nhap
    /// MOI hoan toan (khong giu id cu) — dung y R4 "thay toan bo".
    ///
    /// # Errors
    /// `ScriptError::SceneInvalid` khi cu phap TXT sai; `ScriptError::SceneRunning` khi kich ban
    /// dang co canh chay (TXT moi khong the giu duoc canh dang chay vi khong mang id cu);
    /// `ScriptError::Store` khi DB loi.
    pub async fn import_txt_to_script(
        &self,
        script_id: &str,
        text: &str,
    ) -> Result<usize, ScriptError> {
        let scenes =
            parse_txt(text, &self.limits).map_err(|e| ScriptError::SceneInvalid(e.to_string()))?;
        let items: Vec<SceneIn> = scenes
            .iter()
            .map(|s| SceneIn {
                id: None,
                prompt: s.prompt.clone(),
                ratio: Some(s.ratio.as_str().to_string()),
                duration: Some(i64::from(s.duration)),
                model: Some(s.model.as_str().to_string()),
                mode: Some(s.mode.as_str().to_string()),
                logo: Some(s.logo.as_str().to_string()),
                account: s.account.clone(),
                refs: s.reference_images.clone(),
            })
            .collect();
        let saved = ScriptRepo::new(&self.db)
            .replace_scenes(script_id, items)
            .await?;
        Ok(saved.len())
    }

    /// Xoa kich ban — canh dang chay -> tu choi (ruling R3); xoa KHONG huy task (`ScriptRepo::delete`
    /// chi go lien ket, khong dung toi `TaskRepo`). Nguoi goi (lvc-app) chiu trach nhiem kiem
    /// `script_id` co ton tai truoc (giong `workflow_delete_impl`/`_script_or_404` cua Python) —
    /// ham nay khong tu bao `SCRIPT_NOT_FOUND` rieng, `id` la khong ton tai thi don gian khong co
    /// canh nao dang chay va `ScriptRepo::delete` la no-op.
    ///
    /// # Errors
    /// `ScriptError::SceneRunning` khi con canh `queued`/`processing`; `ScriptError::Store` khi
    /// DB loi.
    pub async fn delete_script(&self, script_id: &str) -> Result<Vec<String>, ScriptError> {
        let repo = ScriptRepo::new(&self.db);
        let running = repo.running_scene_ids(script_id).await?;
        if !running.is_empty() {
            return Err(ScriptError::SceneRunning(running));
        }
        Ok(repo.delete(script_id).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lvc_browser::dola::{ConversationId, poll::NOTE_RAW};
    use lvc_browser::{FakeDriver, FakeStep};
    use lvc_core::{Limits, Mode, Model, Ratio, TaskStatus};
    use lvc_store::{AccountRepo, BatchRef, Db, SceneIn, ScriptRepo, TaskRepo};

    use super::{Engine, ScriptError};
    use crate::generate::EngineConfig;

    /// Nhu `dag::tests::build_engine` (rieng ban cho `scripts.rs`, khong dung chung duoc vi
    /// khong `pub(crate)` — xem doc do): `threads=1` de `FakeDriver` (MOT `VecDeque` dung chung
    /// cho moi job) khong bi nhieu job xen ke goi driver cung luc.
    async fn build_engine(
        accounts: &[&str],
        steps: Vec<FakeStep>,
    ) -> (tempfile::TempDir, Arc<Engine<FakeDriver>>, Db) {
        let db = Db::open_memory().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let accounts_dir = dir.path().join("accounts");
        let repo = AccountRepo::new(&db);
        for n in accounts {
            std::fs::create_dir_all(accounts_dir.join(n)).unwrap();
            repo.upsert(n).await.unwrap();
        }
        let mut cfg = EngineConfig::new(&accounts_dir, dir.path().join("downloads"));
        cfg.logs_dir = dir.path().join("logs");
        let driver = Arc::new(FakeDriver::new(steps));
        let (engine, _handle) = Engine::new(driver, db.clone(), Limits::default(), cfg, 1)
            .await
            .unwrap();
        (dir, Arc::new(engine), db)
    }

    fn artifact(src: &std::path::Path) -> lvc_browser::dola::VideoArtifact {
        lvc_browser::dola::VideoArtifact {
            video_url: "https://v.example/x.mp4".into(),
            local_path: src.to_path_buf(),
            conversation_id: ConversationId("c".into()),
            account: String::new(),
            watermarked: false,
            bytes: 5,
            note: NOTE_RAW,
        }
    }

    /// `n` bo (Launch, Submit, Poll) thanh cong, deu tra ve `src` — giong `dag::tests::happy_steps`.
    fn happy_steps(n: usize, src: &std::path::Path) -> Vec<FakeStep> {
        let mut v = Vec::new();
        for i in 0..n {
            v.push(FakeStep::Launch(Ok(())));
            v.push(FakeStep::Submit(Ok(ConversationId(format!("c{i}")))));
            v.push(FakeStep::Poll(Ok(artifact(src))));
        }
        v
    }

    fn scene_in(prompt: &str) -> SceneIn {
        SceneIn {
            id: None,
            prompt: prompt.to_string(),
            ..SceneIn::default()
        }
    }

    /// `run_script` CHI cho toi khi dong `TaskRow` xuat hien (xem doc `Engine::run_script`) —
    /// KHONG cho video tao xong (chay nen, `FakeDriver` gan-tuc-thi nhung van la task rieng). Test
    /// can biet ket qua cuoi cung (Completed/Failed) thi phai tu doi them, giong
    /// `run_start_tao_run_va_3_task_run_list_thay` (`crates/lvc-app/src/commands/workflow.rs`).
    async fn wait_task_done(tasks: &TaskRepo, task_id: &str) -> lvc_store::TaskRow {
        for _ in 0..200 {
            let row = tasks.get(task_id).await.unwrap().unwrap();
            if !matches!(row.status, TaskStatus::Queued | TaskStatus::Processing) {
                return row;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("task {task_id} khong xong trong khoang cho cua test");
    }

    /// Doi CA lan chay xong (Save chay SAU Video trong `run_steps`, xem `dag.rs`) — dung khi test
    /// can kiem tra file da luu (`wait_task_done` chi bao dam tung task xong, khong bao dam buoc
    /// Save theo sau da chay toi).
    async fn wait_run_done(engine: &Engine<FakeDriver>, run_id: &str) {
        for _ in 0..200 {
            let row = engine.run_repo.get(run_id).await.unwrap();
            if row.is_some_and(|r| r.status != lvc_store::RunStatus::Running) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("run {run_id} khong xong trong khoang cho cua test");
    }

    #[tokio::test]
    async fn ba_canh_thanh_cong_tao_1_run_3_task_map_dung_scene_index() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();
        let (dir, engine, db) = build_engine(&["acc1", "acc2", "acc3"], happy_steps(3, &src)).await;
        let scripts = ScriptRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        scripts
            .replace_scenes(
                &s.id,
                vec![scene_in("canh 1"), scene_in("canh 2"), scene_in("canh 3")],
            )
            .await
            .unwrap();

        let report = engine
            .run_script(&s.id, &[], "", &lvc_core::AppSettings::default())
            .await
            .unwrap();
        assert!(report.run_id.is_some());
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.tasks.len(), 3);

        let tasks = TaskRepo::new(&db);
        let scenes = scripts.list_scenes(&s.id).await.unwrap();
        for (i, scene) in scenes.iter().enumerate() {
            let mapped = report
                .tasks
                .iter()
                .find(|t| t.scene_id == scene.id)
                .expect("moi canh phai co task tuong ung");
            assert_eq!(scene.task_id.as_deref(), Some(mapped.task_id.as_str()));
            let row = wait_task_done(&tasks, &mapped.task_id).await;
            assert_eq!(row.status, TaskStatus::Completed, "{row:?}");
            assert_eq!(
                row.scene_index,
                Some(i64::try_from(i + 1).unwrap()),
                "scene_index phai dung thu tu {i}"
            );
        }

        // out_dir fallback: dir rong -> downloads_dir/<ten kich ban an toan>.
        let out_dir = dir.path().join("downloads").join("Demo");
        let run_id = report.run_id.unwrap();
        wait_run_done(&engine, &run_id).await;
        for i in 1..=3 {
            let f = out_dir.join(format!("{run_id}_video_{i}.mp4"));
            assert!(f.exists(), "{f:?} phai ton tai");
        }
    }

    #[tokio::test]
    async fn canh_dang_chay_bi_chon_tuong_minh_thi_loi_scene_running_khong_chay_lai() {
        let (_dir, engine, db) = build_engine(&["acc1"], vec![]).await;
        let scripts = ScriptRepo::new(&db);
        let tasks = TaskRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        let scenes = scripts
            .replace_scenes(&s.id, vec![scene_in("canh 1")])
            .await
            .unwrap();

        let scene = lvc_core::Scene::normalize(
            &lvc_core::RawScene {
                prompt: Some("canh 1".into()),
                ..lvc_core::RawScene::default()
            },
            &Limits::default(),
            None,
        )
        .unwrap();
        let batch = BatchRef {
            batch_id: "run_ngoai".to_string(),
            batch_name: "n".to_string(),
            scene_index: 1,
        };
        tasks
            .create("t_running", &scene, Some(&batch))
            .await
            .unwrap();
        scripts
            .set_scene_task(&scenes[0].id, "t_running")
            .await
            .unwrap();
        // TaskRepo::create mac dinh status = 'queued' -> canh dang chay.

        let report = engine
            .run_script(
                &s.id,
                std::slice::from_ref(&scenes[0].id),
                "",
                &lvc_core::AppSettings::default(),
            )
            .await
            .unwrap();
        assert!(report.run_id.is_none(), "khong canh nao hop le de chay");
        assert!(report.tasks.is_empty());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].scene_id, scenes[0].id);
        assert_eq!(report.errors[0].code, "SCENE_RUNNING");

        // task_id cu KHONG bi doi (khong chay lai).
        let after = scripts.list_scenes(&s.id).await.unwrap();
        assert_eq!(after[0].task_id.as_deref(), Some("t_running"));
    }

    #[tokio::test]
    async fn prompt_rong_thi_loi_scene_empty() {
        let (_dir, engine, db) = build_engine(&["acc1"], vec![]).await;
        let scripts = ScriptRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        scripts
            .replace_scenes(&s.id, vec![scene_in("   ")])
            .await
            .unwrap();

        let report = engine
            .run_script(&s.id, &[], "", &lvc_core::AppSettings::default())
            .await
            .unwrap();
        assert!(report.run_id.is_none());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].code, "SCENE_EMPTY");
    }

    #[tokio::test]
    async fn style_duoc_noi_chi_khi_chua_co_san_khong_phan_biet_hoa_thuong() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();
        // 2 tai khoan (khong phai 1) — mot tai khoan xu ly 2 job TUAN TU se TAI DUNG phien am
        // (SessionCache) cho job thu hai, bo qua buoc Launch ma FakeDriver khong luong truoc
        // duoc (xem doc `happy_steps`/precedent `dag::tests::video_3_canh_thanh_cong...` dung
        // N tai khoan cho N canh, khong bao gio tai su dung 1 tai khoan cho nhieu canh).
        let (_dir, engine, db) = build_engine(&["acc1", "acc2"], happy_steps(2, &src)).await;
        let scripts = ScriptRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        scripts
            .replace_scenes(
                &s.id,
                vec![
                    scene_in("canh chua co style"),
                    scene_in("canh da co Style: Cinematic san roi"),
                ],
            )
            .await
            .unwrap();

        let report = engine
            .run_script(&s.id, &[], "Cinematic", &lvc_core::AppSettings::default())
            .await
            .unwrap();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.tasks.len(), 2);

        let tasks = TaskRepo::new(&db);
        let scenes = scripts.list_scenes(&s.id).await.unwrap();
        let t0 = wait_task_done(&tasks, scenes[0].task_id.as_deref().unwrap()).await;
        assert_eq!(t0.prompt, "canh chua co style Style: Cinematic");
        let t1 = wait_task_done(&tasks, scenes[1].task_id.as_deref().unwrap()).await;
        assert_eq!(
            t1.prompt, "canh da co Style: Cinematic san roi",
            "khong noi lap khi da co san (khong phan biet hoa/thuong)"
        );
    }

    #[tokio::test]
    async fn mac_dinh_lay_tu_app_settings_khi_canh_khong_dat() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();
        let (_dir, engine, db) = build_engine(&["acc1"], happy_steps(1, &src)).await;
        let scripts = ScriptRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        scripts
            .replace_scenes(&s.id, vec![scene_in("canh 1")])
            .await
            .unwrap();

        let defaults = lvc_core::AppSettings {
            default_ratio: Ratio::R16x9,
            default_mode: Mode::Pro,
            default_model: Model::Seedance25,
            default_duration: 15,
            ..lvc_core::AppSettings::default()
        };
        let report = engine.run_script(&s.id, &[], "", &defaults).await.unwrap();
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let tasks = TaskRepo::new(&db);
        let row = tasks.get(&report.tasks[0].task_id).await.unwrap().unwrap();
        assert_eq!(row.ratio, "16:9");
        assert_eq!(row.mode, "pro");
        assert_eq!(row.model, "seedance-2.5");
        assert_eq!(row.duration, 15);
    }

    #[tokio::test]
    async fn out_dir_dung_dir_kich_ban_khi_co_dat() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4").unwrap();
        let (dir, engine, db) = build_engine(&["acc1"], happy_steps(1, &src)).await;
        let scripts = ScriptRepo::new(&db);
        let custom_dir = dir.path().join("noi luu rieng");
        let s = scripts
            .create("Demo", custom_dir.to_str().unwrap())
            .await
            .unwrap();
        scripts
            .replace_scenes(&s.id, vec![scene_in("canh 1")])
            .await
            .unwrap();

        let report = engine
            .run_script(&s.id, &[], "", &lvc_core::AppSettings::default())
            .await
            .unwrap();
        let run_id = report.run_id.unwrap();
        wait_run_done(&engine, &run_id).await;
        let f = custom_dir.join(format!("{run_id}_video_1.mp4"));
        assert!(f.exists(), "{f:?} phai ton tai trong dir kich ban");
    }

    #[tokio::test]
    async fn import_txt_thay_toan_bo_canh() {
        let (_dir, engine, db) = build_engine(&["acc1"], vec![]).await;
        let scripts = ScriptRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        scripts
            .replace_scenes(&s.id, vec![scene_in("canh cu")])
            .await
            .unwrap();

        let n = engine
            .import_txt_to_script(&s.id, "canh moi 1\ncanh moi 2 | ratio=16:9\n")
            .await
            .unwrap();
        assert_eq!(n, 2);

        let scenes = scripts.list_scenes(&s.id).await.unwrap();
        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes[0].prompt, "canh moi 1");
        assert_eq!(scenes[1].prompt, "canh moi 2");
        assert_eq!(scenes[1].ratio.as_deref(), Some("16:9"));
    }

    #[tokio::test]
    async fn import_txt_cu_phap_sai_bao_scene_invalid_khong_doi_gi() {
        let (_dir, engine, db) = build_engine(&["acc1"], vec![]).await;
        let scripts = ScriptRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        scripts
            .replace_scenes(&s.id, vec![scene_in("canh cu")])
            .await
            .unwrap();

        let err = engine
            .import_txt_to_script(&s.id, "x | bogus=1")
            .await
            .unwrap_err();
        assert!(matches!(err, ScriptError::SceneInvalid(_)), "{err:?}");
        let scenes = scripts.list_scenes(&s.id).await.unwrap();
        assert_eq!(scenes.len(), 1, "khong ghi gi khi TXT sai cu phap");
        assert_eq!(scenes[0].prompt, "canh cu");
    }

    #[tokio::test]
    async fn delete_script_tu_choi_khi_con_canh_dang_chay() {
        let (_dir, engine, db) = build_engine(&["acc1"], vec![]).await;
        let scripts = ScriptRepo::new(&db);
        let tasks = TaskRepo::new(&db);
        let s = scripts.create("Demo", "").await.unwrap();
        let scenes = scripts
            .replace_scenes(&s.id, vec![scene_in("canh 1")])
            .await
            .unwrap();
        let scene = lvc_core::Scene::normalize(
            &lvc_core::RawScene {
                prompt: Some("canh 1".into()),
                ..lvc_core::RawScene::default()
            },
            &Limits::default(),
            None,
        )
        .unwrap();
        tasks.create("t1", &scene, None).await.unwrap();
        scripts.set_scene_task(&scenes[0].id, "t1").await.unwrap();

        let err = engine.delete_script(&s.id).await.unwrap_err();
        match err {
            ScriptError::SceneRunning(ids) => assert_eq!(ids, vec![scenes[0].id.clone()]),
            other => panic!("{other:?}"),
        }
        assert!(scripts.get(&s.id).await.unwrap().is_some(), "chua bi xoa");

        tasks
            .set_status("t1", TaskStatus::Completed, None, None)
            .await
            .unwrap();
        let task_ids = engine.delete_script(&s.id).await.unwrap();
        assert_eq!(task_ids, vec!["t1".to_string()]);
        assert!(scripts.get(&s.id).await.unwrap().is_none());
        // Xoa KHONG huy task (R3) — dong task van con nguyen.
        assert!(tasks.get("t1").await.unwrap().is_some());
    }
}
