# P3 — Tauri + React Flow: node graph, DAG executor, tab tài khoản: Kế hoạch triển khai

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Người dùng mở app desktop (Tauri 2), kéo thả **Prompt → Tạo video → Lưu** (kèm ảnh tham chiếu), bấm Chạy → engine P2 tạo N video song song theo hạn mức, thấy tiến độ từng cảnh, huỷ được, mở lại app vẫn tiếp tục; tab Tài khoản nạp cookie, bật/tắt lịch, đặt proxy, kiểm đăng nhập. **Nghiệm thu: kéo thả 3 cảnh → 3 video thật.**

**Architecture:** Bốn tầng mới trên P2:
1. `lvc-core::graph` (thuần): mô hình đồ thị (node/cạnh/cổng), validate (kiểu cổng, chu trình, node bắt buộc), topo-sort, **map ngầm** (Prompt nhả N dòng → N lượt Tạo video, `scene_index` 1-based), **barrier** (cổng `collect`). Serde JSON = định dạng lưu workflow.
2. `lvc-engine` mở rộng: **huỷ** (`CancellationToken`), **sự kiện** (`EngineEvent` qua `broadcast`), **resume** từ `conversation_id`, requeue khi khởi động (`RESTART`), `Limits`/`EngineConfig` từ bảng `settings`, **DAG executor** chạy đồ thị (fan-out video theo limiter, join ở barrier).
3. `lvc-app` (Tauri 2, `src-tauri/`): state `AppState { engine, db, events }`, **commands** (ADR-R010: không HTTP server), **emit** sự kiện (ADR-R005), DB tại `app_data_dir/lvc.db`, nhập dữ liệu Python lần đầu (`legacy.rs`).
4. Frontend `app/` (Vite + React 19 + TS + `@xyflow/react` + zustand + Tailwind): 4 màn — Workflow (canvas), Tác vụ, Tài khoản, Cài đặt. Tiếng Việt.

**Tech Stack:** tauri 2.10 (plugins: dialog, fs, shell-open) · tokio-util (CancellationToken) · serde · @xyflow/react 12 · zustand · vitest · pnpm 10 · Node 24 · WebView2 (có sẵn) · ffmpeg tại `C:\ffmpeg\bin` (P4 mới dùng)

**Spec:** `docs/superpowers/specs/2026-09-16-lvc-seedance-rust-design.md` §4 (node graph), §5, §6, §7.1/7.3, ADR-R003/R004/R005/R007/R010, ADR-R013. **Đầu vào từ P2:** `.superpowers/sdd/2026-09-17-p2-dola-video/final-review.md` §3 (park) + §4 (5 gap P3).

## Global Constraints

- Mọi ràng buộc P0–P2 giữ nguyên: `lvc-core` thuần (gate `cargo tree`); clippy all+pedantic `-D warnings`; PF-3; LF; không log cookie/prompt/URL ký; headed; `--user-data-dir` tuyệt đối; policy log `chromiumoxide=warn,chromiumoxide::handler=error` nối SAU directive người dùng.
- **Chỉ Tauri commands** (ADR-R010). Không axum, không port mở. Frontend gọi `invoke`, nhận `listen`.
- **Không đụng `A:\ToolLVC\lvcseedance`** (DB/profile gốc). Nhập dữ liệu = đọc copy/read-only; profile Chromium **giữ tại chỗ**: `accounts_dir` mặc định trỏ tới thư mục người dùng chọn (mặc định đề xuất `A:\ToolLVC\lvcseedance\accounts` — chỉ đọc/ghi profile của tài khoản đang chạy, y như Python đang làm; **nhưng P3 dev/test dùng bản copy**).
- **Live tốn credit** (Task 16 P3: 3 cảnh ≈ 6 credit) — CHỈ chạy khi người dùng đồng ý trong phiên. Mọi task khác xanh không cần Dola.
- Frontend: TypeScript `strict`, không `any` ở IPC; kiểu IPC sinh từ Rust bằng tay nhưng **ghim bằng test JSON** (Rust serialize fixture → TS parse) để không lệch.
- Tiếng Việt cho UI; tên node/kiểu cổng theo spec §4.2.
- Auto-login Google/Facebook (spec §7.2) **KHÔNG** ở P3 (plan riêng P3b). P3 tab tài khoản = danh sách + nạp cookie + bật/tắt lịch + proxy + kiểm đăng nhập (probe) + xoá.

---

## Bố cục file

```
crates/lvc-core/src/graph/{mod,node,edge,validate,plan}.rs      # T1-T2
crates/lvc-engine/src/{cancel,events,settings,resume,dag}.rs      # T3-T7
crates/lvc-store/{migrations/0002_p3.sql, src/{settings,tasks,workflows}.rs}  # T3/T7
crates/lvc-app/                                                  # T8-T10 (Tauri src-tauri)
  Cargo.toml, tauri.conf.json, capabilities/default.json, build.rs, icons/
  src/{main,lib,state,commands/{workflow,tasks,accounts,settings},events,paths,bootstrap}.rs
app/                                                             # T11-T15 (Vite React)
  package.json, vite.config.ts, tsconfig.json, index.html, tailwind
  src/{main.tsx, App.tsx, ipc/{types.ts,commands.ts,events.ts}, store/{graph.ts,tasks.ts,accounts.ts,settings.ts},
       pages/{Workflow,Tasks,Accounts,Settings}.tsx, nodes/{PromptNode,ImageUploadNode,VideoNode,SaveNode}.tsx,
       components/{Palette,RunBar,ProgressPanel,AccountRow,CookieImport}.tsx}
  tests/*.test.ts (vitest)
```

## Quyết định P3 (ruling)

| # | Quyết định | Vì sao | Nếu sai tốn gì |
|---|---|---|---|
| R1 | Định dạng workflow = JSON `Graph` của lvc-core (serde), lưu bảng `workflows(id, name, json, updated_at)`; React Flow state ↔ `Graph` qua adapter thuần trong TS (test vitest) | Một nguồn sự thật, Rust validate | Adapter 1 file |
| R2 | Executor P3 chỉ 4 node: **Prompt**, **Tải ảnh lên**, **Tạo video**, **Lưu**; node P4 (Tạo ảnh, Xoá logo, H.264, Ghép, Lấy ảnh URL) có trong palette dạng *mờ, chưa hỗ trợ* | Nghiệm thu P3 = 3 video; P4 thêm executor không đổi mô hình | 0 |
| R3 | Map ngầm: `Prompt` nhả `Vec<Scene>` từ `split_bulk`/`parse_txt` (định dạng TXT P0 có `ratio,dur,model,mode,logo,acc,refs`); node Tạo video nhận `Text[]` + `Image[]?` → chạy **mỗi cảnh một task** qua `Engine::generate_video`, song song theo limiter; kết quả `Video[]` giữ `scene_index` | Đúng spec §4.3, tái dùng P0 | 0 |
| R4 | Huỷ = `CancellationToken` (tokio-util) truyền vào `generate_video`; kiểm tại 5 điểm (trước launch, sau lease, trước submit, mỗi tick poll qua `PollHooks::should_cancel()`, trước download); huỷ sau submit → giữ `conversation_id` trong DB + `FailureCode::Cancelled`, phiên trả cache (Python giữ conv để Resume) | Gap P3 #1 | 0 |
| R5 | Sự kiện = `enum EngineEvent { TaskStage{task_id, stage, detail, elapsed_ms}, TaskDone{task_id, artifact}, TaskFailed{task_id, code, message}, NodeState{run_id, node_id, state}, RunDone{run_id} }` `Serialize`, gửi qua `tokio::sync::broadcast` (cap 1024, lag = bỏ qua có log); Tauri bridge forward thành `emit("engine://event", payload)` | ADR-R005 | 0 |
| R6 | Resume: `Engine::resume(task_id)` khi row có `conversation_id` và `status=processing|failed(cancelled/restart)`; khi app khởi động: `processing` không conv → `queued` + `RESTART`; có conv → tự resume (Python `requeue_orphans`) | Gap #3, spec §4.4 | 0 |
| R7 | Settings: bảng `settings(key TEXT PK, value TEXT)` (đã có P0) → `AppSettings { threads, accounts_dir, output_dir, silent_mode, mode_fallback, default_mode, default_model, default_ratio, default_duration }` serde; `Limits` vẫn mặc định (đổi = P5) | Gap #4 | 0 |
| R8 | Tauri 2 với `tauri-plugin-dialog` (chọn file/thư mục), `tauri-plugin-shell` (mở thư mục output), **không** `fs` plugin cho frontend (đọc file ở Rust) | Ít bề mặt | 0 |
| R9 | Frontend không test e2e tự động (không Playwright cho UI); test = vitest cho adapter/store thuần + `tsc --noEmit` + `vite build` trong gate | UI người dùng tự thử; e2e sau P5 | Lỗi UI phát hiện muộn hơn |
| R10 | `cargo tauri dev` là cách chạy chính trong P3; `cargo tauri build` (NSIS) thử một lần ở Task 15 để chắc đóng gói được, ký/nén/license = P5 | Người dùng cần chạy ngay | 0 |
| R11 | Cookie import: `parse_netscape` (P1) → `validate_dola_cookies` → tạo `accounts/<tên>/` mới (Chromium 1234 launch headless, `add_cookies`, goto `/chat`, `assert_logged_in`; thất bại → xoá thư mục, báo `COOKIE_NOT_LOGGED_IN`) — port `add_account_from_cookies` (add_account.py:547-596). Không ghi cookie ra file | spec §7.1 | 0 |
| R12 | Legacy import lần đầu: nếu `lvc.db` mới và người dùng chọn thư mục Python → `import_legacy` (P0) cho `tasks.db`/`pool_usage.db` **bản copy tạm** (đọc `?mode=ro`), accounts_dir trỏ thẳng profile gốc | §6.5 | 0 |

---

### Task 1: `lvc-core::graph` — mô hình + validate

**Files:** Create `crates/lvc-core/src/graph/{mod.rs,node.rs,edge.rs,validate.rs}`; Modify `lib.rs`.

**Interfaces — Produces:**
```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)] pub enum PortType { Text, Image, Video }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    Prompt { text: String },                                   // Text[] (bulk TXT)
    ImageUpload { paths: Vec<String> },                        // Image[]
    ImageFromUrl { url: String },                              // P4 — validate: Unsupported
    ImageGen { prompt: String, backend: String },              // P4
    Video { mode: Mode, model: Model, ratio: Ratio, duration: u16, logo: LogoMode, account: String }, // Text[] + Image[]? → Video[]
    RemoveLogo, EnsureH264, Concat,                            // P4
    Save { out_dir: String },                                  // Video[]|Image[] → –
}
pub struct Node { pub id: String, pub kind: NodeKind, pub x: f32, pub y: f32 }
pub struct Edge { pub id: String, pub from: String, pub from_port: String, pub to: String, pub to_port: String }
pub struct Graph { pub version: u32 /* =1 */, pub nodes: Vec<Node>, pub edges: Vec<Edge> }
pub struct PortSpec { pub name: &'static str, pub ty: PortType, pub many: bool, pub required: bool, pub collect: bool }
impl NodeKind { pub fn inputs(&self) -> &'static [PortSpec]; pub fn outputs(&self) -> &'static [PortSpec]; pub fn supported_in_p3(&self) -> bool; pub fn label_vi(&self) -> &'static str }
#[derive(thiserror::Error, Debug, PartialEq)] pub enum GraphError { DuplicateNodeId(String), UnknownNode(String), UnknownPort{node,port}, TypeMismatch{edge, from: PortType, to: PortType}, MissingRequiredInput{node, port}, Cycle(Vec<String>), Unsupported{node, kind: String}, Empty, MultipleInputsOnSinglePort{node, port} }
pub fn validate(g: &Graph) -> Result<(), Vec<GraphError>>;   // gom hết lỗi
pub fn topo_order(g: &Graph) -> Result<Vec<String>, GraphError>;   // Kahn; Cycle → chu trình nhỏ nhất tìm được
```
Port table (spec §4.2): Prompt → `out: Text[]`; ImageUpload → `out: Image[]`; Video ← `prompt: Text[] (required)`, `refs: Image[] (optional)` → `out: Video[]`; Save ← `in: Video[] | Image[] (required; nhận cả hai kiểu → PortSpec riêng `video`, `image`, cần ≥1)`; Concat ← `in: Video[] collect`. Kiểm kiểu: `Text→Text`, `Image→Image`, `Video→Video`; `many=false` cổng chỉ nhận 1 cạnh.

- [ ] **Step 1: RED** — tests: serde round-trip JSON (fixture string cố định ghim `kind`/`snake_case`); `validate` từng lỗi (id trùng, cạnh tới node lạ, port lạ, Text→Image mismatch, Video thiếu prompt, chu trình A→B→A, Concat → Unsupported trong P3, đồ thị rỗng, 2 cạnh vào cổng `many=false`); `topo_order` cho hình kim cương; đồ thị hợp lệ 3 node → Ok.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(core): graph — mo hinh node/canh/cong, validate, topo`.

---

### Task 2: `lvc-core::graph::plan` — map ngầm thành kế hoạch chạy

**Files:** Create `crates/lvc-core/src/graph/plan.rs`.

**Interfaces — Produces:**
```rust
pub struct VideoJob { pub node_id: String, pub scene_index: u32 /* 1-based */, pub scene: Scene, pub ref_paths: Vec<String> }
pub struct SaveJob { pub node_id: String, pub out_dir: String, pub from_node: String }
pub enum Step { Prompt { node_id: String, scenes: Vec<Scene> }, ImageUpload { node_id: String, paths: Vec<String> }, Video { node_id: String, jobs: Vec<VideoJob> }, Save(SaveJob) }
pub struct RunPlan { pub steps: Vec<Step> /* theo topo */, pub video_total: u32 }
pub fn plan(g: &Graph, limits: &Limits) -> Result<RunPlan, Vec<GraphError>>;
// Prompt: `bulk_scenes(text, limits)` (P0) → Vec<Scene>; lỗi scene → GraphError::Scene{node, line, msg}
// Video: với mỗi scene từ cổng prompt: scene.mode/model/ratio/duration/logo = giá trị TXT nếu có, else node.kind mặc định;
//        scene.reference_images = refs TXT (nếu có) ∪ paths từ cổng refs (ImageUpload) — tối đa Limits.reference_image_max_count;
//        scene.account = node.account nếu khác "" else giá trị TXT `acc`.
// Save: out_dir từ node.
```
- [ ] **Step 1: RED** — Prompt 3 dòng → Video 3 job `scene_index` 1..3; TXT có `ratio=16:9` thắng mặc định node; refs từ ImageUpload gắn vào mọi cảnh; vượt `reference_image_max_count` → lỗi; Save nối Video → SaveJob; `video_total` = 3.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(core): graph plan — map ngam Prompt→N video`.

---

### Task 3: `lvc-store` — migration 0002 + `settings`/`workflows`/`tasks` mở rộng

**Files:** Create `crates/lvc-store/migrations/0002_p3.sql`; Modify `src/{settings,tasks}.rs`; Create `src/workflows.rs`; tests.

`0002_p3.sql`: `ALTER TABLE tasks ADD COLUMN run_id TEXT NOT NULL DEFAULT ''`, `node_id TEXT NOT NULL DEFAULT ''`, `stage TEXT NOT NULL DEFAULT ''`, `started_at REAL NOT NULL DEFAULT 0`, `finished_at REAL NOT NULL DEFAULT 0`, `retry_at REAL NOT NULL DEFAULT 0`; `CREATE TABLE workflows(id TEXT PRIMARY KEY, name TEXT NOT NULL, json TEXT NOT NULL, updated_at REAL NOT NULL)`; `CREATE TABLE runs(id TEXT PRIMARY KEY, workflow_id TEXT NOT NULL, status TEXT NOT NULL, started_at REAL, finished_at REAL, error TEXT)`; index `(run_id)`, `(status, retry_at)`.
- `SettingsRepo`: `get_all() -> HashMap`, `set(key, value)`, `get(key)`; `AppSettings::from_map`/`to_map` (thuần, trong lvc-engine T5 hoặc lvc-core? — ruling: `AppSettings` ở **lvc-core::settings** thuần với defaults + parse, store chỉ key/value).
- `TaskRepo`: `list_by_run(run_id)`, `list_recent(limit)` (có), `list_by_status(status)`, `set_stage(id, stage)`, `set_started/finished`, `requeue_orphans(now) -> Vec<id>` (processing không conv → queued + `RESTART`), `orphans_with_conversation() -> Vec<TaskRow>`.
- `WorkflowRepo`: `upsert(id, name, json)`, `get`, `list`, `delete`. `RunRepo`: `create`, `set_status`, `get`.
- [ ] **Step 1: RED** — test migration áp lên DB P0 (`open_memory` chạy cả 0001+0002), mỗi hàm 1-2 test, `requeue_orphans` 3 trường hợp.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(store): migration 0002 P3 — runs/workflows, tasks stage/run_id, settings repo`.

---

### Task 4: `lvc-engine` — huỷ + sự kiện

**Files:** Create `crates/lvc-engine/src/{cancel.rs,events.rs}`; Modify `generate.rs`, `lib.rs`, `Cargo.toml` (`tokio-util = { version = "0.7", features = ["rt"] }`), lvc-browser `dola/poll.rs` (`PollHooks::should_cancel(&self) -> bool { false }` mặc định + kiểm mỗi tick → `DolaError::Cancelled`), `dola/error.rs` (+`Cancelled` variant → `FailureCode::Cancelled`, không xoay).

```rust
pub struct EngineEvents { tx: broadcast::Sender<EngineEvent> }  // subscribe() -> Receiver; emit() bỏ qua khi không có receiver
pub enum EngineEvent { … theo R5 … }   // Serialize + Clone
pub struct CancelRegistry { map: DashMap<String /*task_id*/, CancellationToken> }  // register(task_id) -> token; cancel(task_id) -> bool; cancel_run(run_id, ids)
impl Engine { pub async fn generate_video_cancellable(&self, task_id, scene, prefer, hooks, token: CancellationToken) -> …; pub fn events(&self) -> &EngineEvents; pub fn cancel(&self, task_id) -> bool }
```
Điểm kiểm huỷ (R4). Sự kiện `TaskStage` phát ở: `launching`, `page`, `mode`, `composer`, `refs`, `sending`, `waiting_conv`, `generating`, `confirm`, `downloading`, `done` — cần **hook từ lvc-browser submit**: thêm `SubmitHooks { fn on_stage(&self, stage: &str) }` vào `SubmitCtx` (mặc định no-op) và gọi tại các bước tương ứng Python `task_stage.set_stage` (video_worker_ui.py: 2433 page, 2476 mode, 2533 sending, 2570 generating, refs 836, prompt 2118, downloading 1266).
- [ ] **Step 1: RED** — FakeDriver: huỷ trước launch → `Cancelled`, không gọi driver; huỷ giữa poll (FakeStep::Poll chờ token — thêm `FakeStep::PollUntilCancelled`) → row `failed/cancelled` **giữ conversation_id**, `claim` không; sự kiện nhận đủ chuỗi stage; broadcast lag không panic.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(engine): huy bang CancellationToken + EngineEvent broadcast; stage hooks tu submit`.

---

### Task 5: `lvc-core::settings` + `lvc-engine::settings` — cấu hình từ bảng settings

**Files:** Create `crates/lvc-core/src/settings.rs` (`AppSettings` + `from_map(HashMap<String,String>)`/`to_map`, defaults: threads 1, accounts_dir "", output_dir "downloads", silent_mode false, mode_fallback true, default_mode fast, default_model seedance-2.0, default_ratio 9:16, default_duration 10; giá trị sai → giữ mặc định + `Vec<String>` cảnh báo); Modify `lvc-engine`: `EngineConfig::from_settings(&AppSettings, app_data_dir)`; `Engine::apply_settings(&AppSettings)` (threads → `LimiterHandle::set_threads`; accounts_dir/output_dir đổi → chỉ có hiệu lực cho job mới — ruling; ghi log).
- [ ] **Step 1: RED** — parse map đủ/thiếu/sai; round-trip; `apply_settings` đổi threads thấy `effective()` đổi.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(core,engine): AppSettings tu bang settings, apply_settings`.

---

### Task 6: `lvc-engine::resume` — tiếp tục từ conversation_id + requeue khi khởi động

**Files:** Create `crates/lvc-engine/src/resume.rs`; Modify `generate.rs`, lvc-browser trait (`BrowserDriver::poll_and_download` đã có — dùng lại), `driver.rs` (`FakeStep::Poll` đủ).

```rust
impl Engine {
    pub async fn resume(&self, task_id: &str, hooks, token) -> Result<GenerateOutcome, EngineError>; // row phải có conversation_id + account; mở phiên tài khoản đó (lease), goto /chat/<conv> KHÔNG gửi gì (Python resume_video 1787-1842: goto conv + assert_logged_in + poll), poll_and_download với timeout còn lại (deadline_at nếu có, else video_timeout), claim khi Ok
    pub async fn requeue_on_start(&self, now: f64) -> RequeueReport { requeued: Vec<id>, resumable: Vec<id> }; // TaskRepo::requeue_orphans + orphans_with_conversation
}
```
- [ ] **Step 1: RED** — FakeDriver: resume task có conv → gọi launch + poll (không submit), row completed; task không conv → `EngineError::NotResumable`; `requeue_on_start` đổi trạng thái đúng.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(engine): resume tu conversation_id, requeue khi khoi dong`.

---

### Task 7: `lvc-engine::dag` — chạy đồ thị

**Files:** Create `crates/lvc-engine/src/dag.rs`.

```rust
pub struct RunHandle { pub run_id: String, token: CancellationToken, join: JoinHandle<()> }
pub struct NodeOutputs { texts: HashMap<node_id, Vec<Scene>>, images: HashMap<node_id, Vec<PathBuf>>, videos: HashMap<node_id, Vec<(u32 /*scene_index*/, VideoArtifact)>> }
impl Engine {
    pub async fn run_graph(self: &Arc<Self>, workflow_id: &str, graph: &Graph, hooks_factory: …) -> Result<RunHandle, EngineError>;
    // 1. validate + plan (lvc-core) → lỗi trả ngay (không tạo run)
    // 2. RunRepo::create(run_id); emit NodeState(Pending) cho mọi node
    // 3. theo thứ tự steps: Prompt → texts; ImageUpload → kiểm file tồn tại → images; Video → tạo N TaskRow (run_id, node_id, scene_index, batch_id = run_id) rồi `JoinSet` chạy `generate_video_cancellable` cho từng cảnh (limiter lo song song), gom `(scene_index, artifact)`; lỗi 1 cảnh → cảnh đó failed, các cảnh khác tiếp tục; NodeState(Running/Done{ok,failed}); Save → copy/move mp4 sang out_dir (tên `<run_id>_<node>_<scene_index>.mp4`), emit NodeState(Done)
    // 4. RunDone; RunRepo::set_status(completed|failed|cancelled)
    pub fn cancel_run(&self, run_id) -> bool   // huỷ token + cancel mọi task của run
}
```
- [ ] **Step 1: RED** — FakeDriver kịch bản 3 video: 3 TaskRow, 3 launch/submit/poll (2 tài khoản, limiter threads 2 → tối đa 2 đồng thời), Save tạo 3 file; 1 cảnh Fail → run `completed_with_errors`? — ruling: run `failed` nếu ≥1 cảnh failed nhưng vẫn Save các cảnh ok; huỷ giữa chừng → các task còn lại `cancelled`, run `cancelled`; đồ thị lỗi validate → Err không tạo run; sự kiện NodeState đúng thứ tự.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(engine): DAG executor — chay do thi, fan-out video, barrier, huy run`.

---

### Task 8: `lvc-app` scaffold Tauri 2 + state + paths + bootstrap

**Files:** Create `crates/lvc-app/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,icons/*,src/{main.rs,lib.rs,state.rs,paths.rs,bootstrap.rs,events.rs}}`; Modify workspace `Cargo.toml` (member; `tauri = { version = "2", features = [] }`, `tauri-build`, `tauri-plugin-dialog`, `tauri-plugin-shell`), `.gitignore` (+`app/node_modules`, `app/dist`, `crates/lvc-app/gen`), `scripts/check.ps1` (+`pnpm -C app typecheck && pnpm -C app test && pnpm -C app build` khi `app/` tồn tại — có thể bỏ qua bằng `-SkipFrontend`).

- `paths.rs`: `app_data_dir()` (tauri `path().app_data_dir()`) → `lvc.db`, `logs/`, `downloads/` mặc định; `bootstrap.rs`: mở DB + migrate, `SettingsRepo` → `AppSettings`, `Engine::new(ChromiumDriver, …)`, `requeue_on_start`, spawn bridge `EngineEvents → app.emit("engine://event")`; `state.rs`: `AppState { engine: Arc<Engine<ChromiumDriver>>, db: Db, settings: RwLock<AppSettings>, runs: DashMap<run_id, RunHandle> }`; `lib.rs`: `run()` builder với plugins + `manage(state)` + `invoke_handler` (rỗng ở T8, T9-10 thêm); `tauri.conf.json`: `identifier "vn.lvcmedia.seedance"`, `productName "LVC Seedance"`, window 1400×900, `build.frontendDist "../../app/dist"`, `devUrl "http://localhost:5173"`, `beforeDevCommand "pnpm -C app dev"`, `beforeBuildCommand "pnpm -C app build"`; capabilities: `core:default`, `dialog:default`, `shell:allow-open`.
- Log: `tracing_subscriber` với policy (constraint) ghi ra `logs/app.log` (rolling theo ngày, `tracing-appender`) + stderr khi dev.
- [ ] **Step 1:** `cargo build -p lvc-app` xanh với frontend placeholder `app/dist/index.html` (T11 thay); unit test `paths`/`bootstrap` với DB tạm (không Tauri runtime: tách hàm thuần `open_db(path)`/`build_engine(...)`).
- [ ] **Step 2: Commit** — `feat(app): scaffold Tauri 2 — state, paths, bootstrap, event bridge, log policy`.

---

### Task 9: `lvc-app` commands — workflow/run/tasks/settings

**Files:** Create `crates/lvc-app/src/commands/{workflow.rs,tasks.rs,settings.rs}`; Modify `lib.rs`.

IPC contract (tất cả `#[tauri::command] async fn … -> Result<T, IpcError>` với `IpcError { code: String, message: String }` `Serialize`; mọi kiểu `Serialize` + `Deserialize` + `#[serde(rename_all = "camelCase")]`):
- `workflow_validate(graph: Graph) -> Result<PlanSummary { video_total, warnings }, IpcError{code:"GRAPH_INVALID", details: Vec<GraphError>}>`
- `workflow_save(id?, name, graph) -> WorkflowMeta`, `workflow_list() -> Vec<WorkflowMeta>`, `workflow_get(id) -> (WorkflowMeta, Graph)`, `workflow_delete(id)`
- `run_start(workflow_id, graph) -> RunInfo { run_id }` (validate lại), `run_cancel(run_id) -> bool`, `run_list(limit) -> Vec<RunRow>`
- `task_list(run_id?, limit) -> Vec<TaskDto>` (không có prompt đầy đủ — `prompt_preview` 80 ký tự), `task_cancel(id)`, `task_resume(id)`, `task_open_file(id)` (shell open thư mục chứa)
- `settings_get() -> AppSettings`, `settings_set(AppSettings) -> Vec<String> warnings` (ghi DB + `apply_settings`)
- `pick_directory() -> Option<String>`, `pick_files(images: bool) -> Vec<String>` (dialog plugin)
- [ ] **Step 1: RED** — test hàm lõi của mỗi command tách khỏi macro (nhận `&AppState`-like struct) với FakeDriver engine + DB tạm: `workflow_validate` trả lỗi có cấu trúc; `run_start` tạo run + tasks; `run_cancel`; `settings_set` áp threads; **fixture JSON**: serialize `Graph`, `TaskDto`, `EngineEvent`, `AppSettings`, `IpcError` ra `app/src/ipc/fixtures/*.json` (test ghi file khi `LVC_WRITE_FIXTURES=1`, mặc định so sánh bằng).
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(app): commands workflow/run/tasks/settings + fixture IPC`.

---

### Task 10: `lvc-app` commands — tài khoản

**Files:** Create `crates/lvc-app/src/commands/accounts.rs`; Modify lvc-engine (`AccountService`: `list_with_status(now)` gộp `AccountRow` + trạng thái tính (`cooling/rate_limited/quota_blocked/used_today/limit` như Python `list_accounts` 300-330), `import_cookies(name, text, proxy) -> ImportReport` (R11: dùng `ChromiumDriver` headless launch trên `accounts_dir/<name>` mới, `add_cookies`, goto `/chat`, `assert_logged_in`; fail → xoá thư mục + Err `COOKIE_NOT_LOGGED_IN`), `verify(name) -> SessionProbe` (launch headed? — ruling: headless probe chỉ đọc; nếu `logged_out` → `set_login_ok(false)`), `set_scheduling`, `set_proxy` (validate `parse_proxy`), `delete(name)` (xoá thư mục profile — hỏi xác nhận ở UI), `import_legacy(python_dir) -> ImportReport` (R12; `import_legacy` P0 trên bản copy tạm read-only)).
- Commands: `account_list`, `account_import_cookies(name, cookie_text, proxy)`, `account_verify(name)`, `account_set_scheduling(name, on)`, `account_set_proxy(name, proxy)`, `account_delete(name)`, `legacy_import(python_dir)`.
- [ ] **Step 1: RED** — FakeDriver cho `import_cookies` (probe_state LoggedIn → thư mục còn + row; LoggedOut → thư mục xoá + lỗi); `list_with_status` tính đúng cờ; `set_proxy` sai → lỗi `INVALID_PROXY` (message Python); `legacy_import` với DB fixture P0.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(app,engine): tai khoan — list/import cookie/verify/proxy/lich/xoa, nhap du lieu Python`.

---

### Task 11: Frontend scaffold + IPC types + adapter Graph

**Files:** Create `app/` (Vite React TS, Tailwind, `@xyflow/react`, zustand, `@tauri-apps/api`, `@tauri-apps/plugin-dialog`, vitest), `app/src/ipc/{types.ts,commands.ts,events.ts}`, `app/src/graph/{adapter.ts,ports.ts}`, `app/tests/{ipc_fixtures.test.ts, adapter.test.ts}`; `pnpm-lock.yaml` commit; `app/package.json` scripts `dev/build/typecheck/test`.

- `types.ts` mirror Rust (camelCase) — test đọc `app/src/ipc/fixtures/*.json` (T9) và parse bằng zod schema → lệch = fail.
- `adapter.ts`: `toGraph(nodes: RFNode[], edges: RFEdge[]) -> Graph` và `fromGraph(g) -> {nodes, edges}`; `ports.ts`: bảng cổng/kiểu/màu theo `NodeKind` (giống Rust `inputs()/outputs()` — ghim bằng fixture `ports.json` xuất từ Rust test).
- [ ] **Step 1:** vitest RED cho adapter/fixtures → implement → GREEN; `pnpm typecheck && pnpm build` xanh; `cargo tauri dev` mở cửa sổ trắng có chữ "LVC Seedance" (kiểm tay, ghi report kèm screenshot).
- [ ] **Step 2: Commit** — `feat(app-ui): scaffold Vite+React+ReactFlow, IPC types ghim fixture, adapter Graph`.

---

### Task 12: Màn Workflow — canvas, palette, node, chạy/huỷ, tiến độ

**Files:** `app/src/pages/Workflow.tsx`, `nodes/{PromptNode,ImageUploadNode,VideoNode,SaveNode}.tsx`, `components/{Palette,RunBar,ProgressPanel,NodeStatusBadge}.tsx`, `store/{graph.ts,run.ts}`.

- Palette kéo thả 4 node P3 (+ 5 node P4 mờ). Node Prompt: textarea nhiều dòng (định dạng TXT P0, gợi ý `ratio=16:9|dur=10|…`), đếm số cảnh; ImageUpload: nút chọn ảnh (dialog) + thumbnail tên file; Video: chọn mode/model/tỉ lệ/thời lượng (theo `allowed_durations(mode, model)` — bảng tĩnh mirror), logo, tài khoản (dropdown từ `account_list` hoặc tự chọn); Save: chọn thư mục. Cổng có màu theo kiểu; nối sai kiểu bị từ chối ngay (adapter `isValidConnection`).
- RunBar: Kiểm tra (gọi `workflow_validate`, hiện lỗi theo node), Chạy (`run_start`), Huỷ (`run_cancel`), Lưu/Mở workflow (`workflow_save/list/get`).
- ProgressPanel: nghe `engine://event`; mỗi cảnh một dòng: `#index · tài khoản · stage · elapsed`; NodeStatusBadge trên node (pending/running/done n/m/failed).
- Store: zustand `graph` (nodes/edges/selected, undo 20 bước), `run` (runId, events, per-task state).
- [ ] **Step 1:** vitest cho reducer sự kiện (`applyEvent(state, EngineEvent)`), `allowedDurations` mirror (ghim fixture từ Rust), `isValidConnection`. Kiểm tay: tạo 3 node, nối, Kiểm tra OK/lỗi, Chạy với **FakeDriver**? — không có trong app thật; ruling: thêm setting ẩn `LVC_FAKE_DRIVER=1` (env khi `cargo tauri dev`) để `bootstrap` dùng `FakeDriver` kịch bản "mọi submit Ok, poll Ok sau 3 s" → luồng UI thử được không tốn credit. Ghi report + screenshot.
- [ ] **Step 2: Commit** — `feat(app-ui): man Workflow — canvas, node, chay/huy, tien do; FakeDriver dev mode`.

---

### Task 13: Màn Tác vụ + Cài đặt

**Files:** `app/src/pages/{Tasks,Settings}.tsx`, `store/{tasks,settings}.ts`.

- Tác vụ: bảng (thời gian, run, cảnh, tài khoản, mode, trạng thái/failure_code, conv id, thời lượng), lọc theo run/trạng thái, nút Mở thư mục, Tiếp tục (`task_resume` khi có conv), Huỷ; tự cập nhật qua event + `task_list` khi mở.
- Cài đặt: threads (1..100, áp ngay), thư mục tài khoản, thư mục đầu ra, silent_mode, mode_fallback, mặc định mode/model/tỉ lệ/thời lượng; nút "Nhập dữ liệu từ bản Python" (chọn thư mục → `legacy_import` → báo cáo).
- [ ] **Step 1:** vitest cho `formatTask` (mã lỗi → nhãn Việt) và validate form; kiểm tay với FakeDriver mode.
- [ ] **Step 2: Commit** — `feat(app-ui): man Tac vu + Cai dat`.

---

### Task 14: Màn Tài khoản

**Files:** `app/src/pages/Accounts.tsx`, `components/{AccountRow,CookieImport,ProxyEditor}.tsx`, `store/accounts.ts`.

- Bảng: tên, đăng nhập (ok/hết/chưa rõ), lịch (toggle), proxy (sửa inline, validate lỗi Python), dùng hôm nay/hạn mức theo mode, cooldown/rate/quota còn (phút), nút Kiểm tra (verify → cập nhật), Xoá (xác nhận 2 bước).
- CookieImport: tên tài khoản + dán nội dung cookies.txt (hoặc chọn file — đọc ở Rust qua đường dẫn) + proxy tuỳ chọn → `account_import_cookies` → tiến trình (đang mở Chrome headless…) → kết quả; không hiển thị giá trị cookie.
- [ ] **Step 1:** vitest cho `accountStatusLabel`, form validate; kiểm tay: nhập cookie giả → lỗi `COOKIE_NOT_LOGGED_IN` hiện đúng (FakeDriver LoggedOut), toggle lịch/proxy ghi DB.
- [ ] **Step 2: Commit** — `feat(app-ui): man Tai khoan — nap cookie, lich, proxy, kiem tra, xoa`.

---

### Task 15: Đóng gói thử + gate frontend + tài liệu chạy

**Files:** `scripts/check.ps1` (frontend steps), `README.md` (cách chạy dev/build, chuẩn bị Chromium 1234, thư mục dữ liệu), `docs/superpowers/notes/2026-09-18-p3-run.md` (hướng dẫn thao tác 3 màn).
- `cargo tauri build` một lần → `crates/lvc-app/target/release/bundle/nsis/*.exe` (không ký) chạy được trên máy này; ghi kích thước; **không** commit binary.
- [ ] **Step 1:** gate xanh cả Rust + frontend; build NSIS thành công (ghi report).
- [ ] **Step 2: Commit** — `chore: gate frontend, README chay P3, build NSIS thu`.

---

### Task 16: LIVE — kéo thả 3 cảnh → 3 video (CẦN NGƯỜI DÙNG ĐỒNG Ý, ~6 credit)

- Controller hỏi: tài khoản (1 hay 2), threads, prompt 3 cảnh. Chạy `cargo tauri dev` (driver thật), người dùng hoặc controller thao tác UI: Prompt 3 dòng → Tạo video (fast/2.0/10 s) → Lưu; theo dõi ProgressPanel; kết quả 3 mp4 trong thư mục Lưu; Tác vụ hiện 3 dòng completed; đóng app giữa chừng 1 lần (sau khi có conv id) → mở lại → resume tự động (kiểm riêng nếu người dùng cho phép thêm 2 credit).
- ADR-R014 ghi số đo; screenshot xoá PII.

---

## Cố ý KHÔNG làm ở P3
Auto-login Google/Facebook + TOTP (P3b); node Tạo ảnh/Xoá logo/H.264/Ghép/Lấy ảnh URL (P4); license/ký/bundle chính thức (P5); e2e UI tự động; xoay proxy `rotate/random` (P3b cùng tab tài khoản nâng cao); auto_retry; captcha; đổi `Limits` từ UI.

## Nghiệm thu P3
- [ ] Gate xanh (Rust + frontend) không cần Dola.
- [ ] `cargo tauri dev` chạy, 4 màn dùng được với FakeDriver mode.
- [ ] Task 16: 3 cảnh → 3 video thật, ADR-R014.
- [ ] Bảng park P2 §3: mục nào P3 đã nhặt ghi rõ, còn lại chuyển P4/P5.
