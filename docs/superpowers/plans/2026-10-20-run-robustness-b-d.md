# Run robustness — Phần B (lưu/khôi phục) + D (xem lỗi) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (hoặc subagent-driven-development) để thực thi từng task. Steps dùng checkbox (`- [ ]`).

**Goal:** Tắt/mở app (hoặc dev build lại, hoặc đổi tab) → node đã xong vẫn hiện video, node lỗi vẫn đỏ + bấm xem được lý do; run dở KHÔNG bị ép fail oan, KHÔNG tự chạy tiếp.

**Architecture:** Thêm bảng `node_states` (một dòng/node/run) ghi inline-await trong scheduler `run_steps` (generic, đúng ADR-R014 — không match theo kind). Khôi phục preview video bằng **quét `downloads_dir`** theo tên tất định `{run_id}_{node_id}_*`. Run `running` lúc boot → đổi sang `interrupted` (không `failed`). UI: lệnh mới `run_node_states`/`run_node_outputs`, reconcile dựng lại `nodes`/`nodeVideos`, re-attach run gần nhất của workflow khi mở (nhớ `runId` qua localStorage). KHÔNG auto-resume.

**Tech Stack:** Rust (sqlx/sqlite, tokio), lvc-store/lvc-engine/lvc-app (Tauri 2), React + zustand (frontend).

**Spec:** `docs/superpowers/specs/2026-10-20-run-robustness-design.md`

## Global Constraints

- ADR-R014: `dag.rs` KHÔNG `match` theo node kind — persistence node_states phải generic ở scheduler (một phễu `emit_*`/vòng `run_steps`), không special-case kind.
- `lvc-core` THUẦN: không `sqlx`/`tokio`/`reqwest`/`chromiumoxide`/`tauri`. (Plan này không đụng lvc-core.)
- MSRV 1.85: KHÔNG let-chains. `clippy::pedantic` + `-D warnings` (chạy cả `cargo clippy -p lvc-app --features fake-driver`).
- Luật #3: KHÔNG log/IPC bí mật (cookie/token/prompt đầy đủ). node_states.error chỉ chứa thông điệp lỗi kỹ thuật đã có sẵn (vd "echo ref", "0 video") — KHÔNG được chứa prompt/secret; nếu `e.message` có thể mang prompt thì cắt, nhưng hiện các message là mô tả kỹ thuật.
- Migration mới đánh số tiếp: `0012_*.sql` (hiện cao nhất `0011_credential_usage.sql`).
- Fixtures IPC sinh từ Rust: sau khi thêm lệnh mới phải chạy `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures` rồi commit — KHÔNG sửa tay `app/src/ipc/fixtures/*.json`.
- Cổng trước khi báo xong: `powershell -ExecutionPolicy Bypass -File scripts/check.ps1`.
- Đừng sửa code gây rebuild khi người dùng đang có run live (hỏi trước).

## Review Focus

- **node_id chứa dấu gạch dưới** khi quét file: `{run_id}_{node_id}_{scene}.ext` nhập nhằng nếu tách bằng `_`. → `node_outputs` PHẢI nhận danh sách node_id đã biết (từ node_states) và khớp tiền tố `{run_id}_{node_id}_`, KHÔNG parse mù. Test với node_id = `im_scene_1`.
- **Run của workflow KHÁC** không được "rò" sang tab đang mở: `run_node_states`/reconcile chỉ áp cho đúng `runId`; re-attach chỉ lấy run có `workflow_id` khớp workflow đang mở. Test reconcile bỏ qua dto khác runId (đã có khuôn ở `reconcileTasksFromDb`).
- **downloads_dir không tồn tại / rỗng**: `node_outputs` trả map rỗng, không panic/không lỗi (app mới, chưa chạy gì). Test thư mục trống.
- **node_states ghi khi app bị kill giữa chừng**: ghi inline-await TRƯỚC khi node kế chạy → trạng thái cuối cùng của node đã xong luôn nằm trong DB. Test: sau khi 1 node done, dòng node_states của nó = `done` kể cả khi run bị hủy ngay sau.
- **Trạng thái cũ `failed` vs mới `interrupted`**: run đã `completed`/`failed`/`cancelled` KHÔNG bị đổi thành `interrupted` lúc boot (chỉ `running`). Test giữ nguyên run đã xong.

---

### Task 1: Bảng `node_states` + `NodeStateRepo` (lvc-store, thuần)

**Files:**
- Create: `crates/lvc-store/migrations/0012_node_states.sql`
- Create: `crates/lvc-store/src/node_states.rs`
- Modify: `crates/lvc-store/src/lib.rs` (thêm `mod node_states;` + re-export `NodeStateRepo`, `NodeStateRow`)
- Test: trong `node_states.rs` (`#[cfg(test)] mod tests`, dùng `Db::open_memory`)

**Interfaces:**
- Consumes: `crate::{Db, StoreError}`; khuôn mẫu `RunRepo` trong `workflows.rs` (dùng `sqlx::query`, `row.try_get`, `Db::pool()`).
- Produces:
  - `NodeStateRow { run_id: String, node_id: String, state: String, done: u32, total: u32, ok: u32, failed: u32, error: String, updated_at: f64 }` — `#[derive(Debug, Clone, Serialize, Deserialize)]` + `#[serde(rename_all = "camelCase")]` (wire khớp `NodeRunState` của frontend: `nodeId/state/done/total/ok/failed/error`).
  - `NodeStateRepo::new(db: &Db) -> Self`
  - `NodeStateRepo::upsert(&self, run_id: &str, node_id: &str, state: &str, counts: (u32,u32,u32,u32), error: Option<&str>, updated_at: f64) -> Result<(), StoreError>` (ON CONFLICT(run_id,node_id) DO UPDATE tất cả cột).
  - `NodeStateRepo::list_for_run(&self, run_id: &str) -> Result<Vec<NodeStateRow>, StoreError>`.

Schema (`0012_node_states.sql`):
```sql
-- Trang thai MUC NODE cua mot run (khac `tasks` chi co cho dola_video): MOI node moi run
-- deu co 1 dong, ghi inline trong scheduler (dag.rs run_steps) de khoi phuc khi mo lai app.
CREATE TABLE IF NOT EXISTS node_states (
  run_id     TEXT    NOT NULL,
  node_id    TEXT    NOT NULL,
  state      TEXT    NOT NULL,
  done       INTEGER NOT NULL DEFAULT 0,
  total      INTEGER NOT NULL DEFAULT 0,
  ok         INTEGER NOT NULL DEFAULT 0,
  failed     INTEGER NOT NULL DEFAULT 0,
  error      TEXT    NOT NULL DEFAULT '',
  updated_at REAL    NOT NULL DEFAULT 0,
  PRIMARY KEY (run_id, node_id)
);
CREATE INDEX IF NOT EXISTS idx_node_states_run ON node_states(run_id);
```

- [ ] **Step 1: Viết test thất bại** trong `node_states.rs`:
  - `upsert_roi_doc_lai_mot_dong`: `Db::open_memory` → `upsert("r1","im_scene_1","running",(0,3,0,0),None,1.0)` → `list_for_run("r1")` có 1 dòng, `node_id=="im_scene_1"`, `state=="running"`, `total==3`, `error==""`.
  - `upsert_de_len_trang_thai_moi`: upsert cùng (r1, n1) lần 2 với `state="done", counts=(3,3,3,0), error=None, updated_at=2.0` → vẫn 1 dòng, `state=="done"`, `done==3`.
  - `upsert_ghi_error_khi_failed`: `upsert("r1","v1","failed",(1,1,0,1),Some("echo ref"),3.0)` → dòng có `error=="echo ref"`.
  - `list_for_run_loc_dung_run`: hai run r1/r2 → `list_for_run("r1")` chỉ trả node của r1.
  - `node_state_row_serde_camelcase`: `serde_json::to_value(NodeStateRow{..})` có khóa `nodeId`, `runId`, không có `node_id`.
- [ ] **Step 2: Chạy test → FAIL** (`cargo test -p lvc-store node_states`) — Expected: không biên dịch (`NodeStateRepo` chưa có).
- [ ] **Step 3: Tạo migration + impl `NodeStateRepo`/`NodeStateRow`** theo khuôn `RunRepo` (workflows.rs). `upsert` dùng `INSERT ... ON CONFLICT(run_id,node_id) DO UPDATE SET state=excluded.state, done=..., error=...`. `u32`↔sqlite: bind `i64::from(x)`, đọc `try_get::<i64,_>` rồi `u32::try_from(..).unwrap_or(0)` (như chỗ khác trong store).
- [ ] **Step 4: Chạy test → PASS**. Expected: 5/5 pass.
- [ ] **Step 5: Commit** `feat(store): bang node_states + NodeStateRepo (luu trang thai muc node)`.

---

### Task 2: `RunStatus::Interrupted` + `RunRepo::interrupt_running` (lvc-store)

**Files:**
- Modify: `crates/lvc-store/src/workflows.rs` (thêm biến thể enum + method)
- Test: `workflows.rs` `mod tests`

**Interfaces:**
- Consumes: `RunStatus`, `RunRepo` (đã có).
- Produces:
  - `RunStatus::Interrupted` (wire string `"interrupted"`, thêm vào `as_str`/`parse`).
  - `RunRepo::interrupt_running(&self, now: f64, error: &str) -> Result<Vec<String>, StoreError>` — như `fail_running` nhưng đặt `status="interrupted"` thay vì `failed`. (Giữ `fail_running` nếu nơi khác còn dùng; nếu chỉ `requeue_on_start` dùng thì đổi call site ở Task 3.)

- [ ] **Step 1: Viết test thất bại**:
  - `interrupt_running_dat_trang_thai_interrupted`: tạo r1 `Running`, r2 `Completed` → `interrupt_running(9000.0,"RESTART")` → trả `["r1"]`; `get("r1").status == Interrupted`, `r1.error=="RESTART"`; `r2` vẫn `Completed`.
  - `run_status_interrupted_serde`: mở rộng `run_status_serde_wire_string_khop_as_str` để gồm `Interrupted` (serde value == `"interrupted"`).
  - `run_status_parse_interrupted`: `RunStatus::parse("interrupted") == Some(Interrupted)`.
- [ ] **Step 2: Chạy test → FAIL** (`cargo test -p lvc-store workflows`). Expected: `Interrupted` chưa tồn tại → không biên dịch.
- [ ] **Step 3: Thêm biến thể + method.** Cập nhật cả `as_str` và `parse`. `interrupt_running` copy `fail_running`, đổi `RunStatus::Failed` → `RunStatus::Interrupted`.
- [ ] **Step 4: Chạy test → PASS.**
- [ ] **Step 5: Commit** `feat(store): RunStatus::Interrupted + interrupt_running (run do khong bi fail oan)`.

---

### Task 3: Engine ghi node_states inline + boot đổi running→interrupted

**Files:**
- Modify: `crates/lvc-engine/src/dag.rs` (ghi node_states trong `run_steps` + queued + pending-terminal)
- Modify: `crates/lvc-engine/src/generate.rs` hoặc nơi `Engine` được dựng (thêm field `node_states: NodeStateRepo`, khởi tạo từ `self.db`/`Db`)
- Modify: `crates/lvc-engine/src/resume.rs` (`requeue_on_start` gọi `interrupt_running` thay vì `fail_running` cho `runs`)
- Modify: `crates/lvc-engine/src/lib.rs` nếu cần thêm method `Engine::node_outputs`
- Test: `dag.rs` `mod tests` (đã có hạ tầng fake graph + `Db::open_memory`)

**Interfaces:**
- Consumes: `lvc_store::{NodeStateRepo, NodeStateRow}` (Task 1), `RunRepo::interrupt_running` (Task 2), `self.cfg.downloads_dir`.
- Produces:
  - Engine ghi `node_states` tại MỌI chuyển trạng thái node (queued/running/done/failed/cancelled + pending-terminal), await trong `run_steps`/`run_graph_task` (thứ tự theo node đảm bảo).
  - `Engine::node_outputs(&self, run_id: &str, node_ids: &[String]) -> HashMap<String, String>` — với mỗi `node_id`, tìm file ĐẦU TIÊN trong `downloads_dir` có tên bắt đầu `{run_id}_{node_id}_`, trả `node_id -> path` (bỏ node không có file). Thuần đọc thư mục, không lỗi khi thư mục trống/không tồn tại.

- [ ] **Step 1: Viết test thất bại** (dùng khuôn test DAG hiện có — graph Prompt→Video→Save với fake driver, `Db::open_memory`):
  - `node_states_ghi_dung_trang_thai_cuoi`: chạy 1 graph 2 node tới `RunDone` → `NodeStateRepo::list_for_run(run_id)` có 1 dòng/node, node xong = `done`, `done==total`.
  - `node_states_queued_truoc_khi_chay`: xác nhận mỗi node có dòng (ít nhất đã từng `queued`→`done`), không node nào thiếu dòng.
  - `node_outputs_khop_tien_to_node_id`: tạo tay file `{run_id}_im_scene_1_0.png` + `{run_id}_other_0.mp4` trong downloads_dir giả → `node_outputs(run_id, &["im_scene_1".into()])` trả đúng 1 entry trỏ file png (không nhầm `other`).
  - `node_outputs_thu_muc_rong`: downloads_dir trống → trả map rỗng, không lỗi.
- [ ] **Step 2: Chạy test → FAIL** (`cargo test -p lvc-engine node_states`). Expected: method/ghi chưa có.
- [ ] **Step 3: Impl.**
  - Thêm `node_states: NodeStateRepo` vào struct Engine (khởi tạo cạnh `run_repo`). 
  - Trong `run_steps`: sau mỗi `emit_node_state*` cho node (running/done/failed), `self.node_states.upsert(&run_id, node_id, state, counts, err, now_unix()).await` (bỏ qua lỗi ghi bằng `if let Err(e) = ... { warn!(...) }` — ghi hỏng không được làm sập run). Với `queued`: lặp `graph.nodes` ở đầu `run_steps` (sau `start_run`) ghi `queued`. Với pending-terminal: sau `emit_pending_terminal`, lặp các node chưa `touched` ghi trạng thái cuối. **KHÔNG match kind** — chỉ lặp node.
  - `Engine::node_outputs`: `std::fs::read_dir(downloads_dir)` (ok_or rỗng), với mỗi node_id tìm `file_name().starts_with(&format!("{run_id}_{node_id}_"))`.
  - `resume.rs`: đổi `run_repo.fail_running(...)` → `run_repo.interrupt_running(...)` cho bảng `runs` (tasks giữ nguyên `fail_running`/`fail_orphans` của TaskRepo — cảnh dở của Dola vẫn fail để chạy lại được ở phần C).
- [ ] **Step 4: Chạy test → PASS** + `cargo test -p lvc-engine` (toàn crate) xanh.
- [ ] **Step 5: Commit** `feat(engine): ghi node_states inline + boot interrupt run do + node_outputs (quet dia)`.

---

### Task 4: Lệnh IPC `run_node_states` + `run_node_outputs` (lvc-app)

**Files:**
- Modify: `crates/lvc-app/src/commands/workflow.rs` (thêm 2 lệnh cạnh `run_list`)
- Modify: `crates/lvc-app/src/commands/mod.rs`/`crates/lvc-app/src/lib.rs` (đăng ký `invoke_handler!`)
- Test: `workflow.rs` `mod tests` (khuôn `run_list` đã có) + regen fixtures

**Interfaces:**
- Consumes: `NodeStateRepo::list_for_run` (qua Engine/state), `Engine::node_outputs`.
- Produces:
  - `run_node_states(run_id: String) -> Result<Vec<NodeStateRow>, CommandError>` (trả thẳng `NodeStateRow`, camelCase).
  - `run_node_outputs(run_id: String) -> Result<HashMap<String,String>, CommandError>` — gọi `run_node_states` lấy danh sách node_id rồi `Engine::node_outputs`.

- [ ] **Step 1: Viết test thất bại**: `run_node_states_tra_dong_da_ghi` — dựng Ctx test (như `run_list` test), engine chạy 1 graph tới xong, `run_node_states_impl(run_id)` trả đúng số node với state cuối. `run_node_outputs_impl` trả map (rỗng nếu fake driver không ghi file — chấp nhận, hoặc tạo file giả).
- [ ] **Step 2: Chạy test → FAIL** (`cargo test -p lvc-app workflow`).
- [ ] **Step 3: Impl 2 lệnh** theo khuôn `run_list_impl`/`run_list`. Đăng ký trong `invoke_handler!` (lib.rs).
- [ ] **Step 4: Chạy test → PASS**; rồi `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures` sinh fixture 2 lệnh mới.
- [ ] **Step 5: Commit** `feat(app): lenh run_node_states + run_node_outputs (+ fixtures)` (gồm fixtures JSON mới).

---

### Task 5: Frontend — khôi phục node state + preview + xem lỗi sau restart (B + D)

**Files:**
- Modify: `app/src/store/run.tsx` (thêm `reconcileNodeStatesFromDb` + mở rộng `reconcile`)
- Modify: `app/src/ipc/commands.ts` (bind `runNodeStates`/`runNodeOutputs`)
- Modify: `app/src/ipc/types.ts` (type `NodeStateDto` khớp camelCase)
- Modify: component boot (Workflow/tab mở) — nhớ `runId` gần nhất theo workflow (localStorage) + gọi reconcile khi mở
- Modify: `app/src/components/NodeLogPanel.tsx` (render từ `nodes[id]` đã khôi phục, không chỉ khi còn live)
- Test: `app/src/store/run.test.ts` (đã có khuôn test cho `reconcileTasksFromDb`)

**Interfaces:**
- Consumes: lệnh `runNodeStates(runId): Promise<NodeStateDto[]>`, `runNodeOutputs(runId): Promise<Record<string,string>>` (Task 4). `NodeStateDto` = `{ runId, nodeId, state, done, total, ok, failed, error }`.
- Produces: `reconcileNodeStatesFromDb(state: RunState, rows: readonly NodeStateDto[]): RunState` — set `nodes[nodeId]` từ mỗi row (bỏ qua row khác `state.runId` khi `state.runId!=null`); giữ nguyên nếu không đổi (tham chiếu ổn định như `reconcileTasksFromDb`).

- [ ] **Step 1: Viết test thất bại** (`run.test.ts`):
  - `reconcileNodeStatesFromDb dung lai nodes`: state có `runId="r1"`, rows 2 node (1 done, 1 failed+error) → `nodes` có 2 entry đúng `state`/`error`.
  - `reconcileNodeStatesFromDb bo qua run khac`: `state.runId="r1"`, row `runId="r2"` → không thêm node.
  - `reconcileNodeStatesFromDb runId null thi nhan`: `state.runId=null` + rows → adopt (set nodes). (Khớp hành vi `reconcileFromRunRow`.)
- [ ] **Step 2: Chạy test → FAIL** (`pnpm --filter app test run.test` hoặc lệnh test FE của repo). Expected: `reconcileNodeStatesFromDb` chưa export.
- [ ] **Step 3: Impl `reconcileNodeStatesFromDb`** (thuần, khuôn `reconcileTasksFromDb`). Mở rộng `reconcile` store action nhận thêm node_states (hoặc thêm action `reconcileNodes`). Bind lệnh IPC + type. Boot: khi mở workflow, đọc `localStorage` key `lvc:lastRun:<workflowId>`; nếu có → `runNodeStates`+`runNodeOutputs` → `beginRun(runId)` (chỉ set runId, KHÔNG runStart) + `reconcileNodeStatesFromDb` + set `nodeVideos`. Ghi `localStorage` key này trong `beginRun`/khi `run_start` trả runId. `NodeLogPanel`: bỏ điều kiện chỉ-khi-live, đọc `nodes[selectedId]` từ store (đã có sau reconcile).
- [ ] **Step 4: Chạy test → PASS** + typecheck.
- [ ] **Step 5: Commit** `feat(ui): khoi phuc node state + preview + xem loi sau restart (B+D)`.

---

## Nghiệm thu (sau Task 5, xin phép user trước live)

Chạy reel vài cảnh → tắt app (hoặc để nó build lại) → mở lại workflow: cảnh xong hiện video, cảnh lỗi đỏ, bấm node lỗi thấy đúng lý do; run hiện `interrupted` (không mất, không tự chạy tiếp). Đây là tiền đề cho phần C ("Chạy tiếp"/"Chạy lại node lỗi").
