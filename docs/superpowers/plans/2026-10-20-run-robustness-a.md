# Run robustness — Phần A (chạy song song N node) — Implementation Plan

> REQUIRED SUB-SKILL: superpowers:executing-plans. Steps dùng checkbox.

**Goal:** Chạy nhiều node đồng thời (tối đa N, N = setting `node_threads`, mặc định 1) thay vì tuần tự — tiết kiệm thời gian reel (bootstrap MuseAI nối tiếp nhưng generation ~60s chồng nhau). **N=1 PHẢI tái lập đúng hành vi tuần tự hiện tại** (không hồi quy B/C).

**Architecture:** Viết lại vòng lặp tuần tự trong `run_steps` thành **ready-set scheduler**: in-degree từ `graph.edges`, chọn node sẵn sàng theo CHỈ SỐ NHỎ NHẤT trong `prepared.order` (khớp Kahn → N=1 giữ nguyên thứ tự), `tokio::JoinSet` + `Semaphore(node_threads)`. **Task thuần**: nhận `inputs` (gom bằng `gather_inputs` dưới quyền sở hữu của vòng lặp TRƯỚC khi spawn), tự emit running/done + persist, trả `NodeOutcome{outputs, failure, fatal}`; **vòng lặp đơn luồng** sở hữu `outputs/failures/hard_stop/touched` và gộp khi join → KHÔNG cần khoá. MuseAI: lease per-credential giữ CHỈ quanh `fetch_bootstrap` (Engine giữ `DashMap<profile_dir, Arc<Mutex<()>>>`, chia cho 2 host).

**Tech Stack:** Rust (tokio JoinSet/Semaphore, dashmap), React+zustand.

**Spec:** `docs/superpowers/specs/2026-10-20-run-robustness-design.md` (§A).

## Global Constraints
- ADR-R014: scheduler generic, không match kind. `lvc-core` thuần.
- MSRV 1.85: **KHÔNG let-chains** (quan trọng — vòng lặp scheduler dễ lỡ dùng `if let … && …`). clippy pedantic -D warnings ×2. fmt.
- N=1 ≡ hành vi hiện tại: chọn ready theo chỉ số nhỏ nhất trong `prepared.order`; task đơn tại một thời điểm khi N=1.
- FakeDriver (test) KHÔNG chịu được >1 job đụng driver đồng thời → test N>1 PHẢI dùng node KHÔNG đụng driver (image_upload/prompt/save), không dùng dola_video song song.
- Setting mới persist qua `AppSettings` (key `node_threads`); live-apply như `threads`. Fixtures sinh từ Rust.
- Cổng: `scripts/check.ps1` với `CARGO_TARGET_DIR=D:/LVCMedia/_lvcrt`.

## Review Focus
- **N=1 giữ đúng thứ tự event** (test `video_3_canh_...` assert vector chính xác; test 2 nhánh `huy_giua_video_dau_...` giả định nhánh A trước B). Ready chọn theo prepared.order index nhỏ nhất.
- **gather_inputs đọc outputs TRƯỚC spawn** (vòng lặp sở hữu outputs; task không chạm) — successor chỉ spawn sau khi predecessor join + outputs đã insert.
- **hard_stop/cancel**: ngừng spawn node mới, join hết node đang chạy (JoinSet không bỏ giữa chừng), rồi `emit_pending_terminal` cho node chưa touched.
- **Task panic**: `join_next` trả `Err` → coi như node fail (fatal), không treo run (mirror `run_graph_task`'s spawn-catch).
- **MuseAI bootstrap collision**: 2 node museai cùng account song song → lease serialize bootstrap; sai lease = 2 Chromium cùng profile_dir.

---

### Task 1: `AppSettings.node_threads` (lvc-core)
**Files:** `crates/lvc-core/src/settings/mod.rs` (+ test cùng file).
**Interfaces (produces):** field `pub node_threads: usize` (camelCase `nodeThreads`), `const NODE_THREADS_DEFAULT = 1`, parse từ map (key `node_threads`, 0/invalid→1, >THREADS_MAX→THREADS_MAX), `to_map` ghi `node_threads`.
- [ ] Step 1 test đỏ: `node_threads_default_1` (`AppSettings::default().node_threads==1`); `node_threads_parse_va_to_map` (from_map{"node_threads":"3"} →3; to_map chứa `("node_threads","3")`); mở rộng pin test `serde_json_pin_wire_strings_mac_dinh` thêm `v["nodeThreads"]`.
- [ ] Step 2 chạy → FAIL (`cargo test -p lvc-core settings`).
- [ ] Step 3 impl: field + default + `parse_node_threads` (mirror `parse_threads`, dùng chung `THREADS_MAX`) gọi trong `from_map` + dòng trong `to_map`.
- [ ] Step 4 PASS; `cargo test -p lvc-core` xanh.
- [ ] Step 5 commit `feat(core): AppSettings.node_threads (so node chay song song)`.

### Task 2: Engine giữ node_threads sống + apply_settings (lvc-engine)
**Files:** `crates/lvc-engine/src/generate.rs` (field + getter), `crates/lvc-engine/src/settings.rs` (`apply_settings`).
**Interfaces:** Engine field `node_threads: Arc<AtomicUsize>` (khởi tạo 1 trong `Engine::new`); `pub(crate) fn node_threads(&self) -> usize` (đọc, max 1); `apply_settings` thêm `self.node_threads.store(s.node_threads.max(1), Relaxed)`.
- [ ] Step 1 test đỏ (`settings.rs` mod tests, mirror `apply_settings_doi_threads...`): `apply_settings_doi_node_threads` — set AppSettings{node_threads:3,..} → `engine.node_threads()==3`.
- [ ] Step 2 FAIL.
- [ ] Step 3 impl field + getter + store. Default 1 trong `Engine::new`.
- [ ] Step 4 PASS; `cargo test -p lvc-engine --lib settings` xanh.
- [ ] Step 5 commit `feat(engine): node_threads song (apply_settings live)`.

### Task 3: MuseAI bootstrap lease per-credential (lvc-engine)
**Files:** `crates/lvc-engine/src/generate.rs` (Engine field), `crates/lvc-engine/src/museai.rs` (2 host: field + 5th ctor param + lock quanh fetch_bootstrap), `crates/lvc-engine/src/dag.rs` start_run (truyền map).
**Interfaces:** Engine field `museai_bootstrap_locks: Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>`. Helper thuần `fn bootstrap_lock(map, profile_dir) -> Arc<Mutex<()>>` (get-or-insert). Cả 2 ctor nhận thêm `bootstrap_locks: Arc<DashMap<...>>`. Trong `generate_videos`/`generate_images`: sau khi có `profile_dir`, `let _guard = bootstrap_lock(&self.locks, profile_dir_str).lock().await;` BAO quanh CHỈ `fetch_bootstrap(...)`, thả trước `HatchConnection::connect`.
- [ ] Step 1 test đỏ (museai.rs mod tests): `bootstrap_lock_cung_key_cung_arc` — cùng profile_dir → 2 lần gọi trả Arc trỏ cùng Mutex (Arc::ptr_eq); key khác → Mutex khác.
- [ ] Step 2 FAIL.
- [ ] Step 3 impl helper + field + ctor param + lock quanh fetch_bootstrap (cả 2 host). start_run truyền `self.museai_bootstrap_locks.clone()`.
- [ ] Step 4 PASS; `cargo test -p lvc-engine --lib museai` xanh; clippy engine sạch. (Lưu MEMORY.md museai: bootstrap serialize per profile_dir khi song song.)
- [ ] Step 5 commit `feat(engine): lease bootstrap MuseAI per-credential (song song an toan)`.

### Task 4: Scheduler song song trong run_steps (lvc-engine) — TRỌNG TÂM
**Files:** `crates/lvc-engine/src/dag.rs` (`run_steps` 719-929 + helper `run_one_node`).
**Interfaces:** `NodeOutcome { node_id: String, outputs: Option<Outputs>, failure: Option<String>, fatal: Option<String> }`. `async fn run_one_node(self: Arc<Self>, run_id, workflow_id, graph: Arc<Graph>, node_id, total, services: Services, limits: Arc<Limits>, token) -> NodeOutcome` — emit running+persist, registry.get (None→failure), build ctx, gather_inputs(&graph,node_id, inputs đã truyền?) — thực ra inputs gom ở vòng lặp rồi truyền vào. Chốt: run_one_node nhận `inputs: Inputs` đã gom; emit running/done/failed + persist + set_outputs; trả NodeOutcome. **Vòng lặp** (thay 762-909): `Arc<Graph>`; `indeg`/`succ` từ edges (preds DISTINCT); ready = BinaryHeap theo `Reverse(order_index)`; `JoinSet`; `node_threads = self.node_threads()`; fill tới `in_flight < node_threads` (skip-node xử inline không chiếm slot: emit done+persist+set_outputs+preview, enqueue successors); non-skip: gom inputs (đọc `outputs` sở hữu), spawn `run_one_node`; `join_next().await` → fold (insert outputs/failures/hard_stop/touched) + giảm indeg successor → enqueue; dừng fill khi hard_stop/cancel, vẫn join hết. Panic join → fatal.
- [ ] Step 1 test đỏ:
  - `scheduler_hai_nhanh_doc_lap_N2_hoan_tat`: graph 2 chuỗi image_upload→save ĐỘC LẬP (không đụng driver), `node_threads=2` → run completed, cả 4 node done, 2 file lưu. (Dựng engine với node_threads=2 qua apply_settings hoặc set field.)
  - `scheduler_N1_giu_thu_tu_tuan_tu`: tái dùng graph linear 1 cảnh — events = queued×n, rồi running/done theo đúng thứ tự topo (như test cũ). (Có thể dựa test cũ `video_3_canh_...` vẫn xanh thay cho test mới.)
- [ ] Step 2 FAIL (compile/hành vi).
- [ ] Step 3 impl. KHÔNG let-chains. Giữ `emit_pending_terminal`/`tasks_ready_tx`/error-agg y cũ. Fold theo thứ tự join (outputs/failures); `hard_stop` lấy fatal đầu tiên gặp.
- [ ] Step 4 PASS + **toàn bộ `cargo test -p lvc-engine --lib` xanh** (đặc biệt `video_3_canh_...`, `huy_giua_video_dau_...`) + clippy engine sạch.
- [ ] Step 5 commit `feat(engine): scheduler song song N node (ready-set + semaphore, N=1 giu tuan tu)`.

### Task 5: UI chọn node_threads (frontend)
**Files:** `app/src/ipc/types.ts` (zod `nodeThreads`), `app/src/components/settings/GeneralSettings.tsx` (ô nhập), `app/src/components/settings/validate.ts`, `app/src/store/settings.ts` (clamp), `app/src/ipc/fixtures/app_settings.json`.
**Interfaces:** `AppSettingsSchema.nodeThreads: z.number()`; UI SettingRow "Số node chạy song song" (min 1, max THREADS_MAX) cạnh "Số luồng"; clamp trong `toAppSettings`.
- [ ] Step 1 test đỏ (nếu validate.ts có hàm thuần): `validateSettings` bắt nodeThreads ngoài [1,64]. (Nếu không tách được, dựa typecheck/build.)
- [ ] Step 2 FAIL.
- [ ] Step 3 impl type + UI row + validate + clamp + fixture (`"nodeThreads": 1`).
- [ ] Step 4 `pnpm test` + `pnpm typecheck` + `pnpm build` xanh.
- [ ] Step 5 commit `feat(ui): chon so node chay song song (node_threads)`.

## Nghiệm thu (xin phép user; LIVE tốn credit)
Reel với node_threads=3 → nhanh hơn rõ (bootstrap nối tiếp, generation chồng), không kẹt profile, concat nhận đủ video đúng thứ tự; node_threads=1 → như cũ.
