# Run robustness — Phần C (chạy lại node lỗi + tự ghép) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps dùng checkbox.

**Goal:** Reel có vài cảnh lỗi → bấm "Chạy lại node lỗi" → CHỈ node lỗi + node phía sau (concat/save) chạy lại; cảnh đã xong được tái dùng (KHÔNG tốn credit lại) → ra reel hoàn chỉnh.

**Architecture:** Lưu `Outputs` (JSON) của mỗi node khi xong (cột mới `node_states.outputs`, ghi server-side, KHÔNG ra IPC). Lệnh `run_retry_failed(run_id, graph)` tính tập node chạy lại = node `failed` ∪ con cháu của chúng (hàm thuần `lvc_core::graph::descendants`); node đã xong ngoài tập đó được SEED outputs (deserialize) + bỏ qua thực thi. Reuse scheduler qua một biến thể `run_graph_seeded` (run_graph thường = seed rỗng). Retry là một run MỚI (run_id mới) tái dùng file cũ qua đường dẫn tuyệt đối trong Outputs đã seed.

**Tech Stack:** Rust (lvc-store/lvc-core/lvc-engine/lvc-app), React+zustand.

**Spec:** `docs/superpowers/specs/2026-10-20-run-robustness-design.md` (§C). Dựa trên phần B đã xong (bảng `node_states`, `NodeStateRepo`, `Engine::node_outputs`, khôi phục UI).

## Global Constraints

- ADR-R014: scheduler KHÔNG match theo kind; seed/skip làm generic theo node_id. Logic chọn tập chạy lại THUẦN ở `lvc-core` (không I/O).
- `lvc-core` thuần. MSRV 1.85 (không let-chains). clippy pedantic -D warnings ×2. fmt.
- KHÔNG re-submit cảnh đã xong (tốn credit) — đó là mục tiêu chính của C.
- Fixtures sinh từ Rust; migration mới = `0013_*.sql`.
- Cổng: `powershell -ExecutionPolicy Bypass -File scripts/check.ps1` (chạy với `CARGO_TARGET_DIR=D:/LVCMedia/_lvcrt` để không đụng tauri dev).
- Dùng target `D:/LVCMedia/_lvcrt` cho test/clippy.

## Review Focus

- **Node lỗi có cảnh con một phần (museai video 2/3 cảnh ok)**: retry phải chạy lại CẢ node (không có cơ chế seed từng cảnh con trong một node museai) — chấp nhận re-credit node đó, nhưng KHÔNG node khác. Test: node failed → trong rerun set; node done → seeded.
- **Con cháu của node lỗi phải chạy lại** dù bản thân từng "done" (vd concat đã chạy với input thiếu): `descendants` phải gồm toàn bộ hậu duệ. Test concat downstream của video lỗi → rerun.
- **Seed outputs sai đường dẫn**: seed deserialize từ JSON đã lưu; file vẫn ở downloads_dir (đường dẫn tuyệt đối). Nếu file bị xoá tay → node phía sau có thể lỗi khi đọc; chấp nhận (ngoài phạm vi). Test seed round-trips đúng Outputs.
- **Graph đổi giữa chừng** (user sửa canvas rồi retry): seed theo node_id; node không còn → bỏ seed; node mới → chạy tươi. KHÔNG được panic. Test node_id trong node_states không có trong graph → bỏ qua.
- **Không có node lỗi**: retry → rerun rỗng → chỉ seed + reassembly? Nếu rerun rỗng thì không có gì để làm; lệnh trả lỗi rõ ("không có node lỗi để chạy lại") hoặc no-op. Quyết định ở Task 4.

---

### Task 1: `node_states.outputs` (JSON) — lưu/đọc Outputs node đã xong (lvc-store)

**Files:**
- Create: `crates/lvc-store/migrations/0013_node_states_outputs.sql`
- Modify: `crates/lvc-store/src/node_states.rs`
- Test: trong `node_states.rs`

**Interfaces:**
- Consumes: `NodeStateRepo` (phần B).
- Produces:
  - `NodeStateRepo::set_outputs(&self, run_id: &str, node_id: &str, outputs_json: &str) -> Result<(), StoreError>` — UPDATE cột `outputs` của dòng `(run_id,node_id)` (chỉ gọi ở trạng thái done).
  - `NodeStateRepo::outputs_for_run(&self, run_id: &str) -> Result<Vec<(String, String)>, StoreError>` — trả `(node_id, outputs_json)` cho các dòng `outputs != ''`.
  - `NodeStateRow` GIỮ NGUYÊN (cột `outputs` KHÔNG vào row → không ra IPC/fixture, giữ payload UI gọn).

SQL: `ALTER TABLE node_states ADD COLUMN outputs TEXT NOT NULL DEFAULT '';`

- [ ] Step 1: Test đỏ: `set_outputs_roi_outputs_for_run_tra_dung` — upsert dòng done, `set_outputs("r1","n1","{\"out\":[]}")`, `outputs_for_run("r1")` trả `[("n1","{\"out\":[]}")]`; dòng chưa set outputs không xuất hiện. `set_outputs_dong_khong_ton_tai_khong_loi` (UPDATE 0 dòng → Ok).
- [ ] Step 2: Chạy → FAIL (`cargo test -p lvc-store node_states`).
- [ ] Step 3: Migration + 2 method. `outputs_for_run`: `SELECT node_id, outputs FROM node_states WHERE run_id=?1 AND outputs<>''`.
- [ ] Step 4: Chạy → PASS; `cargo test -p lvc-store` toàn bộ xanh.
- [ ] Step 5: Commit `feat(store): node_states.outputs (luu Outputs node da xong cho retry)`.

---

### Task 2: `lvc_core::graph::descendants` — tập hậu duệ (thuần)

**Files:**
- Modify: `crates/lvc-core/src/graph/validate.rs` (hoặc file mới `crates/lvc-core/src/graph/retry.rs` + khai báo trong `mod.rs`)
- Test: cùng file

**Interfaces:**
- Produces: `pub fn descendants(graph: &Graph, seeds: &BTreeSet<String>) -> BTreeSet<String>` — mọi node với tới được TỪ `seeds` theo cạnh `edge.from -> edge.to` (KHÔNG gồm seeds trừ khi có chu trình; graph đã validate không chu trình nên gồm đúng hậu duệ; trả set GỒM cả seeds để caller dùng trực tiếp làm "rerun set"). Quyết định: trả `seeds ∪ reachable` để `rerun = descendants(graph, failed)` dùng thẳng.

- [ ] Step 1: Test đỏ:
  - `descendants_gom_hau_due_va_seed`: graph A→B→C, D rời → `descendants({A}) == {A,B,C}` (không D).
  - `descendants_nhieu_seed`: A→C, B→C → `descendants({B}) == {B,C}`.
  - `descendants_seed_khong_co_canh`: node lá → `== {no đó}`.
- [ ] Step 2: Chạy → FAIL (`cargo test -p lvc-core descendants`).
- [ ] Step 3: Impl BFS/DFS qua `graph.edges` (map from→[to]); dùng `BTreeSet` cho ổn định.
- [ ] Step 4: PASS; `cargo test -p lvc-core` xanh.
- [ ] Step 5: Commit `feat(core): graph::descendants (tap hau due cho chay lai node loi)`.

---

### Task 3: Engine — ghi Outputs khi done + `run_graph_seeded` (seed + skip)

**Files:**
- Modify: `crates/lvc-engine/src/dag.rs` (persist outputs ở done; `RunJob` thêm `seed_outputs`/`skip`; `run_steps` seed+skip; `run_graph_seeded` công khai; `run_graph` gọi lại với seed rỗng)
- Test: `dag.rs`

**Interfaces:**
- Consumes: `NodeStateRepo::set_outputs` (T1), `descendants` (T2), `Outputs` (serde).
- Produces:
  - Ghi `self.node_states.set_outputs(run_id, node_id, &serde_json::to_string(&res.outputs))` tại nhánh done (không failed) trong `run_steps`.
  - `Engine::run_graph_seeded(workflow_id, graph, hooks_factory, seed_outputs: HashMap<String,Outputs>, skip: HashSet<String>) -> Result<RunHandle, EngineError>`. `run_graph` = `run_graph_seeded(.., empty, empty)`.
  - `run_steps`: trước vòng lặp, `outputs.extend(seed_outputs)`. Trong vòng lặp, nếu `skip.contains(node_id)`: emit `done` (counts từ node_states cũ hoặc `(1,1,1,0)` tối giản) + persist + `emit_preview_if_media(&seeded outputs)` + `touched.insert` + `continue` (KHÔNG chạy exec → không tốn credit). Node không skip chạy như cũ (gather_inputs thấy seed outputs của upstream).

- [ ] Step 1: Test đỏ (fake graph pr→vid→sv, cho vid "fail" 1 lần qua FakeStep lỗi): chạy lần đầu → vid failed. Rồi gọi `run_graph_seeded` với `seed_outputs={pr: <outputs>}`, `skip={pr}`, happy steps cho vid → assert: pr KHÔNG chạy lại (driver calls không có cho pr / hoặc đếm), vid+sv chạy, run completed. (Đo bằng FakeDriver calls hoặc NodeState events: pr chỉ có `done` tức thì, không `running`.)
  - `run_graph_seeded_bo_qua_node_skip`: skip 1 node → NodeState của nó là done ngay, exec không gọi.
- [ ] Step 2: Chạy → FAIL.
- [ ] Step 3: Impl. Lưu ý thứ tự: seed trước, skip trong loop. `set_outputs` bọc `if let Err` warn.
- [ ] Step 4: PASS; `cargo test -p lvc-engine --lib` xanh; clippy -p lvc-engine sạch.
- [ ] Step 5: Commit `feat(engine): run_graph_seeded (seed outputs + bo qua node da xong) + luu Outputs khi done`.

---

### Task 4: Lệnh `run_retry_failed(run_id, graph)` (lvc-app)

**Files:**
- Modify: `crates/lvc-app/src/commands/workflow.rs` + đăng ký `lib.rs` + fixtures nếu cần
- Test: `workflow.rs`

**Interfaces:**
- Consumes: `NodeStateRepo::{list_for_run, outputs_for_run}`, `descendants`, `Engine::run_graph_seeded`.
- Produces:
  - `run_retry_failed_impl(ctx, run_id, graph) -> Result<RunInfo, IpcError>`:
    1. `states = list_for_run(run_id)`; `failed = {node_id: state=="failed"}`. Rỗng → `IpcError` mã `NO_FAILED_NODES` (UI không hiện nút thì không tới đây, nhưng phòng thủ).
    2. `rerun = descendants(graph, failed)`.
    3. `outputs_map = outputs_for_run(run_id)` → deserialize từng JSON thành `Outputs`; `seed = {node_id -> Outputs}` cho node CÓ trong graph VÀ KHÔNG thuộc `rerun`. `skip = seed.keys()`.
    4. `engine.run_graph_seeded(workflow_id_của_graph?, migrate(graph), noop_hooks, seed, skip)`. workflow_id: lấy từ run cũ (`run_repo.get(run_id).workflow_id`) để run mới cùng workflow.
    5. Trả `RunInfo{run_id: new}`.
  - `#[tauri::command] run_retry_failed(state, run_id, graph)`.
- [ ] Step 1: Test đỏ `run_retry_failed_chi_chay_lai_node_loi`: chạy graph 3 node với vid lỗi → retry → run mới completed; node_states run mới: pr `done` (skip, không chạy lại), vid `done`, sv `done`; (xác nhận pr không tốn "công" qua đếm driver calls trước/sau hoặc qua event pr chỉ done).
- [ ] Step 2: Chạy → FAIL.
- [ ] Step 3: Impl + đăng ký invoke_handler. Fixtures: `RunInfo` đã có fixture; không cần mới.
- [ ] Step 4: PASS; `cargo test -p lvc-app --features fake-driver commands::workflow` xanh; clippy fake-driver sạch.
- [ ] Step 5: Commit `feat(app): lenh run_retry_failed (chay lai node loi + tu ghep)`.

---

### Task 5: Frontend — nút "Chạy lại node lỗi" (RunBar)

**Files:**
- Modify: `app/src/components/RunBar.tsx` (nút hiện khi run có node `failed`)
- Modify: `app/src/ipc/commands.ts` (`runRetryFailed`)
- Test: nếu tách được logic thuần "có node failed không" → test; nếu không, dựa typecheck + build.

**Interfaces:**
- Consumes: `runRetryFailed(runId, graph)` → `RunInfo`.
- Produces: nút trong RunBar: hiện khi `status !== null && Object.values(nodes).some(n => n.state === "failed")`. Bấm → `runRetryFailed(runId, toGraph())` → `clearRun()` (giữ runId cũ trong history) → `beginRun(info.runId)` + `registerRunOwner` (như handleRun). KHÔNG hỏi lại (an toàn — chỉ chạy lại node lỗi).
- [ ] Step 1: Test đỏ (nếu tách hàm `hasFailedNode(nodes)` thuần): `hasFailedNode` true khi có node failed. (Nếu không tách, bỏ test, dựa build.)
- [ ] Step 2: Chạy → FAIL.
- [ ] Step 3: Impl nút + binding + handler (mirror `handleRun`).
- [ ] Step 4: `pnpm test` + `pnpm typecheck` + `pnpm build` xanh.
- [ ] Step 5: Commit `feat(ui): nut Chay lai node loi (RunBar)`.

---

## Nghiệm thu (sau Task 5, xin phép user trước live)
Reel 14 cảnh, ép 1-2 cảnh lỗi → bấm "Chạy lại node lỗi" → chỉ cảnh đó + concat/save chạy lại, cảnh cũ giữ nguyên (không tốn credit lại), ra reel hoàn chỉnh.
