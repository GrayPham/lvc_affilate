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
//! # Fix-round 1 (review `task-7-review.md`, commit `e6b9a82`)
//!
//! **C1 (Critical, sua)**: ban dau `cancel_run` chi huy mot token RIENG (`RunCancelHooks`) duoc OR
//! vao CAC DIEM KIEM DONG BO cua `generate_video` (`should_cancel()`/`check(..)`) — nhung BA
//! `tokio::select!` cho async (cho slot `DynamicLimiter`, `lease_wait` mot tai khoan chi dinh, ngu
//! giua nhip doi tai khoan ban) chi bam token CUA `CancelRegistry` (`hooks.token`, dang ky BEN
//! TRONG `generate_video`), nen mot canh bi chan o do KHONG BAO GIO thay `cancel_run` — run co the
//! "running" vo thoi han neu mot job KHAC (run khac / resume / P2 CLI) dang giu het permit. Sua
//! bang [`crate::cancel::CancelRegistry::register_child`] (moi, `cancel.rs`): `run_video_step` dang
//! ky token cua TUNG canh la CON cua token cua RUN TRUOC KHI spawn; `generate_video` tu goi
//! `register_guarded` (get-or-insert) va nhan lai DUNG token con nay — `tokio-util` tu lan truyen
//! huy tu cha xuong con nen ca ba `select!` do gio thay `cancel_run` NGAY, khong can doi. Xoa
//! `RunCancelHooks` (het tac dung — hooks cua caller truyen thang, khong boc them).
//!
//! **I1 (sua)**: run `failed` do canh loi gio ghi `runs.error` neu ten canh that bai (vd `"node
//! vid: canh 1,3 that bai"`), thay vi chuoi rong.
//!
//! **I2 (sua)**: vong lap buoc dung NGAY khi `token.is_cancelled()` (kiem o dau vong lap NGOAI, va
//! LAI o dau vong lap tao dong task trong `run_video_step` — huy co the den GIUA luc dang tao dong
//! cho nhieu canh cua CUNG mot buoc Video). Node nao chua kip chay toi (con "queued") khi vong lap
//! dung som duoc phat MOT `NodeState` cuoi (`cancelled` neu do huy, `failed` neu do `hard_stop`)
//! thay vi bo mac o "queued" mai mai.
//!
//! **I3 (sua)**: than vong buoc (goi `hooks_factory` cua Task 9 — co the panic) chay trong MOT
//! `tokio::spawn` con rieng; `Err(JoinError)` (panic) duoc bat va coi nhu run that bai voi thong
//! diep loi, KHONG de dong `runs` ket "running" mai mai / `run_tokens` ri rac / khong `RunDone`.
//!
//! **I4**: test 3 (huy giua chung) doi su kien `TaskStage{stage:"generating"}` thay vi `sleep` co
//! dinh truoc khi goi `cancel_run` — het "0 hoac 3 loi goi" (co the la 2 duoi tai), luon dung 3.
//!
//! **M1/M3/M4 (sua)**: `HooksFactory` bo bound `Send + Sync` thua tren `Box<dyn PollHooks>`
//! (`PollHooks: Send + Sync` da la supertrait — `poll.rs:177`); canh bi huy ma CHUA BAO GIO thuc
//! su chay (bi bat o `check("sau slot")`/lease/doi tai khoan ban truoc khi mo Chrome) cung duoc ghi
//! `finished_at` (khoang trong co san cua `generate_video`, gioi han sua o day cho dung nhom canh
//! nay — xem bao cao); su kien `NodeState` "queued" ban dau mang dung `total` (so canh cua node,
//! tra tu `RunPlan` qua [`node_totals`]) thay vi luon `0`.
//!
//! # Fix-round 2 (re-review `task-7-review-r1.md`, commit `f0801cf`)
//!
//! **I3-NEW (moi, sua)**: fix-round 1 chi bat panic o TAM RUN (than vong `run_steps`), nhung
//! `hooks_factory(&task_id)` (`run_video_step`) van duoc goi SAU KHI dong `TaskRow` cua canh do da
//! ton tai (`TaskRepo::create` + `set_run`) — panic o do de lai dong "queued" mo coi MAI MAI (khong
//! con code nao con dong lai) VA, voi buoc Video nhieu canh, co the huy giua chung cac job ANH EM
//! da spawn TRUOC do (drop `JoinSet` giua chung — dung DIEU module nay cam). Sua bang cach CHIA HAI
//! GIAI DOAN trong [`Engine::run_video_step`]: giai doan 1 goi `hooks_factory` + dang ky token con
//! (`register_child`) cho TAT CA canh TRUOC, KHONG dung `TaskRepo`; chi khi ca buoc da chuan bi xong
//! (khong panic nao xay ra) giai doan 2 moi tao dong + spawn. Panic trong giai doan 1 nghia la CHUA
//! CO dong nao ton tai cho buoc nay, va van nam tren CUNG than vong dong bo voi `run_steps` (chua
//! spawn job nao) nen lan thang len toi cai bay panic cua I3 (`tokio::spawn(steps_fut)` trong
//! `run_graph_task`) — run ket thuc `failed` sach se, khong dong mo coi, khong `JoinSet` nao bi bo.
//!
//! **M5 (tu Minor "tuy Task 9" thanh Important, sua)**: `TaskRepo::set_run` loi (dong da `create`
//! nhung KHONG gan duoc `run_id`/`node_id`) truoc day chi `warn!` roi VAN spawn — dong do vo hinh
//! voi `TaskRepo::list_by_run` (Task 9) MAI MAI du job co the van chay xong. Sua: danh dau NGAY
//! `Failed` + `finished_at` (`Engine::mark_row_failed`, giong huong xu ly canh bi huy — M3), KHONG
//! spawn, tinh la 1 canh that bai (ten canh van vao `runs.error` — I1).
//!
//! **Doc (2 minor)**: doc them o [`Engine::run_steps`] (buoc Save NGAY SAU mot buoc Video vua bi
//! huy cung bi bo qua — day la CO Y, khong phai loi: file cua cac canh da `Completed` VAN con trong
//! `downloads_dir`, duong dan van tren dong `task` cua chung, chi khong duoc copy sang `out_dir`
//! cua node Luu); `generate.rs` sua lai chu thich cu con nhac `dag::RunCancelHooks` (da bi xoa o
//! fix-round 1) thanh `register_child`.
//!
//! # Thiet ke tu quyet dinh con lai (xem bao cao Task 7 de biet ly do day du)
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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lvc_browser::BrowserDriver;
use lvc_browser::dola::{NoopHooks, PollHooks, VideoArtifact};
use lvc_core::graph::{Graph, RunPlan, SaveJob, Step, VideoJob, plan};
use lvc_core::{FailureCode, Scene, TaskStatus};
use lvc_store::{BatchRef, RunStatus};
use tokio::sync::watch;
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
/// Fix-round 1 (I2): trang thai cuoi cho mot node CHUA KIP CHAY khi run bi huy giua chung (phan
/// biet voi [`NODE_FAILED`] — do khong phai node do tu that bai, ma do vong lap dung som).
pub const NODE_CANCELLED: &str = "cancelled";

/// Hooks cho MOT job video, tao theo `task_id` — Task 9 truyen factory rieng cho UI (theo doi tien
/// do tung canh); mac dinh [`noop_hooks_factory`]. `Fn` (khong phai `FnOnce`) vi goi lai cho MOI
/// canh trong cung mot lan chay. Fix-round 1 (M1): `Box<dyn PollHooks>` KHONG can ghi them
/// `+ Send + Sync` — `PollHooks: Send + Sync` da la supertrait (`lvc_browser::dola::poll::
/// PollHooks`), nen moi kieu cu the impl no da Send+Sync san.
pub type HooksFactory = Arc<dyn Fn(&str) -> Box<dyn PollHooks> + Send + Sync>;

/// Factory mac dinh: hooks khong lam gi (dung cho test/CLI khong can theo doi tien do tung job).
#[must_use]
pub fn noop_hooks_factory() -> HooksFactory {
    Arc::new(|_task_id: &str| Box::new(NoopHooks) as Box<dyn PollHooks>)
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
///
/// `tasks_ready` (fix-round 1, P3c Task 1, I1): kenh `watch` bao "dong `TaskRow` cua (cac) buoc
/// Video da tao xong" — gia tri la `Vec<TaskRef>` TICH LUY qua nhieu buoc Video trong CUNG mot lan
/// chay (do thi thuong chi co 1, nhung khong gia dinh dieu do), RONG luc dau. Nguoi doi
/// (`lvc_engine::scripts::run_script`) chi can `changed()` MOT LAN (khong con poll `TaskRepo` moi
/// 10ms) — xem doc [`Engine::run_steps`]/[`Engine::run_video_step`] ve DUNG thoi diem gui: NGAY SAU
/// khi dong DB da tao xong cho MOT buoc Video, TRUOC KHI spawn job tao video that (khong doi video
/// tao xong); VA mot lan "chot" khong dieu kien o cuoi `run_steps` (du khong co buoc Video nao ca,
/// vd do thi `hard_stop` truoc do) de nguoi doi khong bao gio cho vo thoi han neu run KHONG BAO GIO
/// tao dong nao.
#[derive(Debug)]
pub struct RunHandle {
    pub run_id: String,
    pub token: CancellationToken,
    pub join: JoinHandle<()>,
    pub tasks_ready: watch::Receiver<Vec<TaskRef>>,
}

/// Mot dong `TaskRow` da duoc tao (VA gan `run_id`/`node_id`) cho MOT canh trong mot buoc Video —
/// xem [`RunHandle::tasks_ready`]. Khong can `camelCase`/`Serialize`: chi dung NOI BO trong
/// `lvc-engine` (giua `dag.rs` va `scripts.rs`), khong bao gio di qua IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRef {
    pub node_id: String,
    pub scene_index: u32,
    pub task_id: String,
}

/// Ket qua MOT job video da chay xong (`JoinSet` gom lai). `task_id` (fix-round 1, M3/I1): can lai
/// de ghi `finished_at` cho canh bi huy va de thong diep loi (I1) neu can trong tuong lai.
struct VideoJobResult {
    task_id: String,
    scene_index: u32,
    result: Result<GenerateOutcome, EngineError>,
}

/// Fix-round 2 (I3-NEW): mot canh da qua GIAI DOAN 1 cua `run_video_step` (hooks + token con da
/// san sang, KHONG panic) — cho GIAI DOAN 2 (tao dong `TaskRow` + spawn) doc tiep. Khong `Clone`:
/// `hooks` la `Box<dyn PollHooks>` di chuyen mot lan vao than vong spawn.
struct PreparedJob {
    scene_index: u32,
    scene: Scene,
    task_id: String,
    hooks: Box<dyn PollHooks>,
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4())
}

/// Fix-round 1 (M4): tong so "con" (don vi hoan tat) cua tung node theo `RunPlan` — dung de dien
/// `total` cho `NodeState` "queued" phat luc bat dau run (truoc day luon la `0`). Prompt/
/// `ImageUpload`/Save = 1 (mot buoc la xong, ke ca Save gom nhieu canh nguon — xem
/// `run_save_group`); Video = so canh (`jobs.len()`). Node khong xuat hien trong buoc nao (khong
/// nen xay ra sau khi `plan()` da hop le — xem doc `plan::plan`) khong co trong map; goi noi dung
/// `unwrap_or(1)`.
fn node_totals(run_plan: &RunPlan) -> HashMap<String, u32> {
    let mut totals = HashMap::new();
    for step in &run_plan.steps {
        match step {
            Step::Prompt { node_id, .. } | Step::ImageUpload { node_id, .. } => {
                totals.insert(node_id.clone(), 1);
            }
            Step::Video { node_id, jobs } => {
                totals.insert(
                    node_id.clone(),
                    u32::try_from(jobs.len()).unwrap_or(u32::MAX),
                );
            }
            Step::Save(job) => {
                totals.entry(job.node_id.clone()).or_insert(1);
            }
        }
    }
    totals
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
        // I1 (fix-round 1): kenh bao "dong TaskRow da tao xong" — xem doc `RunHandle::tasks_ready`.
        let (tasks_ready_tx, tasks_ready_rx) = watch::channel(Vec::<TaskRef>::new());

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
                    tasks_ready_tx,
                )
                .await;
        });

        Ok(RunHandle {
            run_id,
            token,
            join,
            tasks_ready: tasks_ready_rx,
        })
    }

    /// Huy MOT lan chay dang co (dong bo — xem doc module ve ly do). Tra `false` khi khong biet
    /// `run_id` (chua chay/da xong — `run_tokens` chi giu run DANG chay, xem `run_graph_task`).
    /// Fix-round 1 (C1): huy DUY NHAT token cua run la du — token cua tung canh la CON cua token
    /// nay (`run_video_step` goi `register_child` truoc khi spawn), nen `tokio-util` tu lan truyen
    /// xuong het, ke ca cac `tokio::select!` cho slot/lease (xem doc module).
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
    /// hoach ra ngoai, truoc khi spawn); moi loi o day chi lam RUN NAY that bai, khong panic. Fix
    /// round 1 (I3): than vong BUOC (co goi `hooks_factory` cua caller — co the panic) chay trong
    /// MOT `tokio::spawn` con rieng — `Err(JoinError)` (panic) duoc bat va coi la run that bai voi
    /// thong diep loi, thay vi lam `run_graph_task` panic theo (dong `runs` ket "running" mai mai,
    /// `run_tokens` ri rac, khong `RunDone` — xem doc module).
    #[allow(
        clippy::too_many_arguments,
        reason = "8 tham so mo ta MOT lan chay (id/workflow/node/plan/token/hooks/kenh bao) - than \
                  vong nen chay MOT LAN cho `run_graph`, tach nho se roi tham so qua nhieu ham con \
                  hon la lam ro hon"
    )]
    async fn run_graph_task(
        self: Arc<Self>,
        run_id: String,
        workflow_id: String,
        node_ids: Vec<String>,
        run_plan: RunPlan,
        token: CancellationToken,
        hooks_factory: HooksFactory,
        tasks_ready_tx: watch::Sender<Vec<TaskRef>>,
    ) {
        let started = now_unix();
        if let Err(e) = self
            .run_repo
            .create(&run_id, &workflow_id, RunStatus::Running, started)
            .await
        {
            warn!(run_id, error = %e, "khong tao duoc dong runs - dung run nay");
            self.run_tokens.remove(&run_id);
            // M2 (fix-round 1, parked theo ruling controller): chua co dong `runs` de cap nhat nen
            // KHONG phat `RunDone` o day — chap nhan hien trang, xem bao cao Task 7.
            // I1 (fix-round 1): `tasks_ready_tx` bi drop NGAY DAY (khong gui gi) — nguoi doi
            // (`RunHandle::tasks_ready::changed()`) nhan `Err` ngay (moi sender da mat), khong can
            // cho het han an toan (xem `run_script`).
            return;
        }

        let run_id_steps = run_id.clone();
        let token_steps = token.clone();
        let steps_fut = Arc::clone(&self).run_steps(
            run_id_steps,
            node_ids,
            run_plan,
            token_steps,
            hooks_factory,
            tasks_ready_tx,
        );
        let (error, failed) = match tokio::spawn(steps_fut).await {
            Ok(r) => r,
            Err(join_err) => {
                warn!(run_id, error = %join_err, "than vong chay buoc panic - ket thuc run la failed");
                (
                    Some(format!("loi he thong: than vong chay panic ({join_err})")),
                    true,
                )
            }
        };

        self.finish_run(run_id, &token, error, failed).await;
    }

    /// Duyet `RunPlan::steps` theo dung thu tu topo, tra `(thong diep loi de ghi vao runs.error,
    /// co that bai hay khong)` — KHONG tu ghi DB/phat `RunDone`, `run_graph_task` lam viec do sau
    /// khi than vong nay (co the panic — xem doc do) tra ve/bi bat.
    #[allow(
        clippy::too_many_arguments,
        reason = "7 tham so mo ta MOT lan chay (id/node/plan/token/hooks/kenh bao) - xem ly do o \
                  run_graph_task"
    )]
    async fn run_steps(
        self: Arc<Self>,
        run_id: String,
        node_ids: Vec<String>,
        run_plan: RunPlan,
        token: CancellationToken,
        hooks_factory: HooksFactory,
        tasks_ready_tx: watch::Sender<Vec<TaskRef>>,
    ) -> (Option<String>, bool) {
        let totals = node_totals(&run_plan);
        for node_id in &node_ids {
            let total = totals.get(node_id).copied().unwrap_or(1);
            self.emit_node_state(&run_id, node_id, NODE_QUEUED, (0, total, 0, 0));
        }

        let mut outputs = NodeOutputs::default();
        let mut touched: HashSet<String> = HashSet::new();
        let mut failures: Vec<String> = Vec::new();
        let mut hard_stop: Option<String> = None;

        let mut i = 0usize;
        while i < run_plan.steps.len() {
            // I2 (fix-round 1): huy giua chung -> dung NGAY, KHONG buoc/canh nao moi duoc tao nua.
            // Fix-round 2 (doc minor, review-r1): kiem tra nay dung TRUOC MOI buoc, ke ca mot buoc
            // Save nam NGAY SAU buoc Video vua bi huy — nen buoc Save do CUNG bi bo qua, du buoc
            // Video co the da co vai canh `Completed` truoc khi huy. Day la CO Y (ruling round-2),
            // KHONG phai loi: cac canh da `Completed` do VAN con file that trong `downloads_dir` va
            // duong dan van nam tren dong `task` cua chung — chi la KHONG duoc Save copy sang
            // `out_dir` cua node Luu trong lan chay nay.
            if hard_stop.is_some() || token.is_cancelled() {
                break;
            }
            match &run_plan.steps[i] {
                Step::Prompt { node_id, scenes } => {
                    touched.insert(node_id.clone());
                    self.emit_node_state(&run_id, node_id, NODE_RUNNING, (0, 1, 0, 0));
                    outputs.texts.insert(node_id.clone(), scenes.clone());
                    self.emit_node_state(&run_id, node_id, NODE_DONE, (1, 1, 1, 0));
                    i += 1;
                }
                Step::ImageUpload { node_id, paths } => {
                    touched.insert(node_id.clone());
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
                    touched.insert(node_id.clone());
                    if let Some(msg) = self
                        .run_video_step(
                            &run_id,
                            &node_id,
                            jobs,
                            &token,
                            &hooks_factory,
                            &mut outputs,
                            &tasks_ready_tx,
                        )
                        .await
                    {
                        failures.push(msg);
                    }
                    i += 1;
                }
                Step::Save(first) => {
                    let node_id = first.node_id.clone();
                    let (_, jobs, next_i) = collect_save_group(&run_plan, i);
                    i = next_i;
                    touched.insert(node_id.clone());
                    if let Some(msg) = self
                        .run_save_group(&run_id, &node_id, &jobs, &outputs)
                        .await
                    {
                        failures.push(msg);
                    }
                }
            }
        }

        // I2: node CHUA KIP CHAY toi (vong lap dung som vi huy/hard_stop) khong duoc bo mac "queued"
        // mai mai — phat MOT NodeState cuoi cho tung node do.
        if hard_stop.is_some() || token.is_cancelled() {
            let pending_state = if token.is_cancelled() {
                NODE_CANCELLED
            } else {
                NODE_FAILED
            };
            for node_id in &node_ids {
                if !touched.contains(node_id) {
                    let total = totals.get(node_id).copied().unwrap_or(1);
                    self.emit_node_state(&run_id, node_id, pending_state, (0, total, 0, total));
                }
            }
        }

        // I1 (fix-round 1, P3c Task 1): "chot" tasks_ready KHONG DIEU KIEN, du KHONG co buoc Video
        // nao trong lan chay nay (vd hard_stop/huy TRUOC khi toi buoc Video, hoac do thi khong co
        // node Video). `send_modify` LUON danh dau "changed" ke ca closure khong doi gi — nguoi doi
        // (`run_script`) nho vay khong bao gio cho vo thoi han (toi da het han an toan cua no) chi
        // vi run nay khong tung tao dong `TaskRow` nao. Neu `run_video_step` da gui truoc do (co
        // buoc Video), day chi la MOT lan "changed" thua — vo hai (nguoi doi chi doi `changed()`
        // MOT LAN, lay gia tri o lan dau tien no thuc day).
        tasks_ready_tx.send_modify(|_| {});

        // I1: uu tien thong diep hard_stop (ImageUpload thieu file — chi 1 nguyen nhan ro rang);
        // khong thi gop danh sach canh/Save that bai tu cac node (co the nhieu node cung loi).
        let error = hard_stop.or_else(|| (!failures.is_empty()).then(|| failures.join("; ")));
        let failed = error.is_some();
        (error, failed)
    }

    /// Duoc quyet dinh trang thai cuoi (huy > loi cung/canh > hoan tat), ghi `RunRepo::set_status`,
    /// go token khoi `run_tokens`, roi phat `RunDone` (SAU khi DB da ghi — ruling brief).
    async fn finish_run(
        &self,
        run_id: String,
        token: &CancellationToken,
        error: Option<String>,
        failed: bool,
    ) {
        let status = if token.is_cancelled() {
            RunStatus::Cancelled
        } else if failed {
            RunStatus::Failed
        } else {
            RunStatus::Completed
        };
        let finished = now_unix();
        if let Err(e) = self
            .run_repo
            .set_status(&run_id, status, error.as_deref(), finished)
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

    /// Fix-round 2 (M5, escalated): danh dau MOT `TaskRow` da ton tai la `Failed` + `finished_at`
    /// NGAY, khong spawn job cho no — dung khi mot buoc DB sau khi tao dong (hien tai: chi
    /// `TaskRepo::set_run`) that bai, de dong khong ket lai "queued" mai mai (vo hinh voi
    /// `TaskRepo::list_by_run`). `reason` la thong diep ngan, KHONG chua prompt/cookie/URL ky.
    async fn mark_row_failed(&self, task_id: &str, reason: &str) {
        if let Err(e) = self
            .tasks
            .set_status(task_id, TaskStatus::Failed, None, Some(reason))
            .await
        {
            warn!(task_id, error = %e, "khong ghi duoc trang thai failed");
        }
        if let Err(e) = self.tasks.set_finished(task_id, now_unix()).await {
            warn!(task_id, error = %e, "khong ghi duoc finished_at");
        }
    }

    /// Buoc `Video`: tao MOT `TaskRow` cho MOI canh TRUOC KHI spawn (de UI liet ke duoc ngay), roi
    /// `JoinSet` chay dong thoi (limiter/`AccountPool` lo song song — xem `generate.rs`); mot canh
    /// loi KHONG dung cac canh khac (ruling). Ghi `NodeOutputs.videos[node_id]` = cac canh THANH
    /// CONG (sap theo `scene_index`) — canh loi khong co mat, Save (buoc sau) tu dong chi luu canh
    /// OK. Tra `Some(thong diep)` (I1: `"node {node_id}: canh {idx,..} that bai"`) khi CO IT NHAT
    /// 1 canh loi, `None` khi tat ca OK.
    ///
    /// Fix-round 2 (I3-NEW): chia HAI GIAI DOAN — giai doan 1 goi `hooks_factory(&task_id)` (ma
    /// Task 9, CO THE panic) + `register_child` cho TAT CA canh, KHONG dung `TaskRepo`; giai doan 2
    /// (chi chay neu giai doan 1 xong het, khong panic) moi `TaskRepo::create`/`set_run`/spawn. Neu
    /// `hooks_factory` panic o giai doan 1: CHUA co dong `TaskRow` nao cho buoc nay ton tai (khong
    /// orphan `queued`), va panic van nam tren than vong dong bo cua `run_steps` (chua spawn job
    /// nao vao `JoinSet` — khong co job anh em nao bi huy giua chung) nen lan thang toi cai bay
    /// panic cua I3 o tang tren (`run_graph_task`).
    ///
    /// Fix-round 1 (P3c Task 1, I1): giai doan 2 tach THEM lam BA (2a/2b/2c) — 2a tao dong CHO TAT
    /// CA canh TRUOC (khong spawn gi ca); 2b phat toan bo `TaskRef` vua tao qua `tasks_ready_tx`
    /// (`send_modify`, xem doc [`RunHandle::tasks_ready`]) — nguoi doi biet DAY DU danh sach ngay
    /// khi hang DB da xong, KHONG phai luc mot phan con dang duoc tao (tranh dung sai "moi tao mot
    /// nua"); 2c moi spawn `JoinSet`. Truoc ban sua nay, tao dong + spawn xen ke tung canh mot vong
    /// duy nhat — khong co diem nao "toan bo hang da san sang" de bao ra ngoai.
    #[allow(
        clippy::too_many_arguments,
        reason = "8 tham so mo ta MOT buoc Video (id run/node/canh/token/hooks/dau ra/kenh bao) - \
                  xem ly do o run_graph_task"
    )]
    #[allow(
        clippy::too_many_lines,
        reason = "fix-round 1 (I1) them giai doan 2b (phat tasks_ready) giua 2a/2c da co san - than \
                  ham van la MOT buoc tuan tu (chuan bi hooks -> tao dong -> bao -> spawn -> gom \
                  ket qua), tach nho se roi PreparedJob/CreatedJob qua nhieu ham con"
    )]
    async fn run_video_step(
        self: &Arc<Self>,
        run_id: &str,
        node_id: &str,
        jobs: &[VideoJob],
        run_token: &CancellationToken,
        hooks_factory: &HooksFactory,
        outputs: &mut NodeOutputs,
        tasks_ready_tx: &watch::Sender<Vec<TaskRef>>,
    ) -> Option<String> {
        let total = u32::try_from(jobs.len()).unwrap_or(u32::MAX);
        self.emit_node_state(run_id, node_id, NODE_RUNNING, (0, total, 0, 0));

        let mut failed_indices: Vec<String> = Vec::new();

        // Giai doan 1 (I3-NEW): chuan bi hooks + token con cho TAT CA canh TRUOC — hooks_factory co
        // the panic (ma Task 9), o day CHUA dong `TaskRow` nao ca nen khong the orphan.
        let mut prepared: Vec<PreparedJob> = Vec::with_capacity(jobs.len());
        for job in jobs {
            // I2: huy TRUOC khi kip chuan bi canh nay -> khong can hooks/token cho no nua.
            if run_token.is_cancelled() {
                failed_indices.push(job.scene_index.to_string());
                continue;
            }
            let task_id = new_id("task");
            let hooks = hooks_factory(&task_id);
            // C1 (fix-round 1): dang ky token cua CANH NAY la CON cua token cua RUN, TRUOC KHI
            // spawn — xem doc module + `cancel::CancelRegistry::register_child`.
            let _ = self.cancels().register_child(&task_id, run_token);
            prepared.push(PreparedJob {
                scene_index: job.scene_index,
                scene: job.scene.clone(),
                task_id,
                hooks,
            });
        }

        // Giai doan 2a (I1): tao dong CHO TAT CA canh TRUOC, CHUA spawn gi ca.
        let mut created: Vec<PreparedJob> = Vec::with_capacity(prepared.len());
        for pj in prepared {
            let batch = BatchRef {
                batch_id: run_id.to_string(),
                batch_name: node_id.to_string(),
                scene_index: i64::from(pj.scene_index),
            };
            if let Err(e) = self
                .tasks
                .create(&pj.task_id, &pj.scene, Some(&batch))
                .await
            {
                warn!(run_id, node_id, error = %e, "khong tao duoc dong task cho canh");
                failed_indices.push(pj.scene_index.to_string());
                continue;
            }
            if let Err(e) = self.tasks.set_run(&pj.task_id, run_id, node_id).await {
                warn!(run_id, node_id, task_id = pj.task_id, error = %e, "khong gan duoc run_id/node_id");
                // M5 (fix-round 2, escalated tu Minor thanh Important — review-r1): dong da tao
                // nhung THIEU run_id/node_id se vo hinh voi `TaskRepo::list_by_run` MAI MAI neu
                // chay tiep — danh dau Failed NGAY, KHONG spawn, tinh la 1 canh that bai.
                self.mark_row_failed(&pj.task_id, &format!("store: set_run: {e}"))
                    .await;
                failed_indices.push(pj.scene_index.to_string());
                continue;
            }
            created.push(pj);
        }

        // Giai doan 2b (I1): phat toan bo TaskRef vua tao — TRUOC khi spawn (chua doi video tao
        // xong, chi doi hang DB) — xem doc `RunHandle::tasks_ready`.
        tasks_ready_tx.send_modify(|v| {
            v.extend(created.iter().map(|c| TaskRef {
                node_id: node_id.to_string(),
                scene_index: c.scene_index,
                task_id: c.task_id.clone(),
            }));
        });

        // Giai doan 2c: spawn — chi toi day moi bat dau tao video that.
        let mut set: JoinSet<VideoJobResult> = JoinSet::new();
        for pj in created {
            let PreparedJob {
                scene_index,
                scene,
                task_id,
                hooks,
            } = pj;
            let engine = Arc::clone(self);
            let tid = task_id.clone();
            set.spawn(async move {
                let result = engine
                    .generate_video(&tid, &scene, None, hooks.as_ref())
                    .await;
                VideoJobResult {
                    task_id: tid,
                    scene_index,
                    result,
                }
            });
        }

        let mut ok = 0u32;
        let mut artifacts: Vec<(u32, VideoArtifact)> = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(VideoJobResult {
                    scene_index,
                    result: Ok(out),
                    ..
                }) => {
                    ok += 1;
                    artifacts.push((scene_index, out.artifact));
                }
                Ok(VideoJobResult {
                    task_id,
                    scene_index,
                    result: Err(e),
                }) => {
                    failed_indices.push(scene_index.to_string());
                    // M3 (fix-round 1, pham vi da gioi han theo ruling controller): canh bi huy MA
                    // CHUA BAO GIO thuc su chay (bat o cho slot/lease/doi tai khoan ban) van can
                    // `finished_at` — `generate_video` khong tu ghi cot nay o bat ky duong nao
                    // (khoang trong co san, xem review M3); o day chi sua rieng nhom canh do.
                    if e.failure_code() == Some(FailureCode::Cancelled) {
                        if let Err(se) = self.tasks.set_finished(&task_id, now_unix()).await {
                            warn!(run_id, node_id, task_id, error = %se, "khong ghi duoc finished_at");
                        }
                    }
                }
                Err(join_err) => {
                    warn!(run_id, node_id, error = %join_err, "job video ket thuc bat thuong (panic)");
                    failed_indices.push("panic".to_string());
                }
            }
        }
        artifacts.sort_by_key(|(idx, _)| *idx);
        outputs.videos.insert(node_id.to_string(), artifacts);

        let failed = u32::try_from(failed_indices.len()).unwrap_or(u32::MAX);
        let state = if failed > 0 { NODE_FAILED } else { NODE_DONE };
        self.emit_node_state(run_id, node_id, state, (ok + failed, total, ok, failed));

        (!failed_indices.is_empty())
            .then(|| format!("node {node_id}: canh {} that bai", failed_indices.join(",")))
    }

    /// Buoc `Save`: gom MOI `Step::Save` LIEN TIEP cung `node_id` (do `lvc_core::graph::plan` sinh
    /// chung mot cho khi tham node Luu do — xem doc `plan::plan`) thanh MOT `NodeState`. Nguon la
    /// video -> copy (KHONG di chuyen) tung file vao `out_dir`; nguon la anh -> no-op (P4, xem doc
    /// module); nguon khong co dau ra (buoc truoc bi bo qua/loi ha tang) -> coi la loi. Tra
    /// `Some("{node_id}: luu loi")` (I1) khi CO IT NHAT 1 job loi, `None` khi tat ca OK.
    async fn run_save_group(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: &[SaveJob],
        outputs: &NodeOutputs,
    ) -> Option<String> {
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
        (!ok).then(|| format!("{node_id}: luu loi"))
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
    use std::time::{Duration, Instant};

    use lvc_browser::dola::{ConversationId, poll::NOTE_RAW};
    use lvc_browser::{DolaError, FakeCall, FakeDriver, FakeStep};
    use lvc_core::{
        Edge, FailureCode, Limits, LogoMode, Mode, Model, Node, NodeKind, Ratio, RawScene,
        TaskStatus,
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
        build_engine_with(accounts, threads, steps, |_cfg| {}).await
    }

    /// Nhu [`build_engine`] nhung cho tuy chinh `EngineConfig` (fix-round 1: C1 test 2 can
    /// `busy_wait_max` ngan de kiem tra "dung ngay" khong lien quan gi den han cho tai khoan ban).
    async fn build_engine_with(
        accounts: &[&str],
        threads: usize,
        steps: Vec<FakeStep>,
        tune: impl FnOnce(&mut EngineConfig),
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
        tune(&mut cfg);
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
        video_kind_for("")
    }

    /// Nhu [`video_kind`] nhung chi dinh `account` (fix-round 1, C1 test 2: node "chi dinh" mot
    /// tai khoan cu the -> `Scene::account` -> `prefer` -> `run_mode` vao nhanh `lease_wait`).
    fn video_kind_for(account: &str) -> NodeKind {
        NodeKind::Video {
            mode: Mode::Fast,
            model: Model::Seedance20,
            ratio: Ratio::R9x16,
            duration: 10,
            logo: LogoMode::Off,
            account: account.to_string(),
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

    /// Doc su kien toi khi gap `TaskStage{stage, ..}` khop dung `stage` (fix-round 1, I4): dong bo
    /// theo su kien thay vi `sleep` co dinh — het flaky "duoi tai co the la 0/2/3 loi goi driver".
    async fn wait_for_stage(rx: &mut broadcast::Receiver<EngineEvent>, stage: &str) {
        loop {
            let ev = next_event(rx).await.expect("kenh su kien con mo");
            if let EngineEvent::TaskStage { stage: s, .. } = &ev {
                if s == stage {
                    return;
                }
            }
        }
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

    // ---- RED evidence (fix-round 1, C1): kich ban nay TREO voi co che huy cu (`RunCancelHooks`
    // chi OR vao cac diem kiem DONG BO — hai `tokio::select!` cho slot limiter/`lease_wait` chi
    // xem token cua `CancelRegistry`, khong bao gio thay token cua RUN) — `timeout(2s)` bat panic
    // ro rang thay vi treo ca test suite. Giu comment nay lai lam bang chung sau khi sua (test se
    // xanh); xem task-7-report.md muc "Fix round 1" de biet dau ra chay TRUOC khi sua.
    #[tokio::test]
    async fn huy_khi_canh_dang_cho_slot_limiter_ket_thuc_ngay() {
        let (dir, engine, db, driver) = build_engine(&["acc1", "acc2"], 1, vec![]).await;
        // Giu DUY NHAT permit cua limiter (threads=1) TU BEN NGOAI run — ca 2 canh cua run se
        // block vo thoi han trong `limiter.acquire()` cho toi khi `_permit` duoc tra.
        let _permit = engine.limiter().acquire().await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1\ncanh 2", &out_dir);

        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(engine.cancel_run(&run_id));

        let t0 = Instant::now();
        tokio::time::timeout(Duration::from_secs(2), handle.join)
            .await
            .expect("run phai ket thuc trong 2s (khong duoc treo cho permit ben ngoai)")
            .unwrap();
        assert!(
            t0.elapsed() < Duration::from_secs(1),
            "phai dung gan nhu ngay: {:?}",
            t0.elapsed()
        );

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Cancelled);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert_eq!(rows.len(), 2);
        for r in &rows {
            assert_eq!(r.status, TaskStatus::Failed, "{r:?}");
            assert_eq!(r.failure_code, Some(FailureCode::Cancelled), "{r:?}");
        }
        assert!(
            driver.calls.lock().unwrap().is_empty(),
            "khong canh nao kip cham driver (con bi chan o limiter)"
        );
    }

    /// C1 test 2 (fix-round 1, demanded by review): canh CHI DINH mot tai khoan ("acc1") dang bi
    /// mot lease NGOAI RUN giu -> `run_mode` vao nhanh `lease_wait` (prefer) va bi chan o do (KHONG
    /// lien quan `busy_wait_max` — dat ngan 5s chi de lo neu sua sai lai roi ve cho het gio thay vi
    /// dung ngay). Truoc fix C1 kich ban nay CUNG treo (ly do y het test 1: `select!` cua
    /// `lease_wait` chi bam token registry, khong bam token run).
    #[tokio::test]
    async fn huy_khi_canh_dang_cho_lease_tai_khoan_chi_dinh_ket_thuc_ngay() {
        let (dir, engine, db, driver) = build_engine_with(&["acc1", "acc2"], 1, vec![], |cfg| {
            cfg.busy_wait_max = Duration::from_secs(5);
        })
        .await;
        // Giu khoa "acc1" TU BEN NGOAI run — canh chi dinh account="acc1" se cho vo thoi han
        // (trong pham vi busy_wait_max) tai `lease_wait`.
        let _lease = engine.pool().try_lease("acc1", "").unwrap();

        let out_dir = dir.path().join("out");
        let graph = Graph {
            version: 1,
            nodes: vec![
                node(
                    "pr",
                    NodeKind::Prompt {
                        text: "canh 1".to_string(),
                    },
                ),
                node("vid", video_kind_for("acc1")),
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
        };

        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(engine.cancel_run(&run_id));

        let t0 = Instant::now();
        tokio::time::timeout(Duration::from_secs(2), handle.join)
            .await
            .expect("run phai ket thuc trong 2s (khong duoc treo cho lease_wait)")
            .unwrap();
        assert!(
            t0.elapsed() < Duration::from_secs(1),
            "phai dung gan nhu ngay, khong doi busy_wait_max: {:?}",
            t0.elapsed()
        );

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Cancelled);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, TaskStatus::Failed, "{:?}", rows[0]);
        assert_eq!(
            rows[0].failure_code,
            Some(FailureCode::Cancelled),
            "{:?}",
            rows[0]
        );
        assert!(
            driver.calls.lock().unwrap().is_empty(),
            "khong duoc mo Chrome (IDEA-016)"
        );
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
            // M4 (fix-round 1): "queued" mang dung total tu RunPlan (pr=1 canh Prompt, vid=3 canh
            // video, sv=1 "buoc Luu") thay vi luon 0 nhu truoc.
            node_state(&run_id, "pr", NODE_QUEUED, 0, 1, 0, 0),
            node_state(&run_id, "vid", NODE_QUEUED, 0, 3, 0, 0),
            node_state(&run_id, "sv", NODE_QUEUED, 0, 1, 0, 0),
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

    /// I1 (fix-round 1, P3c Task 1, MUST): `tasks_ready` phai thuc day VOI DU 3 `TaskRef` ma KHONG
    /// can doi ca lan chay xong (`handle.join`) — day CHINH LA co che thay the cho poll ~2s cua
    /// `lvc_engine::scripts::run_script`. Video con dang "tao" (`FakeDriver` chua tra loi) luc
    /// `changed()` thuc day, chung minh viec bao xay ra o buoc TAO DONG DB, khong phai luc video
    /// xong.
    #[tokio::test]
    async fn tasks_ready_phat_du_3_task_ref_truoc_khi_video_tao_xong() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4-bytes").unwrap();

        let (dir, engine, db, _driver) =
            build_engine(&["acc1", "acc2", "acc3"], 1, happy_steps(3, &src)).await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1\ncanh 2\ncanh 3", &out_dir);

        let mut handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();

        handle.tasks_ready.changed().await.unwrap();
        let refs = handle.tasks_ready.borrow_and_update().clone();
        assert_eq!(refs.len(), 3, "{refs:?}");
        let mut scene_indexes: Vec<u32> = refs.iter().map(|r| r.scene_index).collect();
        scene_indexes.sort_unstable();
        assert_eq!(scene_indexes, vec![1, 2, 3]);
        for r in &refs {
            assert_eq!(r.node_id, "vid");
            assert!(!r.task_id.is_empty());
        }

        // Doi chieu voi dong TaskRow that su — id/scene_index khop dung, du video co the CHUA xong.
        let tasks = TaskRepo::new(&db);
        for r in &refs {
            let row = tasks.get(&r.task_id).await.unwrap().unwrap();
            assert_eq!(row.scene_index, Some(i64::from(r.scene_index)));
            assert_eq!(row.run_id, handle.run_id);
        }

        handle.join.await.unwrap();
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
        // I1 (fix-round 1): loi run phai neu ten node + canh that bai — truoc day la chuoi rong.
        assert!(
            run_row.error.contains("vid"),
            "phai neu ten node 'vid': {:?}",
            run_row.error
        );
        assert!(run_row.error.contains("that bai"), "{:?}", run_row.error);

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
        // I1: dung 1 chi so canh duoc neu ten trong loi run.
        let failed_idx = failed_rows[0].scene_index.unwrap().to_string();
        assert!(
            run_row.error.contains(&failed_idx),
            "loi run phai neu dung chi so canh that bai {failed_idx}: {:?}",
            run_row.error
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

        let mut rx = engine.events().subscribe();
        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();

        // I4 (fix-round 1): dong bo theo su kien thay vi `sleep` co dinh — doi DUNG luc job dau
        // tien vao `PollUntilCancelled` (FakeDriver phat `TaskStage{stage:"generating"}` ngay khi
        // vao vong lap do — `fake.rs` doc module). Truoc day mot `sleep(100ms)` co dinh co the
        // roi vao khe ho GIUA `check("truoc launch")` va `should_cancel()` dau tien trong
        // `submit()` (chi co Launch+Submit da xay ra, chua Poll) -> `calls_len == 2`, ngoai tap
        // `{0, 3}` ma test cu gia dinh — flake tiem an duoc I4 vach ra.
        wait_for_stage(&mut rx, "generating").await;
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
            // M3 (fix-round 1): canh bi huy (ca canh CHUA BAO GIO thuc su chay) cung duoc ghi
            // finished_at, khong con NULL vinh vien.
            assert!(r.finished_at.is_some(), "{r:?}");
        }
        assert!(
            !out_dir.exists() || out_dir.read_dir().unwrap().next().is_none(),
            "khong file nao duoc luu (khong canh nao thanh cong)"
        );

        // I4 (fix-round 1): dong bo theo su kien -> LUON dung 1 job cham driver (3 loi goi:
        // Launch+Submit+Poll); 2 job con lai bi huy TRUOC khi mo Chrome (IDEA-016) vi token cua
        // chung la CON cua token run (C1) nen `tokio::select!` cho slot thay huy NGAY.
        assert_eq!(
            driver.calls.lock().unwrap().len(),
            3,
            "dong bo theo su kien 'generating' -> luon dung 3 loi goi (Launch+Submit+Poll)"
        );

        // run da ket thuc -> khong con trong run_tokens -> cancel lan nua tra false.
        assert!(!engine.cancel_run(&run_id));
        assert!(!engine.cancel_run("khong-ton-tai"));
    }

    /// I2 (fix-round 1, demanded by review): do thi CO 2 node Tao video doc lap (prA->vidA->svA,
    /// prB->vidB->svB, khai bao THEO DUNG THU TU do de topo (Kahn's, xem `validate::topo_order`,
    /// luon chon chi so THAP NHAT trong tap san sang) chac chan xu ly HET nhanh A truoc khi cham
    /// toi nhanh B). Huy giua luc vidA dang poll: vidB KHONG duoc tao task nao (vong lap dung o
    /// dau lan lap tiep theo, TRUOC ca svA — xem doc `run_steps`/I2), `NodeState` cuoi cua vidB la
    /// [`NODE_CANCELLED`] thay vi bo mac "queued" mai mai.
    #[tokio::test]
    async fn huy_giua_video_dau_video_thu_hai_khong_tao_task_nao() {
        let steps = vec![
            FakeStep::Launch(Ok(())),
            FakeStep::Submit(Ok(ConversationId("c1".into()))),
            FakeStep::PollUntilCancelled,
        ];
        let (dir, engine, db, driver) = build_engine(&["acc1", "acc2", "acc3"], 1, steps).await;
        let out_dir = dir.path().join("out");
        let graph = Graph {
            version: 1,
            nodes: vec![
                node(
                    "prA",
                    NodeKind::Prompt {
                        text: "canh A".to_string(),
                    },
                ),
                node("vidA", video_kind()),
                node(
                    "svA",
                    NodeKind::Save {
                        out_dir: out_dir.join("a").display().to_string(),
                    },
                ),
                node(
                    "prB",
                    NodeKind::Prompt {
                        text: "canh B".to_string(),
                    },
                ),
                node("vidB", video_kind()),
                node(
                    "svB",
                    NodeKind::Save {
                        out_dir: out_dir.join("b").display().to_string(),
                    },
                ),
            ],
            edges: vec![
                edge("e1", "prA", "out", "vidA", "prompt"),
                edge("e2", "vidA", "out", "svA", "video"),
                edge("e3", "prB", "out", "vidB", "prompt"),
                edge("e4", "vidB", "out", "svB", "video"),
            ],
        };

        let mut rx = engine.events().subscribe();
        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();

        wait_for_stage(&mut rx, "generating").await;
        assert!(engine.cancel_run(&run_id));

        tokio::time::timeout(Duration::from_secs(2), handle.join)
            .await
            .expect("khong duoc treo")
            .unwrap();

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Cancelled);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert_eq!(
            rows.len(),
            1,
            "chi vidA tao dong task cho canh cua no; vidB KHONG duoc tao: {rows:?}"
        );
        assert_eq!(rows[0].node_id, "vidA");

        assert_eq!(
            driver.calls.lock().unwrap().len(),
            3,
            "chi vidA (dang chay khi huy) cham driver"
        );

        // NodeState cuoi cung cho vidB phai la "cancelled" — khong bo mac "queued" mai mai (I2).
        let events = drain_until_run_done(&mut rx).await;
        let vidb_terminal = events.iter().rev().find_map(|e| match e {
            EngineEvent::NodeState { node_id, state, .. } if node_id == "vidB" => {
                Some(state.clone())
            }
            _ => None,
        });
        assert_eq!(vidb_terminal.as_deref(), Some(NODE_CANCELLED), "{events:?}");
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

        let mut handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();

        // I1 (fix-round 1): hard_stop xay ra o buoc ImageUpload, TRUOC khi toi buoc Video nao ca —
        // `run_video_step` khong bao gio duoc goi nen KHONG BAO GIO tu no gui `tasks_ready`. Van
        // phai thuc day (rong) nho lan "chot" khong dieu kien o cuoi `run_steps` (xem doc
        // `RunHandle::tasks_ready`) — nguoi doi (`run_script`) khong bao gio bi treo cho.
        handle.tasks_ready.changed().await.unwrap();
        assert!(handle.tasks_ready.borrow_and_update().is_empty());

        handle.join.await.unwrap();

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Failed);

        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert!(rows.is_empty(), "khong task video nao duoc tao: {rows:?}");
        assert!(driver.calls.lock().unwrap().is_empty());
    }

    // ---- I3 (fix-round 1, demanded by review): panic trong than vong buoc khong lam mat run ----

    /// `hooks_factory` cua Task 9 (ma nguoi dung/UI) panic — tinh huong thuc te duy nhat co the lam
    /// `run_steps` panic (xem doc module). Truoc fix I3, panic nay se lam `run_graph_task` panic
    /// theo (bi `tokio::spawn` cua `run_graph` nuot lang le): dong `runs` ket "running" mai mai,
    /// `run_tokens` ri rac (`cancel_run` tra `true` vinh vien cho mot run da "chet"), khong bao gio
    /// co `RunDone`. Sau fix: than vong con (`tokio::spawn(steps_fut)`) panic, `run_graph_task` bat
    /// duoc qua `Err(JoinError)`, coi la run that bai voi thong diep loi ro rang.
    ///
    /// Fix-round 2 (I3-NEW, mo rong theo yeu cau review-r1): do thi 3 canh — `hooks_factory` panic
    /// NGAY LAN GOI DAU TIEN (canh 1) nen giai doan 1 cua `run_video_step` dung lai TRUOC KHI bat
    /// ky `TaskRepo::create` nao chay (ca cho canh 1 lan 2 canh con lai) -> KHONG dong task nao
    /// duoc tao, KHONG job anh em nao bi spawn roi bi huy giua chung.
    #[tokio::test]
    async fn than_vong_buoc_panic_run_ket_thuc_la_failed_khong_treo() {
        let (dir, engine, db, driver) = build_engine(&["acc1"], 1, vec![]).await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1\ncanh 2\ncanh 3", &out_dir);

        let panicking: HooksFactory = Arc::new(|_task_id: &str| -> Box<dyn PollHooks> {
            panic!("hooks_factory co y panic (I3 test)")
        });

        let mut rx = engine.events().subscribe();
        let handle = engine.run_graph("wf1", &graph, panicking).await.unwrap();
        let run_id = handle.run_id.clone();

        tokio::time::timeout(Duration::from_secs(2), handle.join)
            .await
            .expect("khong duoc treo du than vong buoc panic")
            .unwrap();

        let run_row = engine.run_repo.get(&run_id).await.unwrap().unwrap();
        assert_eq!(run_row.status, RunStatus::Failed, "{run_row:?}");
        assert!(
            run_row.error.contains("panic"),
            "loi run phai neu ro la panic: {:?}",
            run_row.error
        );

        // I3-NEW: hooks_factory panic TRUOC bat ky TaskRepo::create nao (giai doan 1) -> khong
        // dong task nao duoc tao cho buoc Video nay, du do thi co 3 canh.
        let tasks = TaskRepo::new(&db);
        let rows = tasks.list_by_run(&run_id).await.unwrap();
        assert!(rows.is_empty(), "khong dong task nao duoc tao: {rows:?}");

        let events = drain_until_run_done(&mut rx).await;
        let last = events.last().expect("phai co it nhat RunDone");
        assert!(
            matches!(last, EngineEvent::RunDone { status, .. } if status == "failed"),
            "RunDone van phai duoc phat du panic giua chung: {events:?}"
        );

        // run da ket thuc (finish_run da chay, go token) -> cancel_run tra false, khong "song"
        // vinh vien nhu truoc fix.
        assert!(!engine.cancel_run(&run_id));
        assert_eq!(
            driver.calls.lock().unwrap().len(),
            0,
            "panic o giai doan 1 -> khong job nao duoc spawn -> driver chua bi dung toi"
        );
    }

    /// Fix-round 2 (M5, escalated): unit-test cho `Engine::mark_row_failed` — dong `TaskRow` da co
    /// san (`TaskRepo::create` truoc, mo phong `TaskRepo::set_run` da loi) phai chuyen dung sang
    /// `Failed` + dung `error` + co `finished_at`, KHONG con "queued".
    #[tokio::test]
    async fn mark_row_failed_ghi_dung_trang_thai_loi_va_finished_at() {
        let (_dir, engine, db, _driver) = build_engine(&[], 1, vec![]).await;
        let tasks = TaskRepo::new(&db);
        let scene = Scene::normalize(
            &RawScene {
                prompt: Some("canh mau".into()),
                ..RawScene::default()
            },
            &Limits::default(),
            None,
        )
        .unwrap();
        tasks.create("t1", &scene, None).await.unwrap();
        let before = tasks.get("t1").await.unwrap().unwrap();
        assert_eq!(before.status, TaskStatus::Queued, "{before:?}");
        assert!(before.finished_at.is_none());

        engine
            .mark_row_failed("t1", "store: set_run: loi gia lap")
            .await;

        let after = tasks.get("t1").await.unwrap().unwrap();
        assert_eq!(after.status, TaskStatus::Failed, "{after:?}");
        assert_eq!(
            after.error.as_deref(),
            Some("store: set_run: loi gia lap"),
            "{after:?}"
        );
        assert!(after.finished_at.is_some(), "{after:?}");
    }

    // ---- cac test nho bo tro ----

    #[tokio::test]
    async fn cancel_run_khong_biet_id_tra_false() {
        let (_dir, engine, _db, _driver) = build_engine(&[], 1, vec![]).await;
        assert!(!engine.cancel_run("khong-ton-tai"));
    }
}
