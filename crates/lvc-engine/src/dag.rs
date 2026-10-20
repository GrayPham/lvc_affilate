//! Bo thuc thi do thi. P4a (ADR-R014): engine GENERIC — duyet topo, goi `NodeExecutor::run` qua
//! `lvc_node_api::Registry`, KHONG `match` theo kind; node `dola_video` goi lai engine qua dich vu
//! `lvc_dola::DolaVideoHost` (= `run_video_jobs`, logic P3 duoi day giu nguyen). Lich su P3:
//!
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
use std::sync::Arc;

use async_trait::async_trait;
use lvc_browser::BrowserDriver;
use lvc_browser::dola::{NoopHooks, PollHooks};
use lvc_core::graph::{Graph, migrate};
use lvc_core::{FailureCode, Scene, TaskStatus};
use lvc_dola::{DolaVideoHost, JobsReport, SceneJob, VideoDone};
use lvc_genai::TextGenHost;
use lvc_google::{GoogleImageHost, GoogleVideoHost};
use lvc_grok::GrokImageHost;
use lvc_museai::{MuseaiImageHost, MuseaiVideoHost};
use lvc_node_api::{NodeCtx, Outputs, Prepared, Registry, Services, Value, gather_inputs};
use lvc_scripts::ScriptSinkHost;
use lvc_seedance::{SeedanceImageHost, SeedanceVideoHost};
use lvc_store::{BatchRef, RunStatus};
use lvc_tts::TtsHost;
use lvc_veo3::flow::Flow;
use lvc_veo3::{VeoCaptchaHost, VeoImageHost, VeoSessionHost, VeoVideoHost};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::apimedia::{
    EngineGoogleImage, EngineGoogleVideo, EngineGrokImage, EngineSeedanceImage,
    EngineSeedanceVideo, EngineTts,
};
use crate::error::EngineError;
use crate::events::EngineEvent;
use crate::generate::{Engine, GenerateOutcome};
use crate::now_unix;
use crate::scriptsink::EngineScriptSinkHost;
use crate::textgen::EngineTextGenHost;

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
/// `+ Send + Sync` — `PollHooks: Send + Sync` da la supertrait.
pub type HooksFactory = Arc<dyn Fn(&str) -> Box<dyn PollHooks> + Send + Sync>;

/// Factory mac dinh: hooks khong lam gi (dung cho test/CLI khong can theo doi tien do tung job).
#[must_use]
pub fn noop_hooks_factory() -> HooksFactory {
    Arc::new(|_task_id: &str| Box::new(NoopHooks) as Box<dyn PollHooks>)
}

/// Tay cam mot lan chay (`Engine::run_graph` tra ve NGAY, khong doi chay xong). `join` de test/
/// Task 9 doi run ket thuc; `token` de tu huy KHONG qua `Engine::cancel_run` (vd. dong app — huy
/// moi run dang co).
///
/// `tasks_ready` (fix-round 1, P3c Task 1, I1): kenh `watch` bao "dong `TaskRow` cua (cac) node
/// video Dola da tao xong" — gia tri la `Vec<TaskRef>` TICH LUY qua nhieu node video trong CUNG
/// mot lan chay, RONG luc dau. Gui NGAY SAU khi dong DB da tao xong cho MOT node video, TRUOC KHI
/// spawn job tao video that; VA mot lan "chot" khong dieu kien o cuoi `run_steps` de nguoi doi
/// (`scripts::run_script`) khong bao gio cho vo thoi han neu run KHONG BAO GIO tao dong nao.
#[derive(Debug)]
pub struct RunHandle {
    pub run_id: String,
    pub token: CancellationToken,
    pub join: JoinHandle<()>,
    pub tasks_ready: watch::Receiver<Vec<TaskRef>>,
}

/// Mot dong `TaskRow` da duoc tao (VA gan `run_id`/`node_id`) cho MOT canh — xem
/// [`RunHandle::tasks_ready`]. Chi dung NOI BO `lvc-engine`, khong di qua IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRef {
    pub node_id: String,
    pub scene_index: u32,
    pub task_id: String,
}

/// Ket qua MOT job video da chay xong (`JoinSet` gom lai).
struct VideoJobResult {
    task_id: String,
    scene_index: u32,
    result: Result<GenerateOutcome, EngineError>,
}

/// Fix-round 2 (I3-NEW): mot canh da qua GIAI DOAN 1 cua `run_video_jobs` (hooks + token con da
/// san sang, KHONG panic) — cho GIAI DOAN 2 (tao dong `TaskRow` + spawn) doc tiep.
struct PreparedJob {
    scene_index: u32,
    scene: Scene,
    task_id: String,
    hooks: Box<dyn PollHooks>,
}

/// Moi thu mot lan chay can (gom lai tranh ham qua nhieu tham so).
struct RunJob {
    run_id: String,
    workflow_id: String,
    graph: Graph,
    prepared: Prepared,
    registry: Arc<Registry>,
    token: CancellationToken,
    hooks_factory: HooksFactory,
    tasks_ready_tx: watch::Sender<Vec<TaskRef>>,
    /// Phần C (chạy lại node lỗi): output SEED sẵn cho các node đã xong (không chạy lại) — nạp vào
    /// `outputs` TRƯỚC vòng lặp để node phía sau `gather_inputs` thấy. Run thường = rỗng.
    seed_outputs: HashMap<String, Outputs>,
    /// Phần C: node BỎ QUA thực thi (đã xong, dùng `seed_outputs`) — emit `done` ngay, không gọi
    /// `exec` (không tốn credit). Run thường = rỗng.
    skip: HashSet<String>,
}

/// P4a (ADR-R014): dich vu `DolaVideoHost` cho MOT lan chay — node `dola_video` goi qua
/// `NodeCtx::services`; ben trong la NGUYEN `run_video_jobs` (logic P3 da qua 2 vong review).
struct RunDolaHost<D: BrowserDriver + 'static> {
    engine: Arc<Engine<D>>,
    hooks_factory: HooksFactory,
    tasks_ready_tx: watch::Sender<Vec<TaskRef>>,
}

#[async_trait]
impl<D: BrowserDriver + 'static> DolaVideoHost for RunDolaHost<D> {
    async fn run_jobs(
        &self,
        run_id: &str,
        node_id: &str,
        jobs: Vec<SceneJob>,
        cancel: &CancellationToken,
    ) -> JobsReport {
        self.engine
            .run_video_jobs(
                run_id,
                node_id,
                jobs,
                cancel,
                &self.hooks_factory,
                &self.tasks_ready_tx,
            )
            .await
    }
}

/// P5 Task 9: dich vu `VeoCaptchaHost` cho MOT lan chay — node `veo3_image`/`veo3_video` goi qua
/// `NodeCtx::services`. Chi la lop MONG chuyen tiep vao `engine.veo3_captcha` (khac `RunDolaHost` ở
/// tren không chỉ vì kiểu chữ ký — trạng thái trình duyệt khách THẬT sống trên `Engine`, DÀI HẠN
/// xuyên nhiều lần chạy, không phải tạo mới mỗi run; xem [`crate::veo3`]).
struct RunVeoCaptchaHost<D: BrowserDriver + 'static> {
    engine: Arc<Engine<D>>,
}

#[async_trait]
impl<D: BrowserDriver + 'static> VeoCaptchaHost for RunVeoCaptchaHost<D> {
    async fn mint_token(&self, action: &str) -> Result<String, lvc_veo3::error::VeoError> {
        self.engine.veo3_captcha.mint_token(action).await
    }
}

/// P5 Task 9b: dich vu `VeoSessionHost` cho MOT lan chay — lop MONG chuyen tiep vao
/// `engine.veo3_session` (giong `RunVeoCaptchaHost` o tren — trang thai cache theo `credential_id`
/// song tren `Engine`, DAI HAN xuyen nhieu lan chay, xem `crate::veo3::EngineVeoSession`).
struct RunVeoSessionHost<D: BrowserDriver + 'static> {
    engine: Arc<Engine<D>>,
}

#[async_trait]
impl<D: BrowserDriver + 'static> VeoSessionHost for RunVeoSessionHost<D> {
    async fn harvest(
        &self,
        credential_id: &str,
    ) -> Result<lvc_veo3::FlowSession, lvc_veo3::error::VeoError> {
        self.engine.veo3_session.harvest(credential_id).await
    }
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4())
}

/// Run-robustness B (THUAN, de test rieng): ten file `name` co chua TOKEN `{run_id}_{node_id}` voi
/// RANH GIOI dung khong? Ranh gioi truoc token: dau chuoi HOAC ngay sau `_` (sau tag `concat_`/
/// `trim_`/...). Ranh gioi sau token: `_` (truoc `{scene}`) HOAC `.` (truoc duoi file). Nho ranh
/// gioi nay, `concat_{run}_{node}.mp4` va `{run}_{node}_0.png` deu khop, nhung `{run}_{node}abc`
/// (`node_id` khac dai hon) thi khong.
fn name_matches_token(name: &str, token: &str) -> bool {
    let mut from = 0;
    while let Some(rel) = name[from..].find(token) {
        let start = from + rel;
        let end = start + token.len();
        let before_ok = start == 0 || name.as_bytes()[start - 1] == b'_';
        let after_ok = matches!(name.as_bytes().get(end), Some(b'_' | b'.'));
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

impl<D: BrowserDriver + 'static> Engine<D> {
    /// Chay MOT lan workflow. Do thi duoc migrate ve dinh dang hien tai, roi `Registry::prepare`
    /// (validate + preview tung node) — loi tra `Err` NGAY, KHONG tao dong `runs`. Tra
    /// [`RunHandle`] ngay lap tuc; chay nen qua `tokio::spawn`.
    ///
    /// # Errors
    /// `EngineError::Graph` khi do thi khong hop le / loi tham so node.
    #[allow(
        clippy::unused_async,
        reason = "giu async de khop chu ky cong khai (Tauri command async goi ham nay)"
    )]
    pub async fn run_graph(
        self: &Arc<Self>,
        workflow_id: &str,
        graph: &Graph,
        hooks_factory: HooksFactory,
    ) -> Result<RunHandle, EngineError> {
        self.run_graph_seeded(
            workflow_id,
            graph,
            hooks_factory,
            HashMap::new(),
            HashSet::new(),
        )
        .await
    }

    /// Phần C (chạy lại node lỗi): như [`Engine::run_graph`] nhưng SEED sẵn output cho các node đã
    /// xong (`seed_outputs`) và BỎ QUA thực thi chúng (`skip`) — chỉ node lỗi + node phía sau chạy
    /// lại, tái dùng file cũ (không tốn credit). Run thường gọi với hai map rỗng.
    ///
    /// # Errors
    /// `EngineError::Graph` khi đồ thị không hợp lệ / lỗi tham số node.
    #[allow(
        clippy::unused_async,
        reason = "giu async de khop chu ky cong khai (Tauri command async goi ham nay)"
    )]
    pub async fn run_graph_seeded(
        self: &Arc<Self>,
        workflow_id: &str,
        graph: &Graph,
        hooks_factory: HooksFactory,
        seed_outputs: HashMap<String, Outputs>,
        skip: HashSet<String>,
    ) -> Result<RunHandle, EngineError> {
        let graph = migrate(graph.clone());
        let registry = self.registry();
        let prepared = registry.prepare(&graph, &self.limits)?;
        let run_id = new_id("run");
        let token = CancellationToken::new();
        self.run_tokens.insert(run_id.clone(), token.clone());
        let (tasks_ready_tx, tasks_ready_rx) = watch::channel(Vec::<TaskRef>::new());

        let job = RunJob {
            run_id: run_id.clone(),
            workflow_id: workflow_id.to_string(),
            graph,
            prepared,
            registry,
            token: token.clone(),
            hooks_factory,
            tasks_ready_tx,
            seed_outputs,
            skip,
        };
        let engine = Arc::clone(self);
        let join = tokio::spawn(async move { engine.run_graph_task(job).await });

        Ok(RunHandle {
            run_id,
            token,
            join,
            tasks_ready: tasks_ready_rx,
        })
    }

    /// Huy MOT lan chay dang co. Tra `false` khi khong biet `run_id` (chua chay/da xong). Token cua
    /// tung canh la CON cua token run (`register_child`) nen huy lan truyen xuong het.
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

    /// `counts = (done, total, ok, failed)`; `error` = thong diep loi cua node (chi khi
    /// `state == NODE_FAILED`, dung cho nhat ky debug tung node) — [`emit_node_state`] cho cac buoc
    /// khong loi.
    fn emit_node_state_err(
        &self,
        run_id: &str,
        node_id: &str,
        state: &str,
        counts: (u32, u32, u32, u32),
        error: Option<String>,
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
            error,
        });
    }

    /// P-lumina: phat [`EngineEvent::NodeOutputs`] khi `outputs` co it nhat MOT asset anh/video —
    /// cho cac node API (Veo3/Google/Seedance/Grok/MuseAI...) khong di qua `run_video_jobs`/dong
    /// `tasks` rieng (chi `dola_video` co) — UI preview (`OutputPreview.tsx`) khong co nguon nao
    /// khac de biet duong dan. Lay asset DAU TIEN tim thay (duyet cac cong theo thu tu `BTreeMap`,
    /// trong tung cong theo thu tu item) — du cho mot "dai dien" preview, khop cach `OutputPreview`
    /// chi hien MOT anh/video moi node. Khong co asset nao (vd node Text/Save) -> khong phat gi.
    fn emit_preview_if_media(&self, run_id: &str, node_id: &str, outputs: &Outputs) {
        let Some(path) = outputs.values().flatten().find_map(|v| match v {
            Value::Image(a) | Value::Video(a) => Some(a.path.to_string_lossy().into_owned()),
            Value::Text(_) | Value::Audio(_) => None,
        }) else {
            return;
        };
        self.events().emit(EngineEvent::NodeOutputs {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            preview_path: path,
        });
    }

    /// `counts = (done, total, ok, failed)` — buoc khong mang thong diep loi (`error: None`).
    fn emit_node_state(
        &self,
        run_id: &str,
        node_id: &str,
        state: &str,
        counts: (u32, u32, u32, u32),
    ) {
        self.emit_node_state_err(run_id, node_id, state, counts, None);
    }

    /// Run-robustness B: ghi trang thai node vao `node_states` (ben hoa de khoi phuc khi mo lai
    /// app). Goi inline-await TRONG `run_steps` (async) NGAY SAU moi `emit_node_state*` — awaited
    /// theo thu tu nen trang thai moi (vd `done`) khong bi `running` cu de len. Ghi hong KHONG
    /// lam sap run (chi `warn!`) — su kien van phat, dia van la nguon su that cho output.
    async fn persist_node_state(
        &self,
        run_id: &str,
        node_id: &str,
        state: &str,
        counts: (u32, u32, u32, u32),
        error: Option<&str>,
    ) {
        if let Err(e) = self
            .node_states
            .upsert(run_id, node_id, state, counts, error, now_unix())
            .await
        {
            warn!(run_id, node_id, error = %e, "khong ghi duoc node_states");
        }
    }

    /// Run-robustness B: quet `downloads_dir` tim file output cua tung node theo ten TAT DINH. Hai
    /// quy uoc dat ten cung ton tai (CO Y khong ep node theo mot khuon — engine KHONG biet kind):
    /// - Node sinh canh (`museai.rs`/`veo3.rs`/apimedia): `{run_id}_{node_id}_{scene}.ext`.
    /// - Node lap rap (concat/trim/audio/frame): `{tag}_{run_id}_{node_id}.ext` (vd
    ///   `concat_{run_id}_{node_id}.mp4`) — KHONG bat dau bang `{run_id}`, KHONG co `_{scene}`.
    ///
    /// Khop ca hai bang cach tim TOKEN `{run_id}_{node_id}` co RANH GIOI: dung dau chuoi hoac ngay
    /// sau `_` (sau tag), va ngay sau token la `_` (canh) hoac `.` (duoi file). Tra `node_id ->
    /// duong dan` cho MOI node co it nhat MOT file (file DAU TIEN doc duoc — du cho mot "dai dien"
    /// preview, khop `emit_preview_if_media`). Nhan `node_ids` TUONG MINH (tu `node_states`) —
    /// KHONG parse mu. Thu muc khong ton tai/rong -> map rong, khong loi (app moi chua chay gi).
    #[must_use]
    pub fn node_outputs(&self, run_id: &str, node_ids: &[String]) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let Ok(entries) = std::fs::read_dir(&self.cfg.downloads_dir) else {
            return out;
        };
        let names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        for node_id in node_ids {
            let token = format!("{run_id}_{node_id}");
            if let Some(name) = names.iter().find(|n| name_matches_token(n, &token)) {
                let path = self.cfg.downloads_dir.join(name);
                out.insert(node_id.clone(), path.to_string_lossy().into_owned());
            }
        }
        out
    }

    /// Khoi tao mot lan chay: phat `queued` cho MOI node (mang dung `total`), dung `Services` (co
    /// dich vu `DolaVideoHost`) va clone `Limits`. Tach khoi [`Engine::run_steps`] cho gon.
    fn start_run(
        self: &Arc<Self>,
        run_id: &str,
        graph: &Graph,
        prepared: &Prepared,
        hooks_factory: HooksFactory,
        tasks_ready_tx: &watch::Sender<Vec<TaskRef>>,
        cancel: &CancellationToken,
    ) -> (Services, Arc<lvc_core::Limits>) {
        for n in &graph.nodes {
            let total = prepared.jobs.get(&n.id).copied().unwrap_or(1);
            self.emit_node_state(run_id, &n.id, NODE_QUEUED, (0, total, 0, 0));
        }
        let mut services = Services::new();
        services.insert::<Arc<dyn DolaVideoHost>>(Arc::new(RunDolaHost {
            engine: Arc::clone(self),
            hooks_factory,
            tasks_ready_tx: tasks_ready_tx.clone(),
        }));
        // P4d (ADR-R014): chen dich vu sinh text VO DIEU KIEN (engine KHONG `match` theo kind).
        // Resolver mac dinh la NoResolver neu `lvc-app` chua cap (node bao loi ro rang thay vi thieu
        // service). Bi mat resolve NOI BO, KHONG ra event/log.
        services.insert::<Arc<dyn TextGenHost>>(Arc::new(EngineTextGenHost::new(
            Arc::clone(&self.text_client),
            self.text_resolver(),
        )));
        // Luu kich ban (node `script_gen`, tham so `save_as_script`) VO DIEU KIEN, cung khuon cac
        // dich vu khac (engine KHONG `match` theo kind). Dung thang `self.db` (Engine da giu san,
        // xem doc `scripts.rs`) — khong can resolver nhu TextGenHost vi khong co bi mat de giu kin.
        services.insert::<Arc<dyn ScriptSinkHost>>(Arc::new(EngineScriptSinkHost::new(
            self.db.clone(),
        )));
        // P5 Task 9 (ADR-R014): chen dich vu mint reCAPTCHA Veo3 VO DIEU KIEN (engine KHONG `match`
        // theo kind) — node `veo3_image`/`veo3_video` lay qua `ctx.services.get::<Arc<dyn
        // VeoCaptchaHost>>()`. Chua goi `set_veo3_guest_root_dir` thi `mint_token` tra loi ro rang
        // (xem `crate::veo3`), khong thieu service tham lang.
        let captcha_host: Arc<dyn VeoCaptchaHost> = Arc::new(RunVeoCaptchaHost {
            engine: Arc::clone(self),
        });
        services.insert::<Arc<dyn VeoCaptchaHost>>(Arc::clone(&captcha_host));
        // P5 Task 9b (ADR-R014): chen dich vu harvest/cache phien VeoSessionHost VO DIEU KIEN —
        // node khong biet trait nay ton tai (chi `lvc-engine`, phia impl VeoImageHost/VeoVideoHost
        // duoi day, moi goi toi). Chua goi `set_veo3_account_resolver` thi `harvest` tra loi ro
        // rang (xem `crate::veo3::EngineVeoSession`).
        let session_host: Arc<dyn VeoSessionHost> = Arc::new(RunVeoSessionHost {
            engine: Arc::clone(self),
        });
        services.insert::<Arc<dyn VeoSessionHost>>(Arc::clone(&session_host));
        // P5 Task 9c (ADR-R014): chen dich vu be mat NODE thuc VeoImageHost/VeoVideoHost VO DIEU
        // KIEN — ghep 2 dich vu tren qua `lvc_veo3::flow::Flow`, tu quyet `local_path` theo
        // run_id/node_id/scene_index (xem `crate::veo3::EngineVeoJobs`, mirror cach ham nay quyet
        // duong dan cho Dola o tren). `cancel` = token CUA RUN (fix-round 1, C1) — video co the poll
        // nhieu phut nen can kiem huy giua cac lan poll, giong `hooks.token` cua Dola.
        let flow = Flow::new(captcha_host, session_host);
        let veo_jobs = Arc::new(crate::veo3::EngineVeoJobs::new(
            self.cfg.downloads_dir.clone(),
            flow,
            cancel.clone(),
            self.tasks.clone(),
            tasks_ready_tx.clone(),
        ));
        services.insert::<Arc<dyn VeoImageHost>>(Arc::clone(&veo_jobs) as Arc<dyn VeoImageHost>);
        services.insert::<Arc<dyn VeoVideoHost>>(veo_jobs as Arc<dyn VeoVideoHost>);
        // P6 (ADR-R014): chen dich vu node goi API CHINH THUC Google (Imagen/Veo) VO DIEU KIEN —
        // node `google_image`/`google_video` lay qua `ctx.services`. Dung lai `text_resolver()`
        // (resolve API key theo provider tag "gemini") + client HTTP `google_client`. Video can
        // token huy CUA RUN (poll nhieu phut, giong veo_jobs o tren).
        let text_resolver = self.text_resolver();
        services.insert::<Arc<dyn GoogleImageHost>>(Arc::new(EngineGoogleImage::new(
            Arc::clone(&self.google_client),
            Arc::clone(&text_resolver),
            self.cfg.downloads_dir.clone(),
        )));
        services.insert::<Arc<dyn GoogleVideoHost>>(Arc::new(EngineGoogleVideo::new(
            Arc::clone(&self.google_client),
            Arc::clone(&text_resolver),
            self.cfg.downloads_dir.clone(),
            cancel.clone(),
            self.tasks.clone(),
            tasks_ready_tx.clone(),
        )));
        // P6: Seedance (Volcengine) — anh + video (poll task), dung lai text_resolver (tag "seedance").
        services.insert::<Arc<dyn SeedanceImageHost>>(Arc::new(EngineSeedanceImage::new(
            Arc::clone(&self.seedance_client),
            Arc::clone(&text_resolver),
            self.cfg.downloads_dir.clone(),
        )));
        services.insert::<Arc<dyn SeedanceVideoHost>>(Arc::new(EngineSeedanceVideo::new(
            Arc::clone(&self.seedance_client),
            Arc::clone(&text_resolver),
            self.cfg.downloads_dir.clone(),
            cancel.clone(),
            self.tasks.clone(),
            tasks_ready_tx.clone(),
        )));
        // P6: Grok (xAI) — chi anh (tag "grok").
        services.insert::<Arc<dyn GrokImageHost>>(Arc::new(EngineGrokImage::new(
            Arc::clone(&self.grok_client),
            Arc::clone(&text_resolver),
            self.cfg.downloads_dir.clone(),
        )));
        // P-tts: ElevenLabs/MiniMax — node `tts` chon provider qua tham so, tag resolve lay tu
        // `TtsJob::provider` (khac Grok/Google/Seedance tag co dinh mot dong).
        services.insert::<Arc<dyn TtsHost>>(Arc::new(EngineTts::new(
            Arc::clone(&self.tts_client),
            text_resolver,
            self.cfg.downloads_dir.clone(),
        )));
        // MuseAI — chen VO DIEU KIEN (engine KHONG `match` theo kind, dung khuon cac dich vu
        // tren). Chua goi `set_museai_account_resolver` thi node bao loi ro rang (xem
        // `crate::museai::NoAccountResolver`), khong thieu service tham lang.
        services.insert::<Arc<dyn MuseaiVideoHost>>(Arc::new(
            crate::museai::EngineMuseaiJobs::new(
                self.cfg.downloads_dir.clone(),
                self.cfg.logs_dir.clone(),
                self.museai_resolver(),
                cancel.clone(),
                self.museai_bootstrap_locks.clone(),
            ),
        ));
        services.insert::<Arc<dyn MuseaiImageHost>>(Arc::new(
            crate::museai::EngineMuseaiImageJobs::new(
                self.cfg.downloads_dir.clone(),
                self.cfg.logs_dir.clone(),
                self.museai_resolver(),
                cancel.clone(),
                self.museai_bootstrap_locks.clone(),
            ),
        ));
        (services, Arc::new(self.limits.clone()))
    }

    /// I2: phat trang thai CUOI cho cac node CHUA KIP CHAY khi run dung som (huy/`hard_stop`) —
    /// `cancelled` (huy nguoi dung, khong loi) hoac `failed` (mot node khac loi nghiem trong, ghi ly
    /// do vao nhat ky de giai thich vi sao node nay khong chay).
    async fn emit_pending_terminal(
        &self,
        run_id: &str,
        graph: &Graph,
        prepared: &Prepared,
        touched: &HashSet<String>,
        cancelled: bool,
        hard_stop: Option<&str>,
    ) {
        let pending_state = if cancelled {
            NODE_CANCELLED
        } else {
            NODE_FAILED
        };
        let pending_err = if cancelled {
            None
        } else {
            hard_stop.map(|m| format!("run dung do loi nghiem trong o node khac: {m}"))
        };
        for n in &graph.nodes {
            if !touched.contains(&n.id) {
                let total = prepared.jobs.get(&n.id).copied().unwrap_or(1);
                self.emit_node_state_err(
                    run_id,
                    &n.id,
                    pending_state,
                    (0, total, 0, total),
                    pending_err.clone(),
                );
                // Run-robustness B: ben hoa luon (cung nguon voi emit — khong drift).
                self.persist_node_state(
                    run_id,
                    &n.id,
                    pending_state,
                    (0, total, 0, total),
                    pending_err.as_deref(),
                )
                .await;
            }
        }
    }

    /// Than vong chay nen cua [`Engine::run_graph`]. Fix round 1 (I3): than vong NODE chay trong
    /// MOT `tokio::spawn` con rieng — panic (vd `hooks_factory` cua caller, hay mot node module)
    /// bi bat va coi la run that bai, khong de dong `runs` ket "running" mai mai.
    async fn run_graph_task(self: Arc<Self>, job: RunJob) {
        let run_id = job.run_id.clone();
        let token = job.token.clone();
        let started = now_unix();
        if let Err(e) = self
            .run_repo
            .create(&run_id, &job.workflow_id, RunStatus::Running, started)
            .await
        {
            warn!(run_id, error = %e, "khong tao duoc dong runs - dung run nay");
            self.run_tokens.remove(&run_id);
            // `tasks_ready_tx` bi drop o day (khong gui gi) — nguoi doi nhan `Err` ngay.
            return;
        }

        let steps_fut = Arc::clone(&self).run_steps(job);
        let (error, failed) = match tokio::spawn(steps_fut).await {
            Ok(r) => r,
            Err(join_err) => {
                warn!(run_id, error = %join_err, "than vong chay node panic - ket thuc run la failed");
                (
                    Some(format!("loi he thong: than vong chay panic ({join_err})")),
                    true,
                )
            }
        };

        self.finish_run(run_id, &token, error, failed).await;
    }

    /// Chay tung node theo thu tu topo (`Prepared::order`) qua registry — engine KHONG biet kind
    /// nao. Tra `(thong diep loi cho runs.error, co that bai hay khong)`.
    ///
    /// - Huy/`hard_stop` (loi `NodeError::fatal`) -> dung TRUOC node ke tiep; node chua kip chay
    ///   duoc phat MOT `NodeState` cuoi (`cancelled`/`failed`) thay vi bo mac "queued" (I2).
    /// - Node that bai mot phan (vd 1/3 canh) -> van co dau ra phan OK, node sau (Luu) van chay.
    /// - Node loi khong nghiem trong -> KHONG co dau ra; node sau nhan `EdgeInput.values = None`.
    #[allow(
        clippy::too_many_lines,
        reason = "mot buoc tuan tu (start_run -> vong lap topo -> emit_pending_terminal -> tasks_ready)"
    )]
    async fn run_steps(self: Arc<Self>, job: RunJob) -> (Option<String>, bool) {
        let RunJob {
            run_id,
            workflow_id,
            graph,
            prepared,
            registry,
            token,
            hooks_factory,
            tasks_ready_tx,
            seed_outputs,
            skip,
        } = job;
        let total_of = |id: &str| prepared.jobs.get(id).copied().unwrap_or(1);
        let (services, limits) = self.start_run(
            &run_id,
            &graph,
            &prepared,
            hooks_factory,
            &tasks_ready_tx,
            &token,
        );

        // Run-robustness B: ben hoa trang thai "queued" (start_run da PHAT su kien queued o tren;
        // o day ghi them vao `node_states` de khoi phuc duoc ke ca khi app tat truoc khi node chay).
        for n in &graph.nodes {
            self.persist_node_state(
                &run_id,
                &n.id,
                NODE_QUEUED,
                (0, total_of(&n.id), 0, 0),
                None,
            )
            .await;
        }

        // Phần C: nạp output SEED (node đã xong ở lần chạy trước) để node phía sau `gather_inputs`
        // thấy — chúng được BỎ QUA thực thi trong vòng lặp (xem `skip`).
        let mut outputs: HashMap<String, Outputs> = seed_outputs;
        let mut touched: HashSet<String> = HashSet::new();
        let mut failures: Vec<String> = Vec::new();
        let mut hard_stop: Option<String> = None;

        for node_id in &prepared.order {
            // I2: huy giua chung -> dung NGAY truoc node ke tiep (ke ca node Luu ngay sau node video
            // vua bi huy — CO Y: file cac canh da xong van con trong downloads_dir).
            if hard_stop.is_some() || token.is_cancelled() {
                break;
            }
            let Some(node) = graph.nodes.iter().find(|n| &n.id == node_id) else {
                continue;
            };
            touched.insert(node_id.clone());
            let total = total_of(node_id);
            // Phần C: node đã xong lần trước (có trong `skip` + đã seed output) -> emit `done` ngay,
            // KHÔNG gọi `exec` (không tốn credit), preview lại từ output seed.
            if skip.contains(node_id) {
                let c = (total, total, total, 0);
                self.emit_node_state(&run_id, node_id, NODE_DONE, c);
                self.persist_node_state(&run_id, node_id, NODE_DONE, c, None)
                    .await;
                if let Some(seeded) = outputs.get(node_id) {
                    self.emit_preview_if_media(&run_id, node_id, seeded);
                    // Final-review Critical 1: MANG SEED sang node_states.outputs cua run MOI — de
                    // lan "chay lai node loi" KE TIEP (chained retry) van seed duoc node da xong,
                    // KHONG re-chay (khong ton credit lai). Thieu buoc nay thi retry lan 2 tro di
                    // se chay lai node dat tien.
                    if let Ok(json) = serde_json::to_string(seeded) {
                        if let Err(e) = self.node_states.set_outputs(&run_id, node_id, &json).await
                        {
                            warn!(run_id, node_id, error = %e, "khong mang duoc seed outputs sang run moi");
                        }
                    }
                }
                continue;
            }
            let Some(exec) = registry.get(&node.kind.kind) else {
                // prepare() da chan kind la — chi phong thu.
                let msg = format!("node {node_id}: kind {} chua cai", node.kind.kind);
                self.emit_node_state_err(
                    &run_id,
                    node_id,
                    NODE_FAILED,
                    (total, total, 0, total),
                    Some(msg.clone()),
                );
                self.persist_node_state(
                    &run_id,
                    node_id,
                    NODE_FAILED,
                    (total, total, 0, total),
                    Some(&msg),
                )
                .await;
                failures.push(msg);
                continue;
            };
            self.emit_node_state(&run_id, node_id, NODE_RUNNING, (0, total, 0, 0));
            self.persist_node_state(&run_id, node_id, NODE_RUNNING, (0, total, 0, 0), None)
                .await;
            let ctx = NodeCtx {
                run_id: run_id.clone(),
                workflow_id: workflow_id.clone(),
                node_id: node_id.clone(),
                cancel: token.clone(),
                limits: Arc::clone(&limits),
                services: services.clone(),
            };
            let inputs = gather_inputs(&graph, node_id, &outputs);
            // Log MOC vao/ra cua tung node. Truoc day ca mot lan chay khong sinh MOT dong log nao:
            // node treo giua chung thi khong co gi de lan ra cho hong (da gap that voi node
            // `museai_video` 2026-10-14). Chi ghi kind + id + so viec — KHONG ghi prompt/bi mat.
            info!(node = %node_id, kind = %node.kind.kind, total, "node: bat dau");
            let started = std::time::Instant::now();
            let outcome = exec.run(&ctx, &node.kind.params, inputs).await;
            info!(
                node = %node_id,
                kind = %node.kind.kind,
                giay = started.elapsed().as_secs(),
                ok = outcome.is_ok(),
                "node: ket thuc"
            );
            match outcome {
                Ok(res) => {
                    let c = res.counts;
                    let failed = res.is_failed();
                    let state = if failed { NODE_FAILED } else { NODE_DONE };
                    // Node fan-out (vd video) da co dong `tasks` rieng cho tung canh; van dinh kem
                    // `res.failure` (vd "canh 1,3 that bai") lam nhat ky cap node khi that bai.
                    let node_err = if failed { res.failure.clone() } else { None };
                    self.emit_node_state_err(
                        &run_id,
                        node_id,
                        state,
                        (c.done, c.total, c.ok, c.failed),
                        node_err.clone(),
                    );
                    self.persist_node_state(
                        &run_id,
                        node_id,
                        state,
                        (c.done, c.total, c.ok, c.failed),
                        node_err.as_deref(),
                    )
                    .await;
                    if let Some(f) = res.failure {
                        failures.push(f);
                    }
                    if !failed {
                        self.emit_preview_if_media(&run_id, node_id, &res.outputs);
                        // Phần C: lưu Outputs (JSON) node đã xong để "chạy lại node lỗi" seed lại,
                        // không chạy lại (không tốn credit). Ghi hỏng chỉ `warn`, không sập run.
                        match serde_json::to_string(&res.outputs) {
                            Ok(json) => {
                                if let Err(e) =
                                    self.node_states.set_outputs(&run_id, node_id, &json).await
                                {
                                    warn!(run_id, node_id, error = %e, "khong luu duoc node outputs");
                                }
                            }
                            Err(e) => {
                                warn!(run_id, node_id, error = %e, "khong serialize duoc node outputs");
                            }
                        }
                    }
                    outputs.insert(node_id.clone(), res.outputs);
                }
                Err(e) => {
                    self.emit_node_state_err(
                        &run_id,
                        node_id,
                        NODE_FAILED,
                        (total, total, 0, total),
                        Some(e.message.clone()),
                    );
                    self.persist_node_state(
                        &run_id,
                        node_id,
                        NODE_FAILED,
                        (total, total, 0, total),
                        Some(&e.message),
                    )
                    .await;
                    if e.fatal {
                        hard_stop = Some(e.message);
                    } else {
                        failures.push(e.message);
                    }
                }
            }
        }

        if hard_stop.is_some() || token.is_cancelled() {
            self.emit_pending_terminal(
                &run_id,
                &graph,
                &prepared,
                &touched,
                token.is_cancelled(),
                hard_stop.as_deref(),
            )
            .await;
        }

        // I1: "chot" tasks_ready KHONG DIEU KIEN (xem doc `RunHandle::tasks_ready`).
        tasks_ready_tx.send_modify(|_| {});

        let error = hard_stop.or_else(|| (!failures.is_empty()).then(|| failures.join("; ")));
        let failed = error.is_some();
        (error, failed)
    }

    /// Chot trang thai cuoi (huy > loi > hoan tat), ghi `RunRepo::set_status`, go token, roi phat
    /// `RunDone` (SAU khi DB da ghi).
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

    /// Fix-round 2 (M5): danh dau MOT `TaskRow` da ton tai la `Failed` + `finished_at` NGAY, khong
    /// spawn job cho no. `reason` KHONG chua prompt/cookie/URL ky.
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

    /// Dich vu `DolaVideoHost` (P4a): tao MOT `TaskRow` cho MOI canh TRUOC KHI spawn (UI liet ke
    /// duoc ngay), roi `JoinSet` chay dong thoi (limiter/`AccountPool` lo song song); mot canh loi
    /// KHONG dung canh khac. Trang thai node do executor phat — ham nay chi tra `JobsReport`.
    ///
    /// Fix-round 2 (I3-NEW) + P3c I1: giai doan 1 goi `hooks_factory` (CO THE panic) +
    /// `register_child` cho TAT CA canh, chua dung DB; 2a tao dong cho tat ca; 2b phat `tasks_ready`;
    /// 2c spawn. Panic o giai doan 1 -> chua co dong nao (khong orphan), lan len bay panic cua run.
    #[allow(
        clippy::too_many_lines,
        reason = "mot buoc tuan tu (chuan bi hooks -> tao dong -> bao -> spawn -> gom ket qua)"
    )]
    async fn run_video_jobs(
        self: &Arc<Self>,
        run_id: &str,
        node_id: &str,
        jobs: Vec<SceneJob>,
        run_token: &CancellationToken,
        hooks_factory: &HooksFactory,
        tasks_ready_tx: &watch::Sender<Vec<TaskRef>>,
    ) -> JobsReport {
        let mut failed_indices: Vec<String> = Vec::new();

        // Giai doan 1 (I3-NEW).
        let mut prepared: Vec<PreparedJob> = Vec::with_capacity(jobs.len());
        for job in jobs {
            if run_token.is_cancelled() {
                failed_indices.push(job.scene_index.to_string());
                continue;
            }
            let task_id = new_id("task");
            let hooks = hooks_factory(&task_id);
            // C1: token cua CANH la CON cua token RUN, dang ky TRUOC KHI spawn.
            let _ = self.cancels().register_child(&task_id, run_token);
            prepared.push(PreparedJob {
                scene_index: job.scene_index,
                scene: job.scene,
                task_id,
                hooks,
            });
        }

        // Giai doan 2a: tao dong CHO TAT CA canh, chua spawn.
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
                // M5: dong thieu run_id/node_id se vo hinh voi `list_by_run` -> Failed NGAY.
                self.mark_row_failed(&pj.task_id, &format!("store: set_run: {e}"))
                    .await;
                failed_indices.push(pj.scene_index.to_string());
                continue;
            }
            created.push(pj);
        }

        // Giai doan 2b: bao TaskRef TRUOC khi spawn.
        tasks_ready_tx.send_modify(|v| {
            v.extend(created.iter().map(|c| TaskRef {
                node_id: node_id.to_string(),
                scene_index: c.scene_index,
                task_id: c.task_id.clone(),
            }));
        });

        // Giai doan 2c: spawn.
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

        // KHONG bao gio bo JoinSet giua chung — luon join toi het (dong task khong ket processing).
        let mut done: Vec<VideoDone> = Vec::new();
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(VideoJobResult {
                    scene_index,
                    result: Ok(out),
                    ..
                }) => done.push(VideoDone {
                    scene_index,
                    local_path: out.artifact.local_path,
                    account: out.account,
                    conversation_id: out.artifact.conversation_id.0,
                    watermarked: out.artifact.watermarked,
                }),
                Ok(VideoJobResult {
                    task_id,
                    scene_index,
                    result: Err(e),
                }) => {
                    failed_indices.push(scene_index.to_string());
                    // M3: canh bi huy MA CHUA BAO GIO chay van can `finished_at`.
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
        done.sort_by_key(|d| d.scene_index);
        JobsReport {
            done,
            failed: failed_indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use lvc_browser::dola::{ConversationId, VideoArtifact, poll::NOTE_RAW};
    use lvc_browser::{DolaError, FakeCall, FakeDriver, FakeStep};
    use lvc_core::{
        Edge, FailureCode, Limits, LogoMode, Mode, Model, Node, NodeData, Ratio, RawScene,
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
        let engine = Arc::new(engine);
        assert!(engine.set_registry(Arc::new(crate::test_support::test_registry())));
        (dir, engine, db, driver)
    }

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
        video_kind_for("")
    }

    /// Nhu [`video_kind`] nhung chi dinh `account` (fix-round 1, C1 test 2: node "chi dinh" mot
    /// tai khoan cu the -> `Scene::account` -> `prefer` -> `run_mode` vao nhanh `lease_wait`).
    fn video_kind_for(account: &str) -> NodeData {
        NodeData::new(
            "dola_video",
            serde_json::json!({
                "mode": Mode::Fast,
                "model": Model::Seedance20,
                "ratio": Ratio::R9x16,
                "duration": 10,
                "logo": LogoMode::Off,
                "account": account,
            }),
        )
    }

    fn prompt_kind(text: impl Into<String>) -> NodeData {
        NodeData::new("prompt", serde_json::json!({ "text": text.into() }))
    }

    fn save_kind(out_dir: impl Into<String>) -> NodeData {
        NodeData::new("save", serde_json::json!({ "out_dir": out_dir.into() }))
    }

    fn upload_kind(paths: &[String]) -> NodeData {
        NodeData::new("image_upload", serde_json::json!({ "paths": paths }))
    }

    /// Do thi Prompt("pr") -> Tao video("vid") -> Luu("sv"), dung o hau het test.
    fn linear_graph(prompt_text: &str, out_dir: &std::path::Path) -> Graph {
        Graph {
            version: 2,
            nodes: vec![
                node("pr", prompt_kind(prompt_text.to_string())),
                node("vid", video_kind()),
                node("sv", save_kind(out_dir.display().to_string())),
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
            error: None,
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
            version: 2,
            nodes: vec![
                node("pr", prompt_kind("canh 1".to_string())),
                node("vid", video_kind_for("acc1")),
                node("sv", save_kind(out_dir.display().to_string())),
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

    // ---- Run-robustness B: node_states ghi inline + node_outputs quet dia ----

    /// Sau mot run thanh cong, MOI node co dung MOT dong `node_states` mang trang thai CUOI
    /// (`done`, `done==total`) — de khoi phuc khi mo lai app.
    #[tokio::test]
    async fn node_states_moi_node_co_dong_trang_thai_cuoi() {
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("src.mp4");
        std::fs::write(&src, b"fake-mp4-bytes").unwrap();

        let (dir, engine, _db, _driver) = build_engine(&["acc1"], 1, happy_steps(1, &src)).await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1", &out_dir);

        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        let run_id = handle.run_id.clone();
        handle.join.await.unwrap();

        let rows = engine.node_states.list_for_run(&run_id).await.unwrap();
        let mut by_node: std::collections::HashMap<String, _> =
            rows.into_iter().map(|r| (r.node_id.clone(), r)).collect();
        for n in ["pr", "vid", "sv"] {
            let r = by_node
                .remove(n)
                .unwrap_or_else(|| panic!("thieu node_states cho {n}"));
            assert_eq!(r.state, NODE_DONE, "{n}");
            assert_eq!(r.done, r.total, "{n} done==total");
        }
    }

    /// `node_outputs` khop dung tien to `{run_id}_{node_id}_` — KHONG nham node khac co tien to
    /// gan giong (`node_id` chua dau gach duoi). Thu muc rong -> map rong.
    #[tokio::test]
    async fn node_outputs_khop_tien_to_node_id() {
        let (dir, engine, _db, _driver) = build_engine(&["acc1"], 1, Vec::new()).await;
        let downloads = dir.path().join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        std::fs::write(downloads.join("runX_im_scene_1_0.png"), b"img").unwrap();
        std::fs::write(downloads.join("runX_other_0.mp4"), b"vid").unwrap();

        let got = engine.node_outputs("runX", &["im_scene_1".to_string()]);
        assert_eq!(got.len(), 1);
        assert!(
            got["im_scene_1"].ends_with("runX_im_scene_1_0.png"),
            "{got:?}"
        );
    }

    /// Node LAP RAP (concat/trim/audio/frame) dat ten `{tag}_{run_id}_{node_id}.ext` — KHONG bat
    /// dau bang `{run_id}` va KHONG co `_{scene}` duoi. `node_outputs` van phai khop (video reel
    /// cuoi — node "Noi video" — la thu nguoi dung muon thay nhat sau khi mo lai app).
    #[tokio::test]
    async fn node_outputs_khop_ca_node_lap_rap() {
        let (dir, engine, _db, _driver) = build_engine(&["acc1"], 1, Vec::new()).await;
        let downloads = dir.path().join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        // concat: `concat_{run}_{node}.mp4`; trim: `trim_{run}_{node}.mp4`.
        std::fs::write(downloads.join("concat_runX_nC.mp4"), b"reel").unwrap();
        std::fs::write(downloads.join("trim_runX_nT.mp4"), b"t").unwrap();

        let got = engine.node_outputs("runX", &["nC".to_string(), "nT".to_string()]);
        assert!(got["nC"].ends_with("concat_runX_nC.mp4"), "{got:?}");
        assert!(got["nT"].ends_with("trim_runX_nT.mp4"), "{got:?}");
    }

    /// `name_matches_token` (thuan): khop ca hai quy uoc, chan nham `node_id` dai hon.
    #[test]
    fn name_matches_token_ranh_gioi() {
        let tok = "run1_nA";
        assert!(super::name_matches_token("run1_nA_0.png", tok), "canh");
        assert!(
            super::name_matches_token("concat_run1_nA.mp4", tok),
            "lap rap"
        );
        assert!(
            super::name_matches_token("split_audio_run1_nA.m4a", tok),
            "tag dai"
        );
        // node_id khac dai hon (nA vs nAB) KHONG duoc khop nham.
        assert!(
            !super::name_matches_token("run1_nAB_0.png", tok),
            "node_id dai hon"
        );
        // khong co token.
        assert!(
            !super::name_matches_token("run1_nZ_0.png", tok),
            "node khac"
        );
    }

    /// Phần C: `run_graph_seeded` BỎ QUA node trong `skip` (đã xong lần trước, có `seed_outputs`) —
    /// KHÔNG gọi driver (không tốn credit), node phía sau dùng output seed, run vẫn hoàn tất.
    #[tokio::test]
    async fn run_graph_seeded_bo_qua_node_skip_khong_chay() {
        let (dir, engine, _db, driver) = build_engine(&["acc1"], 1, Vec::new()).await;
        let out_dir = dir.path().join("out");
        let graph = linear_graph("canh 1", &out_dir);

        // File video "da xong" tu lan truoc (seed cho node `vid`).
        let seeded_file = dir.path().join("seeded_vid.mp4");
        std::fs::write(&seeded_file, b"seeded-mp4").unwrap();
        let mut vid_out = Outputs::new();
        vid_out.insert(
            "out".to_string(),
            vec![Value::Video(lvc_node_api::Asset {
                path: seeded_file.clone(),
                ..Default::default()
            })],
        );
        let mut seed = HashMap::new();
        seed.insert("vid".to_string(), vid_out);
        let skip: HashSet<String> = ["vid".to_string()].into_iter().collect();

        let handle = engine
            .run_graph_seeded("wf1", &graph, noop_hooks_factory(), seed, skip)
            .await
            .unwrap();
        let run_id = handle.run_id.clone();
        handle.join.await.unwrap();

        // `vid` bi skip -> KHONG mo driver (khong ton credit).
        assert!(
            driver.calls.lock().unwrap().is_empty(),
            "vid skip khong duoc mo driver"
        );
        assert_eq!(
            engine.run_repo.get(&run_id).await.unwrap().unwrap().status,
            RunStatus::Completed
        );
        let states = engine.node_states.list_for_run(&run_id).await.unwrap();
        assert!(
            states
                .iter()
                .any(|s| s.node_id == "vid" && s.state == NODE_DONE),
            "{states:?}"
        );
        // sv (Luu) da nhan output SEED cua vid va luu ra out_dir (>=1 file) — chung to seed chay
        // xuyen toi node phia sau du vid bi skip.
        let saved = std::fs::read_dir(&out_dir).map_or(0, |rd| rd.flatten().count());
        assert!(saved >= 1, "sv phai luu it nhat 1 file tu output seed");
    }

    /// Thu muc downloads khong ton tai -> map rong, khong panic (app moi, chua chay gi).
    #[tokio::test]
    async fn node_outputs_thu_muc_khong_ton_tai() {
        let (_dir, engine, _db, _driver) = build_engine(&["acc1"], 1, Vec::new()).await;
        // build_engine chua tao san `downloads` -> khong ton tai.
        let got = engine.node_outputs("runX", &["n1".to_string()]);
        assert!(got.is_empty());
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
            version: 2,
            nodes: vec![
                node("prA", prompt_kind("canh A".to_string())),
                node("vidA", video_kind()),
                node("svA", save_kind(out_dir.join("a").display().to_string())),
                node("prB", prompt_kind("canh B".to_string())),
                node("vidB", video_kind()),
                node("svB", save_kind(out_dir.join("b").display().to_string())),
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
            version: 2,
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

    // ---- P-lumina: NodeOutputs preview cho node KHONG di qua dong `tasks` rieng ----

    /// `image_upload` khong phai `dola_video` (khong co `run_video_jobs`/dong `tasks` rieng) —
    /// trươc fix nay UI preview (`OutputPreview.tsx`) khong co cach nao biet duong dan asset da
    /// sinh. Xac nhan `emit_preview_if_media` phat dung `NodeOutputs` voi path cua anh dau tien.
    #[tokio::test]
    async fn node_khong_fan_out_sinh_anh_phat_node_outputs_preview() {
        let (dir, engine, _db, _driver) = build_engine(&[], 1, vec![]).await;
        let img_path = dir.path().join("ref.jpg");
        std::fs::write(&img_path, b"fake-jpg-bytes").unwrap();

        let graph = Graph {
            version: 2,
            nodes: vec![node("im", upload_kind(&[img_path.display().to_string()]))],
            edges: vec![],
        };

        let mut rx = engine.events().subscribe();
        let handle = engine
            .run_graph("wf1", &graph, noop_hooks_factory())
            .await
            .unwrap();
        handle.join.await.unwrap();

        let mut found: Option<(String, String)> = None;
        while let Ok(ev) = rx.try_recv() {
            if let EngineEvent::NodeOutputs {
                node_id,
                preview_path,
                ..
            } = ev
            {
                found = Some((node_id, preview_path));
            }
        }
        let (node_id, preview_path) = found.expect("phai phat NodeOutputs cho node im");
        assert_eq!(node_id, "im");
        assert_eq!(preview_path, img_path.display().to_string());
    }

    // ---- (5) ImageUpload thieu file: run failed, khong tao task video nao ----

    #[tokio::test]
    async fn image_upload_thieu_file_run_failed_khong_tao_task_video() {
        let (dir, engine, db, driver) = build_engine(&[], 1, vec![]).await;
        let out_dir = dir.path().join("out");
        let missing = dir.path().join("khong_ton_tai.jpg");
        let graph = Graph {
            version: 2,
            nodes: vec![
                node("pr", prompt_kind("canh 1".to_string())),
                node("im", upload_kind(&[missing.display().to_string()])),
                node("vid", video_kind()),
                node("sv", save_kind(out_dir.display().to_string())),
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
