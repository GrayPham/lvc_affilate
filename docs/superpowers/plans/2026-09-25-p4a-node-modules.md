# Kế hoạch P4a — Node thành module độc lập (không đổi hành vi)

> Spec: `docs/superpowers/specs/2026-09-25-node-modules-design.md` (ADR-R014).
> Nghiệm thu: toàn bộ test cũ (đã chuyển chỗ) xanh; workflow v1 mở/chạy được; tab Kịch bản chạy như cũ;
> `scripts/check.ps1` xanh; thêm 1 node = 1 crate + 1 dòng đăng ký.

## Quyết định chi tiết (chốt khi đọc code P3)

1. **Định dạng JSON giữ nguyên hình dạng**: `Node { id, kind: NodeData, x, y }` với
   `NodeData { kind: String, #[serde(flatten)] params: Map }` → `{"kind":{"kind":"dola_video","mode":…}}`
   y như v1. `Graph.version = 2`. `migrate_v1` chỉ đổi tên kind `video` → `dola_video`.
   Kind giữ: `prompt`, `image_upload`, `save`; đổi: `video` → `dola_video`. Các kind P4 cũ chưa dựng
   (`image_from_url`, `image_gen`, `remove_logo`, `ensure_h264`, `concat`) bị gỡ khỏi lõi — sẽ quay lại
   dưới dạng module riêng; workflow cũ chứa chúng → `GraphError::Unsupported` như trước.
2. **Manifest thuần ở `lvc-core::graph`** (`NodeManifest`, `PortSpec` sở hữu `String`, `Category`,
   `CredentialReq`, trait `Catalog`). `validate(g, &dyn Catalog)` giữ đúng thứ tự/luật lỗi cũ; luật
   "Save cần video HOẶC image" thành trường chung `require_any: Vec<Vec<String>>` trong manifest.
3. **`plan.rs` bị giải thể**: phần Prompt → `node-prompt`, phần dựng job video (gộp TXT/mặc định/refs,
   `TooManyRefs`, `batch_max_scenes`) → `node-dola-video`. Thay bằng `preview()` thuần của từng node,
   chạy theo topo trong `lvc_node_api::Registry::prepare(graph, limits)` → `Prepared { order, totals,
   video_total }` hoặc `Vec<GraphError>` (bộ lỗi y như cũ).
4. **Dữ liệu trên dây**: `Value::{Text(TextItem), Image(Asset), Video(Asset)}`. `TextItem { text,
   source_node, line, attrs }` — `attrs` là override TXT thô (`ratio, dur, model, mode, logo, refs, acc`).
   `Asset { path, meta }` với `meta.node` (node sinh) + `meta.index` (scene_index).
   Đầu vào node: `Inputs` = theo cổng, theo **từng cạnh** (`EdgeInput { from_node, values: Option<Vec<Value>> }`,
   `None` = nguồn không chạy) → Save giữ được lỗi "không có đầu ra để lưu".
5. **Dola video là dịch vụ host**: crate mới `crates/providers/lvc-dola` khai
   `trait DolaVideoHost { async fn run_jobs(run_id, node_id, jobs: Vec<(u32, Scene)>, cancel) -> JobsReport }`.
   `lvc-engine` impl nó bằng NGUYÊN `run_video_step` hiện tại (giai đoạn 1/2a/2b/2c, `tasks_ready`,
   `hooks_factory`, `mark_row_failed`) — không viết lại logic đã qua 2 vòng review. Node lấy host qua
   `ctx.services.get::<Arc<dyn DolaVideoHost>>()`. Không có Arc vòng: engine chèn chính nó vào
   `Services` MỖI LẦN chạy.
6. **Engine generic**: `run_steps` duyệt topo, với mỗi node: kiểm huỷ/hard_stop → phát `running` →
   gom `Inputs` → `executor.run(ctx, params, inputs)` → phát trạng thái cuối theo `NodeResult.counts`.
   `NodeError { fatal: true }` = hard_stop cũ (ImageUpload thiếu file). Chạy tuần tự như P3 (song song
   nhánh để P4d). Bẫy panic giữ nguyên (`tokio::spawn(steps_fut)`).
7. **Registry** sở hữu bởi `Engine` (`OnceLock<Arc<Registry>>`, `Engine::set_registry`), dựng ở
   `lvc-app` (composition root, 1 dòng/node). Test engine dùng node crate làm dev-dependency.
8. **UI P4a** tối thiểu: đổi `video` → `dola_video`, bỏ kind P4 cũ, `ports.json` → `catalog.json` (sinh từ
   registry), `ports.ts` đọc từ fixture. Canvas localStorage cũ tự đổi tên kind khi nạp.
   UI dựng từ catalog lúc chạy + panel kiểu Lumina để P4d.

## Task

| # | Việc | File chính |
|---|---|---|
| 1 | `lvc-core::graph` v2: `NodeData`, manifest, `Catalog`, `validate` generic, `migrate_v1`; gỡ `NodeKind`/`plan.rs` | `crates/lvc-core/src/graph/*` |
| 2 | `lvc-node-api`: `Value/TextItem/Asset`, `Inputs`, `NodeExecutor`, `NodeCtx`, `Services`, `NodeResult/NodeError`, `Registry::prepare`, `FakeServices` | `crates/lvc-node-api` |
| 3 | `lvc-dola` (host trait) + 4 node: `node-prompt`, `node-image-upload`, `node-save`, `node-dola-video` — chuyển test `plan.rs` sang | `crates/providers/lvc-dola`, `crates/nodes/*` |
| 4 | Engine: `dag.rs` generic + `impl DolaVideoHost for Engine`; `scripts.rs` dựng graph mới; chuyển test dag | `crates/lvc-engine/src/{dag,scripts}.rs` |
| 5 | App: `build_registry()`, `node_catalog`, `workflow_validate` qua `prepare`, migrate khi đọc, fixture `catalog.json` | `crates/lvc-app/src/*` |
| 6 | UI: đổi kind, catalog fixture, test | `app/src/*`, `app/tests/*` |
| 7 | Luật + ghi nhớ: `CLAUDE.md` gốc, mỗi node `CLAUDE.md/MEMORY.md/README.md/manifest.json`, `scripts/new-node.ps1`, `check.ps1` cưỡng chế phụ thuộc | |
