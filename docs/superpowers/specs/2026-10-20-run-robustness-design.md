# Thiết kế: Độ bền của Run (lưu/khôi phục + xem lỗi + chạy lại node lỗi + chạy song song)

Ngày: 2026-10-20. Trạng thái: CHỜ DUYỆT. Thứ tự làm (user chốt): **B → D → C → A**.

## Ý định & vì sao

Reel MuseAI chạy ~28 phút (14 cảnh × ảnh+video, tuần tự). Người dùng gặp 3 đau:
1. **Tắt/mở app (hoặc dev build lại) → MẤT video đang/đã làm** (UI quên; dù file vẫn trên đĩa).
2. **Node lỗi không bấm xem được lý do** (chỉ hiện "1 lỗi").
3. **Chạy lại là làm lại TỪ ĐẦU** (tốn credit + thời gian) thay vì chỉ chạy lại node lỗi.
4. Chạy tuần tự quá lâu.

## Sự thật kiến trúc (khảo sát 2026-10-20) — thiết kế bám theo

- Scheduler: `lvc-engine/src/dag.rs:628` `for node_id in prepared.order` — **tuần tự**, 1 `exec.run().await`/node. `outputs: HashMap` nuôi xuôi. `DynamicLimiter` (`limiter.rs`) có sẵn nhưng gate SCENE trong 1 node, không gate node.
- Persistence: bảng `runs` (run-level) + `tasks` (per-scene, **chỉ Dola**). **MuseAI/Veo/Google/Seedance KHÔNG ghi DB** — chỉ event `NodeState`/`NodeOutputs` + file. **Không có bảng per-node.**
- File tất định: `museai.rs:53/59` `{run_id}_{node_id}_{scene_index}.ext` trong `downloads_dir` → **quét lại được** map (run_id,node_id,scene) kể cả khi không có DB row. **Đây là đòn bẩy chính.**
- UI run state: zustand per-tab (`app/src/store/run.tsx`), event-sourced, **mất khi reload**; có `reconcileTasksFromDb` nhưng chỉ phục hồi node CÓ `tasks` row (Dola) và chỉ khi `runId` còn trong bộ nhớ.
- Lỗi node: `NodeState.error` → store `nodes[id].error` → **`NodeLogPanel.tsx` đã hiện khi bấm node** (live + Dola-sau-reconcile). Thiếu: bền hóa cho node non-Dola + render từ state đã lưu.
- Resume hiện có (`resume.rs`): chỉ mức Dola-task, `requeue_on_start` ÉP mọi run `running`→`failed`. **Không resume được run dở.**

## Ràng buộc (cứng)

- **ADR-R014**: `dag.rs` KHÔNG `match` theo kind — mọi thứ qua `Registry`+`Services`. Persistence per-provider phải đẩy vào host service hoặc làm GENERIC ở scheduler (không special-case theo kind).
- `lvc-core` THUẦN: logic plan (cho C) ở `lvc_core::graph::plan`, không I/O. Persistence ở `lvc-store`/`lvc-engine`.
- MSRV 1.85 (không let-chains), clippy pedantic -D warnings ×2, fmt, `scripts/check.ps1`.
- **1 account MuseAI = 1 trình duyệt/profile**: `fetch_bootstrap` khoá profile_dir → **nhiều node MuseAI cùng account KHÔNG được bootstrap đồng thời** (kẹt lock). Browser đóng TRƯỚC generation (chỉ WebSocket) → chỉ CỬA SỔ BOOTSTRAP là điểm tranh chấp; generation chạy chồng được.
- `JoinSet` không bỏ giữa chừng (join hết) để DB không kẹt `processing`.
- Resume/chạy-lại KHÔNG re-submit cảnh đã có kết quả (tốn credit).

## B — Lưu & khôi phục run (nền tảng)

**Mục tiêu:** tắt/mở app, hoặc chuyển tab, hoặc dev build lại → node đã xong vẫn hiện (ảnh/video preview), node lỗi vẫn thấy lỗi; run dở KHÔNG bị ép fail oan.

- **Bảng mới `node_states`** (migration mới, `lvc-store`): `(run_id, node_id, status, done, total, ok, failed, error TEXT, updated_at)`. Repo `NodeStateRepo` (thuần, như `TaskRepo`).
- **Ghi generic ở scheduler:** tại MỌI điểm `emit_node_state`/`emit_node_state_err` (dag.rs) → `node_states.upsert(...)`. Vì làm ở scheduler (không trong provider) nên **generic, đúng ADR-R014** — mọi kind đều được lưu, không special-case.
- **Outputs preview không cần lưu bytes:** khôi phục bằng **quét `downloads_dir`** theo `{run_id}_{node_id}_*` (tất định) → map lại `nodeVideos`/preview. Thêm lệnh `run_node_outputs(run_id)` trả map node→path (quét đĩa).
- **Không ép fail run dở:** `requeue_on_start` đổi: run `running` + còn node chưa xong → giữ trạng thái `interrupted`/để resume thay vì `failed` thẳng (giữ hành vi cũ cho Dola tasks). Chi tiết mức độ để builder quyết, miễn KHÔNG mất node đã xong.
- **QUYẾT ĐỊNH (user 2026-10-20): KHÔI PHỤC RỒI CHỜ BẤM.** Mở lại app → dựng lại đầy đủ trạng thái (cảnh xong hiện video, cảnh lỗi hiện đỏ) nhưng **KHÔNG tự chạy tiếp, KHÔNG tự mở trình duyệt MuseAI**. Người dùng xem rồi bấm "Chạy tiếp"/"Chạy lại node lỗi" (phần C). Lý do: không tự ý tốn credit khi app vừa mở; người dùng kiểm soát. → Không auto-resume ở phần B; "chạy tiếp các node chưa xong" dùng chung cơ chế sub-run của phần C.
- **UI khôi phục:** `Workflow.tsx` khi mở tab/boot → gọi `run_list` lấy run gần nhất của workflow + `node_states` + `run_node_outputs` → `reconcile` dựng lại `nodes`/`nodeVideos`. Nhớ `runId` gần nhất của tab (localStorage per-workflow) để re-attach khi boot.
- **Nghiệm thu:** chạy reel vài cảnh → tắt app → mở lại (hoặc đổi tab) → các cảnh xong vẫn hiện video + node lỗi vẫn đỏ + bấm xem được lỗi; KHÔNG mất.

## D — Bấm node lỗi xem lý do (đa phần đã có)

- `NodeLogPanel.tsx` đã hiện `runState.error` + scene errors khi bấm node. Việc còn lại **cưỡi lên B**: (a) error đã bền hóa trong `node_states` → sau restart vẫn còn; (b) `NodeLogPanel` render được từ state ĐÃ KHÔI PHỤC (không chỉ live). Đảm bảo thông điệp echo/"0 ảnh"/timeout đến `NodeState.error` (hiện có ở dag.rs:698-711; xác nhận museai failure string lên tới đây).
- **Nghiệm thu:** node lỗi (vd echo) → bấm → thấy đúng câu lý do, cả khi live lẫn sau restart.

## C — Chạy lại CHỈ node lỗi + tự ghép lại

- **Plan sub-run (`lvc_core::graph::plan`, thuần):** cho graph + tập node đã `done` (có outputs) → tính tập node CẦN chạy lại = node `failed` + node phụ thuộc chưa xong + node reassembly (concat/save). Node `done` được SEED outputs, KHÔNG chạy lại.
- **Engine:** biến thể `run_graph` nhận "seed outputs" (từ `node_states` + quét đĩa cho node done) + "skip set" (done). Reuse scheduler. Video node đã có `conversation_id` (Dola) ưu tiên `resume` không re-submit; MuseAI không có conversation lưu → chạy lại node đó (chấp nhận tốn credit CHỈ node lỗi).
- **UI:** nút **"Chạy lại node lỗi"** (RunBar) khi run có node `failed` → gọi `run_retry_failed(run_id)` → merge state. Concat/save tự chạy lại để ra video hoàn chỉnh.
- **Nghiệm thu:** reel 14 cảnh, 2 cảnh lỗi → "Chạy lại node lỗi" → chỉ 2 cảnh đó + ghép lại chạy; 12 cảnh cũ giữ nguyên (không tốn credit lại).

## A — Chạy song song N node (chọn số)

- **Scheduler ready-set:** thay vòng tuần tự (dag.rs:628) bằng: in-degree per node + ready set + `FuturesUnordered`/`JoinSet` + `tokio::sync::Semaphore(N)`. `outputs` thành `Arc<Mutex<HashMap>>` hoặc gom qua channel. `NodeCtx`/`Services` đã Clone/Arc → chia sẻ an toàn.
- **Cấu hình N:** setting mới `node_threads` (`lvc-core` `Limits`/settings) — UI cho chọn (mặc định 2 hoặc 3; 1 = như cũ). Dùng `DynamicLimiter` mới mức-node hoặc Semaphore.
- **Khoá bootstrap theo credential:** node MuseAI cùng credential phải nối tiếp PHẦN BOOTSTRAP (profile lock) — thêm lease per-credential giữ CHỈ trong `fetch_bootstrap` rồi thả trước WebSocket (mirror `AccountPool::lease_wait`). Generation (~60s) chạy chồng → vẫn nhanh dù 1 account.
- **Nghiệm thu:** reel với N=3 nhanh hơn rõ (bootstrap nối tiếp ngắn, generation chồng); không kẹt profile lock; không hỏng thứ tự ghép (concat vẫn nhận đủ video theo scene_index).

## Rủi ro / đánh đổi

- B: quét đĩa để preview — nếu user xoá file thủ công thì mất preview (chấp nhận). Dùng tên tất định nên map chắc.
- A: 1 account MuseAI → lợi ích song song giới hạn bởi bootstrap nối tiếp; nếu MuseAI rate-limit nhiều phiên WebSocket thì giảm N (vì vậy cho chọn số). Builder test thực N=2/3.
- C với MuseAI: không có conversation lưu → chạy lại node lỗi vẫn tốn credit node đó (nhưng KHÔNG làm lại node đã xong — đã tiết kiệm phần lớn).

## Kiểm thử/nghiệm thu chung

- Mỗi tính năng: unit test phần thuần (plan sub-run, scheduler ready-set logic, node_states repo) + `scripts/check.ps1` xanh + chạy live nghiệm thu đúng mục "Nghiệm thu" của từng phần (xin phép user trước mỗi live).
