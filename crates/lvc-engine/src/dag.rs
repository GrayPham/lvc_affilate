//! Bo thuc thi do thi (P3 Task 7): chay MOT lan workflow — validate+lap ke hoach (`lvc_core::graph::
//! plan`, da gom `validate`) -> tao dong `runs` -> phat `NodeState` "queued" cho MOI node -> chay
//! tung buoc THEO DUNG THU TU TOPO ma `RunPlan::steps` da co san (Prompt -> `ImageUpload` -> Video ->
//! Save) -> `RunDone` + `RunRepo::set_status`. Task 9 (Tauri `run_start`/`run_cancel`) goi
//! [`Engine::run_graph`]/[`Engine::cancel_run`].
//!
//! Ruling (`task-7-brief.md`, binding):
//! - Do thi khong hop le -> `Err`, KHONG tao dong `runs`.
//! - Run `failed` neu >= 1 canh video that bai — VAN Save cac canh OK (khong huy ca node vi 1 canh
//!   loi).
//! - Huy giua chung -> cac task con lai `failed` (ma `cancelled` — `TaskStatus` khong co bien the
//!   `Cancelled` rieng, xem `lvc_store::tasks`), run `cancelled` (uu tien hon `failed` du co the
//!   canh da loi truoc do vi ly do khac).
//!
//! # Thiet ke tu quyet dinh (khong co san trong brief — xem bao cao Task 7 de biet ly do day du)
//!
//! **Huy (khac vs doc `cancel.rs`)**: `cancel_run` PHAI la ham dong bo (brief ghim chu ky `fn
//! cancel_run(&self, run_id) -> bool`, khong `async`) nen KHONG the tu no doc `TaskRepo::list_by_run`
//! roi `cancels().cancel_many(..)` (can `.await`). Thay vao do, MOI job video nhan hooks rieng
//! ([`RunCancelHooks`]) boc token CUA RUN (khong phai token cua `CancelRegistry` theo `task_id`) —
//! `RunCancelHooks::should_cancel()` tra `true` ngay khi token cua run bi huy, VA `EngineHooks` cua
//! `generate_video` da OR san hai nguon huy (`self.token.is_cancelled() || self.inner.should_cancel()`,
//! xem `generate.rs`), nen job DANG cho slot (limiter) van thay huy o buoc kiem NGAY SAU KHI co slot
//! (`hooks.check("sau slot")`), con job dang poll thay huy qua `PollHooks::should_cancel()` (moi
//! nhip). Ca hai duong deu di qua nhanh loi binh thuong cua `generate_video` — TU NO ghi `Failed` +
//! `failure_code = cancelled` vao dong `tasks` TRUOC khi tra ve (xem `Engine::generate_video`), nen
//! khong co task nao "con queued": moi `TaskRow` da tao cho MOT job video LUON duoc `generate_video`
//! xu ly toi cung (`JoinSet` khong bao gio bi drop giua chung — xem duoi).
//!
//! **File luu**: ten `<run_id>_<video_node_id>_<scene_index>.mp4` — dung id NODE VIDEO NGUON
//! (`SaveJob::from_node`), KHONG dung id node Luu: mot node Luu co the nhan tu NHIEU node Tao video
//! (`plan.rs` test `save_mot_savejob_cho_moi_canh_vao`), dung id node Luu se lam trung ten file giua
//! cac node nguon khac nhau co cung `scene_index`.
//!
//! **Anh (`ImageUpload` -> `Save`)**: P3 chi kiem file ton tai + ghi `NodeOutputs.images` (duong dan
//! tho, P4 moi thuc su xu ly anh — xem `NodeKind::supported_in_p3`); Save voi nguon la anh la
//! no-op (khong copy), KHONG loi.
//!
//! **`JoinSet` khong bao gio bi bo giua chung**: neu vay, future dang chay bi abort — dong `tasks`
//! co the ket lai o `processing`/`queued` (vi pham ruling "khong con task nao queued"). Vong lap
//! chinh LUON `join_next()` toi khi `JoinSet` rong, ke ca sau khi phat hien huy.
//!
//! **id run/task**: `uuid` v4 (da la dependency workspace — xem `Cargo.toml`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lvc_browser::BrowserDriver;
use lvc_browser::dola::{NoopHooks, PollHooks, VideoArtifact};
use lvc_core::Scene;
use lvc_core::graph::{Graph, RunPlan, SaveJob, Step, VideoJob, plan};
use lvc_store::{BatchRef, RunStatus};
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::error::EngineError;
use crate::events::EngineEvent;
use crate::generate::{Engine, GenerateOutcome};
use crate::now_unix;

/// Tu vung trang thai node phat qua `EngineEvent::NodeState::state` (khop doc cua bien the do).
pub const NODE_QUEUED: &str = "queued";
pub const NODE_RUNNING: &str = "running";
pub const NODE_DONE: &str = "done";
pub const NODE_FAILED: &str = "failed";

/// Hooks cho MOT job video, tao theo `task_id` — Task 9 truyen factory rieng cho UI (theo doi tien
/// do tung canh); mac dinh [`noop_hooks_factory`]. `Fn` (khong phai `FnOnce`) vi goi lai cho MOI
/// canh trong cung mot lan chay.
pub type HooksFactory = Arc<dyn Fn(&str) -> Box<dyn PollHooks + Send + Sync> + Send + Sync>;

/// Factory mac dinh: hooks khong lam gi (dung cho test/CLI khong can theo doi tien do tung job).
#[must_use]
pub fn noop_hooks_factory() -> HooksFactory {
    Arc::new(|_task_id: &str| Box::new(NoopHooks) as Box<dyn PollHooks + Send + Sync>)
}

/// Dau ra da gom cua tung node trong MOT lan chay (bo nho tam thoi, khong luu DB — dong `tasks`
/// da co rieng cho video; day chi de node SAU doc dau ra node TRUOC trong cung mot lan chay).
#[derive(Debug, Default)]
pub struct NodeOutputs {
    pub texts: HashMap<String, Vec<Scene>>,
    pub images: HashMap<String, Vec<PathBuf>>,
    /// `(scene_index, VideoArtifact)` — da sap theo `scene_index` (xem `run_video_step`).
    pub videos: HashMap<String, Vec<(u32, VideoArtifact)>>,
}

/// Tay cam mot lan chay (`Engine::run_graph` tra ve NGAY, khong doi chay xong). `join` de test/
/// Task 9 doi run ket thuc; `token` de tu huy KHONG qua `Engine::cancel_run` (vd. dong app — huy
/// moi run dang co).
#[derive(Debug)]
pub struct RunHandle {
    pub run_id: String,
    pub token: CancellationToken,
    pub join: JoinHandle<()>,
}

/// Hooks boc MOT job video: OR token cua RUN vao nguon huy, chuyen tiep phan con lai cho hooks cua
/// caller (`hooks_factory`) — xem doc module ve ly do day la co che huy CHINH (khong qua
/// `CancelRegistry`).
struct RunCancelHooks {
    run_token: CancellationToken,
    inner: Box<dyn PollHooks + Send + Sync>,
}

impl PollHooks for RunCancelHooks {
    fn on_poll(&self, elapsed: Duration) {
        self.inner.on_poll(elapsed);
    }
    fn on_stage(&self, stage: &str) {
        self.inner.on_stage(stage);
    }
    fn on_agent_reply(&self, text: &str) {
        self.inner.on_agent_reply(text);
    }
    fn should_cancel(&self) -> bool {
        self.run_token.is_cancelled() || self.inner.should_cancel()
    }
}

/// Ket qua MOT job video da chay xong (`JoinSet` gom lai).
struct VideoJobResult {
    scene_index: u32,
    result: Result<GenerateOutcome, EngineError>,
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4())
}

/// Gom MOI `Step::Save` LIEN TIEP tu vi tri `i` cung `node_id` (`lvc_core::graph::plan` sinh chung
/// mot cho khi tham MOT node Luu — xem doc `plan::plan`) thanh MOT nhom; tra `(node_id, jobs, i moi
/// — vi tri NGAY SAU nhom, de vong lap ngoai tiep tuc tu do)`. `run_plan.steps[i]` PHAI la
/// `Step::Save` (goi tu nhanh `match` da khop bien the nay).
fn collect_save_group(run_plan: &RunPlan, mut i: usize) -> (String, Vec<SaveJob>, usize) {
    let Step::Save(first) = &run_plan.steps[i] else {
        unreachable!("collect_save_group: steps[i] phai la Step::Save")
    };
    let node_id = first.node_id.clone();
    let mut jobs: Vec<SaveJob> = Vec::new();
    while i < run_plan.steps.len() {
        let Step::Save(job) = &run_plan.steps[i] else {
            break;
        };
        if job.node_id != node_id {
            break;
        }
        jobs.push(job.clone());
        i += 1;
    }
    (node_id, jobs, i)
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Chay MOT lan workflow (brief Task 7). Loi lap ke hoach (`lvc_core::graph::plan`, gom ca
    /// `validate`) tra `Err` NGAY — KHONG tao dong `runs` (ruling). Tra [`RunHandle`] ngay lap tuc;
    /// chay nen qua `tokio::spawn` — `RunHandle::join` de test/Task 9 doi run ket thuc.
    ///
    /// # Errors
    /// `EngineError::Graph` khi do thi khong hop le / loi lap ke hoach.
    #[allow(
        clippy::unused_async,
        reason = "giu async de khop chu ky cong khai cua brief Task 7 - Task 9 se goi ham nay tu \
                  mot Tauri command async, va than than vong chay nen (run_graph_task) can await"
    )]
    pub async fn run_graph(
        self: &Arc<Self>,
        workflow_id: &str,
        graph: &Graph,
        hooks_factory: HooksFactory,
    ) -> Result<RunHandle, EngineError> {
        let run_plan = plan(graph, &self.limits)?;
        let run_id = new_id("run");
        let token = CancellationToken::new();
        self.run_tokens.insert(run_id.clone(), token.clone());

        let node_ids: Vec<String> = graph.nodes.iter().map(|n| n.id.clone()).collect();
        let engine = Arc::clone(self);
        let workflow_id = workflow_id.to_string();
        let run_id_bg = run_id.clone();
        let token_bg = token.clone();
        let join = tokio::spawn(async move {
            engine
                .run_graph_task(
                    run_id_bg,
                    workflow_id,
                    node_ids,
                    run_plan,
                    token_bg,
                    hooks_factory,
                )
                .await;
        });

        Ok(RunHandle {
            run_id,
            token,
            join,
        })
    }

    /// Huy MOT lan chay dang co (dong bo — xem doc module ve ly do). Tra `false` khi khong biet
    /// `run_id` (chua chay/da xong — `run_tokens` chi giu run DANG chay, xem `run_graph_task`).
    #[must_use = "false = khong biet run_id nay (chua chay/da xong)"]
    pub fn cancel_run(&self, run_id: &str) -> bool {
        match self.run_tokens.get(run_id) {
            Some(tok) => {
                tok.cancel();
                true
            }
            None => false,
        }
    }

    /// `counts = (done, total, ok, failed)` — gop lai de tranh `clippy::too_many_arguments` (7 la
    /// tran mac dinh, ham nay von co 4 con so cung don vi `u32` de rieng rat de nham thu tu khi
    /// doc call site).
    fn emit_node_state(
        &self,
        run_id: &str,
        node_id: &str,
        state: &str,
        counts: (u32, u32, u32, u32),
    ) {
        let (done, total, ok, failed) = counts;
        self.events().emit(EngineEvent::NodeState {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            state: state.to_string(),
            done,
            total,
            ok,
            failed,
        });
    }

    /// Than vong chay nen cua [`Engine::run_graph`] — KHONG tra `Result` (da tach nhanh loi lap ke
    /// hoach ra ngoai, truoc khi spawn); moi loi o day chi lam RUN NAY that bai, khong panic.
    async fn run_graph_task(
        self: Arc<Self>,
        run_id: String,
        workflow_id: String,
        node_ids: Vec<String>,
        run_plan: RunPlan,
        token: CancellationToken,
        hooks_factory: HooksFactory,
    ) {
        let started = now_unix();
        if let Err(e) = self
            .run_repo
            .create(&run_id, &workflow_id, RunStatus::Running, started)
            .await
        {
            warn!(run_id, error = %e, "khong tao duoc dong runs - dung run nay");
            self.run_tokens.remove(&run_id);
            return;
        }
        for node_id in &node_ids {
            self.emit_node_state(&run_id, node_id, NODE_QUEUED, (0, 0, 0, 0));
        }

        let mut outputs = NodeOutputs::default();
        let mut any_failed = false;
        let mut hard_stop: Option<String> = None;

        let mut i = 0usize;
        while i < run_plan.steps.len() {
            if hard_stop.is_some() {
                break;
            }
            match &run_plan.steps[i] {
                Step::Prompt { node_id, scenes } => {
                    self.emit_node_state(&run_id, node_id, NODE_RUNNING, (0, 1, 0, 0));
                    outputs.texts.insert(node_id.clone(), scenes.clone());
                    self.emit_node_state(&run_id, node_id, NODE_DONE, (1, 1, 1, 0));
                    i += 1;
                }
                Step::ImageUpload { node_id, paths } => {
                    self.emit_node_state(&run_id, node_id, NODE_RUNNING, (0, 1, 0, 0));
                    match missing_paths(paths).await {
                        None => {
                            outputs
                                .images
                                .insert(node_id.clone(), paths.iter().map(PathBuf::from).collect());
                            self.emit_node_state(&run_id, node_id, NODE_DONE, (1, 1, 1, 0));
                        }
                        Some(missing) => {
                            self.emit_node_state(&run_id, node_id, NODE_FAILED, (1, 1, 0, 1));
                            hard_stop = Some(format!("node {node_id}: thieu file anh: {missing}"));
                        }
                    }
                    i += 1;
                }
                Step::Video { node_id, jobs } => {
                    let node_id = node_id.clone();
                    let failed = self
                        .run_video_step(
                            &run_id,
                            &node_id,
                            jobs,
                            &token,
                            &hooks_factory,
                            &mut outputs,
                        )
                        .await;
                    any_failed |= failed;
                    i += 1;
                }
                Step::Save(_) => {
                    let (node_id, jobs, next_i) = collect_save_group(&run_plan, i);
                    i = next_i;
                    let failed = self
                        .run_save_group(&run_id, &node_id, &jobs, &outputs)
                        .await;
                    any_failed |= failed;
                }
            }
        }

        self.finish_run(run_id, &token, hard_stop, any_failed).await;
    }

    /// Duoc quyet dinh trang thai cuoi (huy > loi cung/canh > hoan tat), ghi `RunRepo::set_status`,
    /// go token khoi `run_tokens`, roi phat `RunDone` (SAU khi DB da ghi — ruling brief).
    async fn finish_run(
        &self,
        run_id: String,
        token: &CancellationToken,
        hard_stop: Option<String>,
        any_failed: bool,
    ) {
        let status = if token.is_cancelled() {
            RunStatus::Cancelled
        } else if hard_stop.is_some() || any_failed {
            RunStatus::Failed
        } else {
            RunStatus::Completed
        };
        let finished = now_unix();
        if let Err(e) = self
            .run_repo
            .set_status(&run_id, status, hard_stop.as_deref(), finished)
            .await
        {
            warn!(run_id, error = %e, "khong ghi duoc trang thai run");
        }
        self.run_tokens.remove(&run_id);
        self.events().emit(EngineEvent::RunDone {
            run_id,
            status: status.as_str().to_string(),
        });
    }

    /// Buoc `Video`: tao MOT `TaskRow` cho MOI canh TRUOC KHI spawn (de UI liet ke duoc ngay), roi
    /// `JoinSet` chay dong thoi (limiter/`AccountPool` lo song song — xem `generate.rs`); mot canh
    /// loi KHONG dung cac canh khac (ruling). Ghi `NodeOutputs.videos[node_id]` = cac canh THANH
    /// CONG (sap theo `scene_index`) — canh loi khong co mat, Save (buoc sau) tu dong chi luu canh
    /// OK. Tra `true` khi CO IT NHAT 1 canh loi.
    async fn run_video_step(
        self: &Arc<Self>,
        run_id: &str,
        node_id: &str,
        jobs: &[VideoJob],
        run_token: &CancellationToken,
        hooks_factory: &HooksFactory,
        outputs: &mut NodeOutputs,
    ) -> bool {
        let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        self.emit_node_state(run_id, node_id, NODE_RUNNING, (0, total, 0, 0));

        let mut set: JoinSet<VideoJobResult> = JoinSet::new();
        let mut pre_fail = 0u32;
        for job in jobs {
            let task_id = new_id("task");
            let batch = BatchRef {
                batch_id: run_id.to_string(),
                batch_name: node_id.to_string(),
                scene_index: i64::from(job.scene_index),
            };
            if let Err(e) = self.tasks.create(&task_id, &job.scene, Some(&batch)).await {
                warn!(run_id, node_id, error = %e, "khong tao duoc dong task cho canh");
                pre_fail += 1;
                continue;
            }
            if let Err(e) = self.tasks.set_run(&task_id, run_id, node_id).await {
                warn!(run_id, node_id, task_id, error = %e, "khong gan duoc run_id/node_id");
            }

            let engine = Arc::clone(self);
            let scene = job.scene.clone();
            let scene_index = job.scene_index;
            let hooks = RunCancelHooks {
                run_token: run_token.clone(),
                inner: hooks_factory(&task_id),
            };
            set.spawn(async move {
                let result = engine.generate_video(&task_id, &scene, None, &hooks).await;
                VideoJobResult {
                    scene_index,
                    result,
                }
            });
        }

        let mut ok = 0u32;
        let mut failed = pre_fail;
        let mut artifacts: Vec<(u32, VideoArtifact)> = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(VideoJobResult {
                    scene_index,
                    result: Ok(out),
                }) => {
                    ok += 1;
                    artifacts.push((scene_index, out.artifact));
                }
                Ok(VideoJobResult { result: Err(_), .. }) => {
                    failed += 1;
                }
                Err(join_err) => {
                    warn!(run_id, node_id, error = %join_err, "job video ket thuc bat thuong (panic)");
                    failed += 1;
                }
            }
        }
        artifacts.sort_by_key(|(idx, _)| *idx);
        outputs.videos.insert(node_id.to_string(), artifacts);

        let state = if failed > 0 { NODE_FAILED } else { NODE_DONE };
        self.emit_node_state(run_id, node_id, state, (ok + failed, total, ok, failed));
        failed > 0
    }

    /// Buoc `Save`: gom MOI `Step::Save` LIEN TIEP cung `node_id` (do `lvc_core::graph::plan` sinh
    /// chung mot cho khi tham node Luu do — xem doc `plan::plan`) thanh MOT `NodeState`. Nguon la
    /// video -> copy (KHONG di chuyen) tung file vao `out_dir`; nguon la anh -> no-op (P4, xem doc
    /// module); nguon khong co dau ra (buoc truoc bi bo qua/loi ha tang) -> coi la loi. Tra `true`
    /// khi CO IT NHAT 1 job loi.
    async fn run_save_group(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: &[SaveJob],
        outputs: &NodeOutputs,
    ) -> bool {
        self.emit_node_state(run_id, node_id, NODE_RUNNING, (0, 1, 0, 0));
        let mut ok = true;
        for job in jobs {
            if let Some(artifacts) = outputs.videos.get(&job.from_node) {
                if let Err(e) =
                    save_videos(run_id, &job.from_node, artifacts, Path::new(&job.out_dir)).await
                {
                    warn!(run_id, node_id, from = %job.from_node, error = %e, "khong luu duoc video");
                    ok = false;
                }
            } else if outputs.images.contains_key(&job.from_node) {
                // P4: chua copy anh — chi la duong dan (xem doc module).
            } else {
                warn!(run_id, node_id, from = %job.from_node, "khong co dau ra de luu");
                ok = false;
            }
        }
        let (state, o, f) = if ok {
            (NODE_DONE, 1, 0)
        } else {
            (NODE_FAILED, 0, 1)
        };
        self.emit_node_state(run_id, node_id, state, (1, 1, o, f));
        !ok
    }
}

/// Copy (khong di chuyen) tung `VideoArtifact.local_path` vao
/// `out_dir/<run_id>_<from_node>_<scene_index>.mp4` (tao `out_dir` neu chua co).
async fn save_videos(
    run_id: &str,
    from_node: &str,
    artifacts: &[(u32, VideoArtifact)],
    out_dir: &Path,
) -> std::io::Result<()> {
    tokio::fs::create_dir_all(out_dir).await?;
    for (scene_index, artifact) in artifacts {
        let dest = out_dir.join(format!("{run_id}_{from_node}_{scene_index}.mp4"));
        tokio::fs::copy(&artifact.local_path, &dest).await?;
    }
    Ok(())
}

/// `None` khi MOI duong dan ton tai; `Some(danh sach, cach nhau ", ")` khi co duong dan thieu.
async fn missing_paths(paths: &[String]) -> Option<String> {
    let mut missing = Vec::new();
    for p in paths {
        if tokio::fs::metadata(p).await.is_err() {
            missing.push(p.clone());
        }
    }
    if missing.is_empty() {
        None
    } else {
        Some(missing.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use lvc_browser::dola::{ConversationId, poll::NOTE_RAW};
    use lvc_browser::{DolaError, FakeCall, FakeDriver, FakeStep};
    use lvc_core::{
        Edge, FailureCode, Limits, LogoMode, Mode, Model, Node, NodeKind, Ratio, TaskStatus,
    };
    use lvc_store::{AccountRepo, Db, TaskRepo};
    use tokio::sync::broadcast;

    use super::*;
    use crate::events::next_event;
    use crate::generate::EngineConfig;

    // Ghi chu thiet ke test (bao cao Task 7 co ban day du): `FakeDriver` dung MOT `VecDeque` KICH
    // BAN DUNG CHUNG cho moi job (xem doc `lvc-browser/src/fake.rs`) — an toan CHI KHI toi da 1 job
    // dung driver cung mot luc, vi cac loai buoc (Launch/Submit/Poll) phai khop dung thu tu tung
    // ham goi. Voi `threads=1` (mot slot duy nhat, gio SUOT job — xem `limiter.rs`), cac job KHAC
    // NHAU khong bao gio goi driver xen ke (da co tien le `generate::tests::
    // limiter_threads_1_hai_job_chay_tuan_tu`), nen moi test o day dung `threads=1`. Doi lai, THU
    // TU cac job gianh duoc slot DAU TIEN la khong doan truoc duoc (`DynamicLimiter` khong FIFO —
    // xem `limiter.rs`), nen cac assertion duoi day khong gia dinh scene_index N ung voi lan chay
    // thu N; chi kiem tong hop (dem so ok/failed, doi chieu file da luu voi trang thai tung dong
    // `TaskRow`). Day la ly do test o day KHONG lam dung "2 tai khoan/threads=2/toi da 2 dong thoi"
    // nhu goi y trong brief — xem bao cao Task 7, muc "concerns".
    async fn build_engine(
        accounts: &[&str],
        threads: usize,
        steps: Vec<FakeStep>,
    ) -> (
        tempfile::TempDir,
        Arc<Engine<FakeDriver>>,
        Db,
        Arc<FakeDriver>,
    ) {
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
        let (engine, _handle) = Engine::new(
            Arc::clone(&driver),
            db.clone(),
            Limits::default(),
            cfg,
            threads,
        )
        .await
        .unwrap();
        (dir, Arc::new(engine), db, driver)
    }

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

    /// Do thi Prompt("pr") -> Tao video("vid") -> Luu("sv"), dung o hau het test.
    fn linear_graph(prompt_text: &str, out_dir: &std::path::Path) -> Graph {
        Graph {
            version: 1,
            nodes: vec![
                node(
                    "pr",
                    NodeKind::Prompt {
                        text: prompt_text.to_string(),
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

    /// `n` bo (Launch, Submit, Poll) thanh cong, deu tra ve `VideoArtifact` tro toi CUNG `src`
    /// (noi dung khong quan trong cho cac assertion — xem ghi chu thiet ke o dau module test).
    fn happy_steps(n: usize, src: &std::path::Path) -> Vec<FakeStep> {
        let mut v = Vec::new();
        for i in 0..n {
            v.push(FakeStep::Launch(Ok(())));
            v.push(FakeStep::Submit(Ok(ConversationId(format!("c{i}")))));
            v.push(FakeStep::Poll(Ok(artifact(src))));
        }
        v
    }

    /// Doc het su kien toi khi thay `RunDone` (bao gom).
    async fn drain_until_run_done(rx: &mut broadcast::Receiver<EngineEvent>) -> Vec<EngineEvent> {
        let mut out = Vec::new();
        loop {
            let ev = next_event(rx).await.expect("kenh su kien con mo");
            let done = matches!(ev, EngineEvent::RunDone { .. });
            out.push(ev);
            if done {
                break;
            }
        }
        out
    }

    fn node_state(
        run_id: &str,
        node_id: &str,
        state: &str,
        done: u32,
        total: u32,
        ok: u32,
        failed: u32,
    ) -> EngineEvent {
        EngineEvent::NodeState {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            state: state.to_string(),
            done,
            total,
            ok,
            failed,
        }
    }

    // ---- (1) 3 canh thanh cong: 3 TaskRow, 3 file luu, NodeState/RunDone ghim dung thu tu ----

    #[tokio::test]
    async fn video_3_canh_thanh_cong_luu_3_file_va_phat_su_kien_dung() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4-bytes").unwrap();

        let (dir, engine, db, driver) =
            build_engine(&["acc1", "acc2", "acc3"], 1, happy_steps(3, &src)).await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1\ncanh 2\ncanh 3", &out_dir);

        let mut rx = engine.events().subscribe();
        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();
        handle.join.await.unwrap();

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Completed);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert_eq!(rows.len(), 3);
        for r in &rows {
            assert_eq!(r.run_id, run_id);
            assert_eq!(r.node_id, "vid");
            assert_eq!(r.status, TaskStatus::Completed, "{r:?}");
        }
        let mut idxs: Vec<i64> = rows.iter().map(|r| r.scene_index.unwrap()).collect();
        idxs.sort_unstable();
        assert_eq!(idxs, vec![1, 2, 3]);

        for i in 1..=3 {
            let f = out_dir.join(format!("{run_id}_vid_{i}.mp4"));
            assert!(f.exists(), "{f:?} phai ton tai");
            assert_eq!(std::fs::read(&f).unwrap(), b"fake-mp4-bytes");
        }

        let calls = driver.calls.lock().unwrap().clone();
        assert_eq!(
            calls.len(),
            9,
            "3 job x 3 loi goi = 9, khong xen ke (threads=1)"
        );
        for chunk in calls.chunks(3) {
            assert!(matches!(chunk[0], FakeCall::Launch { .. }), "{chunk:?}");
            assert!(matches!(chunk[1], FakeCall::Submit { .. }), "{chunk:?}");
            assert!(matches!(chunk[2], FakeCall::Poll { .. }), "{chunk:?}");
        }

        // Loc chi NodeState/RunDone — kenh su kien con mang ca TaskStage/TaskDone (Task 4, tung
        // job video rieng phat, khong thuoc pham vi ghim thu tu cua Task 7).
        let events: Vec<EngineEvent> = drain_until_run_done(&mut rx)
            .await
            .into_iter()
            .filter(|e| {
                matches!(
                    e,
                    EngineEvent::NodeState { .. } | EngineEvent::RunDone { .. }
                )
            })
            .collect();
        let expected = vec![
            node_state(&run_id, "pr", NODE_QUEUED, 0, 0, 0, 0),
            node_state(&run_id, "vid", NODE_QUEUED, 0, 0, 0, 0),
            node_state(&run_id, "sv", NODE_QUEUED, 0, 0, 0, 0),
            node_state(&run_id, "pr", NODE_RUNNING, 0, 1, 0, 0),
            node_state(&run_id, "pr", NODE_DONE, 1, 1, 1, 0),
            node_state(&run_id, "vid", NODE_RUNNING, 0, 3, 0, 0),
            node_state(&run_id, "vid", NODE_DONE, 3, 3, 3, 0),
            node_state(&run_id, "sv", NODE_RUNNING, 0, 1, 0, 0),
            node_state(&run_id, "sv", NODE_DONE, 1, 1, 1, 0),
            EngineEvent::RunDone {
                run_id: run_id.clone(),
                status: "completed".to_string(),
            },
        ];
        assert_eq!(events, expected);
    }

    // ---- (2) 1/3 canh that bai: run failed, 2 file duoc luu, dong task loi co failure_code ----

    #[tokio::test]
    async fn video_1_canh_that_bai_run_failed_van_luu_2_canh_ok() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"ok-bytes").unwrap();

        let mut steps = happy_steps(2, &src);
        steps.push(FakeStep::Launch(Ok(())));
        steps.push(FakeStep::Submit(Err(DolaError::NoConversation(
            "khong ket noi duoc".into(),
        ))));

        let (dir, engine, db, _driver) = build_engine(&["acc1", "acc2", "acc3"], 1, steps).await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1\ncanh 2\ncanh 3", &out_dir);

        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();
        handle.join.await.unwrap();

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Failed);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert_eq!(rows.len(), 3);
        let failed_rows: Vec<_> = rows
            .iter()
            .filter(|r| r.status == TaskStatus::Failed)
            .collect();
        assert_eq!(failed_rows.len(), 1, "{rows:?}");
        assert_eq!(
            failed_rows[0].failure_code,
            Some(FailureCode::NoConversation)
        );
        let ok_rows: Vec<_> = rows
            .iter()
            .filter(|r| r.status == TaskStatus::Completed)
            .collect();
        assert_eq!(ok_rows.len(), 2, "{rows:?}");

        for r in &rows {
            let f = out_dir.join(format!("{run_id}_vid_{}.mp4", r.scene_index.unwrap()));
            assert_eq!(f.exists(), r.status == TaskStatus::Completed, "{r:?}");
        }
    }

    // ---- (3) huy giua chung: cac task con lai failed/cancelled, run cancelled ----

    #[tokio::test]
    async fn huy_giua_chung_cac_task_con_lai_bi_huy_run_cancelled() {
        let steps = vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(ConversationId("c1".into()))),
            FakeStep::PollUntilCancelled,
        ];
        let (dir, engine, db, driver) = build_engine(&["acc1", "acc2", "acc3"], 1, steps).await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1\ncanh 2\ncanh 3", &out_dir);

        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();

        // doi job dau tien vao han PollUntilCancelled (tick 10 ms) roi moi huy — dung nghia "huy
        // giua chung" hon huy ngay tu dau. 100 ms (tran brief cho phep) de bot nhay duoi tai chay
        // song song nhieu test trong cung binary — van co the la 0 (xem assertion calls_len duoi).
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(engine.cancel_run(&run_id));

        handle.join.await.unwrap();

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Cancelled);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert_eq!(rows.len(), 3, "ca 3 dong task deu duoc tao TRUOC khi spawn");
        for r in &rows {
            assert_eq!(r.status, TaskStatus::Failed, "{r:?}");
            assert_eq!(r.failure_code, Some(FailureCode::Cancelled), "{r:?}");
        }
        assert!(
            !out_dir.exists() || out_dir.read_dir().unwrap().next().is_none(),
            "khong file nao duoc luu (khong canh nao thanh cong)"
        );

        // Duoi tai (nhieu test khac chay song song trong cung binary) job dau tien co the CHUA
        // kip toi PollUntilCancelled truoc khi ta huy o tren — khi do ca 3 job deu bi huy truoc
        // Launch (0 loi goi). Binh thuong (job dau kip chay) thi dung 1 job cham driver (3 loi goi:
        // Launch+Submit+Poll) — 2 job con lai luon bi huy TRUOC khi mo Chrome (IDEA-016). Khong bao
        // gio > 3: script chi du cho DUNG 1 job, job thu 2 cham driver se panic (kich ban het).
        let calls_len = driver.calls.lock().unwrap().len();
        assert!(
            calls_len == 0 || calls_len == 3,
            "calls_len={calls_len} (mong 0 hoac 3 — xem ghi chu o tren)"
        );

        // run da ket thuc -> khong con trong run_tokens -> cancel lan nua tra false.
        assert!(!engine.cancel_run(&run_id));
        assert!(!engine.cancel_run("khong-ton-tai"));
    }

    // ---- (4) do thi khong hop le: Err, KHONG tao dong runs ----

    #[tokio::test]
    async fn do_thi_khong_hop_le_tra_err_khong_tao_run() {
        let (_dir, engine, _db, _driver) = build_engine(&[], 1, vec![]).await;
        let empty_graph = Graph {
            version: 1,
            nodes: vec![],
            edges: vec![],
        };
        let err = engine
            .run_graph("wf1", &empty_graph, noop_hooks_factory())
            .await
            .unwrap_err();
        assert!(matches!(err, EngineError::Graph(_)), "{err:?}");

        let recent = engine.run_repo.list_recent(10).await.unwrap();
        assert!(recent.is_empty(), "{recent:?}");
    }

    // ---- (5) ImageUpload thieu file: run failed, khong tao task video nao ----

    #[tokio::test]
    async fn image_upload_thieu_file_run_failed_khong_tao_task_video() {
        let (dir, engine, db, driver) = build_engine(&[], 1, vec![]).await;
        let out_dir = dir.path().join("out");
        let missing = dir.path().join("khong_ton_tai.jpg");
        let graph = Graph {
            version: 1,
            nodes: vec![
                node(
                    "pr",
                    NodeKind::Prompt {
                        text: "canh 1".to_string(),
                    },
                ),
                node(
                    "im",
                    NodeKind::ImageUpload {
                        paths: vec![missing.display().to_string()],
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
                edge("e2", "im", "out", "vid", "refs"),
                edge("e3", "vid", "out", "sv", "video"),
            ],
        };

        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();
        handle.join.await.unwrap();

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Failed);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert!(rows.is_empty(), "khong task video nao duoc tao: {rows:?}");
        assert!(driver.calls.lock().unwrap().is_empty());
    }

    // ---- cac test nho bo tro ----

    #[tokio::test]
    async fn cancel_run_khong_biet_id_tra_false() {
        let (_dir, engine, _db, _driver) = build_engine(&[], 1, vec![]).await;
        assert!(!engine.cancel_run("khong-ton-tai"));
    }
}
