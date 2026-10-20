# MuseAI Image-First Reel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reel MuseAI khoá nhân vật bằng cách mỗi cảnh tạo một ẢNH tĩnh (từ ref cô gái + trang phục + heo) rồi dựng VIDEO 3s từ chính ảnh đó, thay cho cách cũ đưa 2 ref mâu thuẫn thẳng vào video.

**Architecture:** Thêm cổng nhận ảnh ref cho node `museai_image` (mirror `museai_video`), cho `generate_image` của provider đính ảnh base64 vào `/chat/stream` (cơ chế đã có cho video), bỏ hậu tố "product photography" ép ảnh thành chụp sản phẩm, rồi dựng lại template thành `3 image_upload → 6×(museai_image→museai_video) → concat → save` với prompt tối ưu bố cục cảnh.

**Tech Stack:** Rust (lvc-museai provider, node-museai-image, lvc-engine), TypeScript/React (app/src/graph/ports.ts), template JSON, MuseAI Hatch/Noise protocol.

**Spec:** `docs/superpowers/specs/2026-10-20-museai-image-first-reel-design.md`

## Global Constraints

- MSRV 1.85 — KHÔNG let-chains. `clippy::pedantic` + `-D warnings`, chạy CẢ `cargo clippy --workspace --all-targets` VÀ `cargo clippy -p lvc-app --features fake-driver --all-targets`.
- ADR-R014: node chỉ phụ thuộc `lvc-node-api`/`lvc-core`/`crates/providers/*`/thư viện ngoài. Provider `lvc-museai` KHÔNG biết khái niệm node; cấm crate impersonate TLS.
- `lvc-core` thuần. KHÔNG log bí mật (cookie/token/prompt đầy đủ). KHÔNG giả mạo TLS/JA3.
- Ngân sách ref ảnh: ≤3,5 MB mỗi ảnh, ≤11 MB tổng (đã ép ở `attachments.rs::build_items`).
- `kind` giữ nguyên `museai_image`. Thêm CỔNG input KHÔNG phải đổi `params` → **KHÔNG** bump `manifest.version`; nhưng manifest đổi (thêm port) → **sinh lại fixture**, không sửa tay: `LVC_WRITE_FIXTURES=1 cargo test -p node-museai-image` rồi `... -p lvc-app commands::fixtures`.
- Chạy thật tốn credit MuseAI — HỎI người dùng trước mỗi lần chạy live; dev/test dùng fake-driver.
- Cổng chất lượng trước commit: `powershell -ExecutionPolicy Bypass -File scripts/check.ps1`.

## Review Focus

- **museai_image có `refs` nhưng KHÔNG có prompt nối vào** → 0 job, KHÔNG lỗi (giống `museai_video`). Test ở Task 3.
- **museai_image có prompt nhưng KHÔNG có refs** (fallback) → vẫn chạy, `images` rỗng, sinh ảnh thường. Test ở Task 3.
- **`build_image_prompt` sau khi bỏ "product photography" vẫn phải báo hiệu "ảnh tĩnh, không phải video"** để muse.ai trả `kind:"image"` (không ra video). Test ở Task 2.
- **Ảnh ref vượt ngân sách (>3,5 MB)** → `build_items` trả lỗi rõ ràng, node ghi `failure` chứ không panic (đường dẫn đã có cho video; xác nhận không vỡ ở image). Test ở Task 2.
- **Cảnh 2–6 không có ref heo** → `museai_image` chỉ nhận nv+ao, vẫn tạo ảnh đúng cô gái. Bao phủ bởi template hợp lệ (Task 6) + fallback test (Task 3).

---

### Task 1: Baseline — cổng xanh + commit việc đang dở

Làm sạch cây làm việc TRƯỚC khi xây tính năng: các sửa đổi hôm 2026-10-19/20 (lỗi node Nối video `adapter REMOVED_KINDS`, chống xoá cạnh `graph.tsx`, đổi label concat, nút Huỷ MuseAI ở `museai.rs`/`dag.rs`, fixtures đã sinh lại, tài liệu spec) đang chưa commit.

**Files:** không tạo mới; commit các file đang đổi (xem `git status`).

- [ ] **Step 1:** Chạy cổng đầy đủ: `powershell -ExecutionPolicy Bypass -File scripts/check.ps1`. Expected: xanh. Nếu đỏ (clippy/test ở phần cancel fix `crates/lvc-engine/src/museai.rs`), sửa cho tới khi xanh.
- [ ] **Step 2:** `git add -A` rồi commit baseline (tách khỏi tính năng mới). Thông điệp gói: sửa node Nối video hiện lại khi nạp + chống xoá cạnh + nút Huỷ MuseAI cắt ngang + đổi tên "Nối video (ghép cảnh)".
- [ ] **Step 3:** `git status` sạch (chỉ còn file của tính năng sắp làm, hiện chưa có). Expected: nothing to commit ngoài spec/plan nếu muốn commit riêng.

---

### Task 2: Provider `lvc-museai` — ảnh ref cho `generate_image` + bỏ "product photography"

**Files:**
- Modify: `crates/providers/lvc-museai/src/job.rs:48-52` (`ImageJob`)
- Modify: `crates/providers/lvc-museai/src/protocol/client.rs` (`GenerateImageParams`, `generate_image`, `build_image_prompt`, test ~707)
- Test: cùng file `client.rs` (mục `#[cfg(test)]`)

**Interfaces:**
- Produces: `ImageJob { scene_index: u32, credential_id: String, prompt: String, images: Vec<PathBuf> }` (thêm `images`, đối xứng `VideoJob`).
- Produces: `GenerateImageParams` thêm `pub images: &'a [PathBuf]` (đối xứng `GenerateVideoParams`).
- `build_image_prompt(user_prompt: &str) -> String` giữ chữ ký, đổi NỘI DUNG hậu tố.

- [ ] **Step 1: Viết test (sửa test ghim hiện có + thêm test người).** Trong `client.rs` test module: đổi `build_image_prompt_giu_nguyen_y_*` để KHẲNG ĐỊNH hậu tố (a) KHÔNG còn chứa `"product photography"`, (b) VẪN chứa tín hiệu ảnh tĩnh — assert chuỗi chứa `"still"` và `"no video"` (hoặc `"not a video"`). Thêm 1 assert: với prompt người (`"cô gái mặc đầm yếm nâu"`) kết quả vẫn có nguyên `user_prompt` ở đầu.

```rust
let p = build_image_prompt("cô gái mặc đầm yếm nâu");
assert!(p.starts_with("cô gái mặc đầm yếm nâu"));
assert!(!p.to_lowercase().contains("product photography"));
assert!(p.to_lowercase().contains("still") && p.to_lowercase().contains("no video"));
```

- [ ] **Step 2: Chạy test, xác nhận ĐỎ.** Run: `cargo test -p lvc-museai build_image_prompt`. Expected: FAIL (hậu tố cũ còn "product photography").
- [ ] **Step 3: Sửa `build_image_prompt`** (client.rs:550) — hậu tố mới mô tả "một ảnh chụp đời thường một NGƯỜI, ảnh tĩnh, không chuyển động/không phải video" (bỏ "product photography style"). Giữ ý "single still photo ... no motion, no video". Builder chọn câu chữ tiếng Anh ngắn gọn đạt 3 assert trên.
- [ ] **Step 4: Thêm field `images`** vào `ImageJob` (job.rs) và `GenerateImageParams` (`pub images: &'a [PathBuf]`); trong `generate_image` (client.rs:571 vùng) đổi lời gọi `chat_stream(..., &[], ...)` thành `chat_stream(..., params.images, ...)` (hiện video đã truyền `params.images`; mirror y hệt).
- [ ] **Step 5: Chạy test + clippy.** Run: `cargo test -p lvc-museai` và `cargo clippy -p lvc-museai --all-targets -- -D warnings`. Expected: PASS, sạch.
- [ ] **Step 6: Commit.** `feat(museai): generate_image nhận ảnh ref + bỏ ép "product photography"`.

---

### Task 3: Node `museai_image` — cổng `refs` + map vào `ImageJob.images`

**Files:**
- Modify: `crates/nodes/node-museai-image/src/lib.rs` (MANIFEST inputs, `build_jobs`)
- Test: cùng file (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `ImageJob.images` (Task 2).
- Produces: manifest `museai_image` có 2 cổng vào: `prompt`(text, many, required) + `refs`(image, many) — KHÔNG required.

- [ ] **Step 1: Viết test** cho `build_jobs` (thuần): (a) có prompt + có refs → mỗi job mang `images` = danh sách path của refs (dùng CHUNG mọi job, mirror `node-museai-video`); (b) có prompt, KHÔNG refs → `images` rỗng, vẫn ra job; (c) có refs, KHÔNG prompt → 0 job. Mẫu assert mirror test `build_jobs_*` của `node-museai-video`.

```rust
// (a)
assert_eq!(jobs.len(), 2);
assert_eq!(jobs[0].images, vec![PathBuf::from("a.png"), PathBuf::from("b.png")]);
// (b) prompt, no refs
assert!(jobs[0].images.is_empty());
// (c) refs, no prompt
assert!(jobs.is_empty());
```

- [ ] **Step 2: Chạy test, xác nhận ĐỎ.** Run: `cargo test -p node-museai-image build_jobs`. Expected: FAIL (chưa có cổng refs / field images chưa map).
- [ ] **Step 3: Thêm cổng** `PortSpec::new("refs", PortType::Image).many()` vào MANIFEST.inputs (sau `prompt`). Trong `build_jobs`: lấy `let images = inputs.assets("refs").map(|a| PathBuf::from(&a.path)).collect::<Vec<_>>();` và gán `images: images.clone()` vào mỗi `ImageJob` (mirror `node-museai-video::build_jobs`).
- [ ] **Step 4: Chạy test + clippy.** `cargo test -p node-museai-image`, `cargo clippy -p node-museai-image --all-targets -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Sinh lại fixture node** (manifest đổi vì thêm cổng, KHÔNG bump version): `LVC_WRITE_FIXTURES=1 cargo test -p node-museai-image`. Kiểm `git diff` chỉ thêm cổng `refs` trong `manifest.json`.
- [ ] **Step 6: Commit.** `feat(node-museai-image): thêm cổng refs (ảnh tham chiếu)`.

---

### Task 4: Engine host — truyền `job.images` vào `generate_image`

**Files:**
- Modify: `crates/lvc-engine/src/museai.rs` (`EngineMuseaiImageJobs::generate_images`, vòng lặp dựng `GenerateImageParams`)

**Interfaces:**
- Consumes: `GenerateImageParams.images` (Task 2), `ImageJob.images` (Task 3).

- [ ] **Step 1:** Trong `generate_images`, nơi dựng `GenerateImageParams { ... }`, thêm `images: &job.images`. (Đối xứng `generate_videos` đã làm với `images: &job.images`.)
- [ ] **Step 2: Build + clippy.** Run: `cargo check -p lvc-engine` và `cargo clippy -p lvc-engine --all-targets -- -D warnings`. Expected: sạch. (RPC không unit-test được; đã có test thuần ở provider/node.)
- [ ] **Step 3: Commit.** `feat(engine): host museai_image chuyển ảnh ref xuống generate_image`.

---

### Task 5: Frontend — cổng `refs` cho `museai_image` + sinh lại catalog

**Files:**
- Modify: `app/src/graph/ports.ts` (PORTS.museai_image.inputs)
- Regenerate: `app/src/ipc/fixtures/catalog.json` (qua lệnh Rust)

**Interfaces:**
- Consumes: manifest `museai_image` mới (Task 3).

- [ ] **Step 1:** Sửa `PORTS.museai_image.inputs` thêm `{ name: "refs", type: "image" }` (sau `prompt`), khớp manifest Rust.
- [ ] **Step 2: Sinh lại catalog fixture:** `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
- [ ] **Step 3: Chạy test FE.** Run: `cd app && pnpm exec vitest run tests/ipc_fixtures.test.ts tests/labels.test.ts` và `pnpm exec tsc --noEmit`. Expected: PASS (ports.ts khớp catalog).
- [ ] **Step 4: Commit.** `feat(ui): museai_image có cổng refs (khớp manifest)`.

---

### Task 6: Template `museai-reel-mon-an.json` — pipeline ảnh-trước + prompt tối ưu

**Files:**
- Modify (ghi đè): `crates/lvc-app/templates/museai-reel-mon-an.json` (giữ id `public:museai-reel-mon-an`)
- Test: `crates/lvc-app/src/commands/templates.rs::public_mau_hop_le_va_migrate_duoc` (đã có)

**Cấu trúc node (23 node):**
- 3 `image_upload`: `nv` (label "Ảnh nhân vật (character sheet)"), `ao` ("Ảnh trang phục"), `heo` ("Ảnh con heo").
- Mỗi cảnh i∈1..6: `pi` (prompt ẢNH) → `imgi` (`museai_image`, refs ← `nv`+`ao`, riêng `img1` thêm ref `heo`) → `vi` (`museai_video`, refs ← `imgi.out`, prompt ← `pvid`).
- 1 prompt chung `pvid` (prompt VIDEO, nối tới cả `v1..v6.prompt`).
- `ghep` (`concat`) ← `v1..v6.out`; `save` ← `ghep.out`.

**Prompt (tối ưu bố cục — builder chốt câu chữ theo nguyên tắc spec; mỗi `pi` = 1 cảnh):**
Nhân vật khoá trong MỌI `pi`: "đúng cô gái trong ảnh tham chiếu (giữ nguyên gương mặt, tóc nâu dài gợn sóng), mặc ĐẦM YẾM NÂU ôm dáng + cao gót đế xuồng trắng, quay dọc 9:16 kiểu UGC bằng điện thoại". Bối cảnh từng cảnh:
- Cảnh 1 (hook): đứng ở hành lang căn hộ, tay **dắt một con heo** (theo ảnh heo), nhìn thẳng camera tươi tắn.
- Cảnh 2: đứng trong bếp, bê giỏ nguyên liệu tươi khoe.
- Cảnh 3: cận cảnh sơ chế/rửa nguyên liệu trên quầy bếp.
- Cảnh 4: cận cảnh thả thịt/rau vào nồi nước sôi, hơi nước bốc lên.
- Cảnh 5: múc món ăn ra đĩa, trang trí, góc từ trên xuống.
- Cảnh 6 (chốt): bê đĩa món ăn khoe thẳng camera, mỉm cười mời.

`pvid` (dùng chung): "Làm động ảnh đính kèm một cách tự nhiên: cùng CHÍNH người và trang phục trong ảnh, chuyển động nhẹ ~3 giây, quay selfie UGC dọc 9:16, chỉ tiếng động môi trường." (KHÔNG mô tả lại ngoại hình để không mâu thuẫn với ảnh.) Mỗi `vi`: `aspect:"9:16"`, `duration:3`.

- [ ] **Step 1:** Viết lại JSON template theo cấu trúc trên (23 node + cạnh). Đặt toạ độ gọn: cột uploads (x≈40), cột prompt ảnh (x≈360), cột museai_image (x≈680), cột museai_video (x≈1000), prompt video (x≈1000,y cao), concat (x≈1340), save (x≈1640). Dùng python sinh JSON cho chắc id/cạnh.
- [ ] **Step 2: Chạy test template.** Run (target riêng để không đụng app đang chạy): `set CARGO_TARGET_DIR=D:/LVCMedia/_lvcrt && cargo test -p lvc-app public_mau_hop_le_va_migrate_duoc`. Expected: PASS (template hợp lệ, prepare qua registry). Nếu `video_total`/assert số mẫu đổi, cập nhật test cho khớp.
- [ ] **Step 3:** Kiểm thủ công JSON: đúng 6 cạnh `vi→ghep`, 1 cạnh `ghep→save`, mỗi `imgi` có cạnh `nv→refs`+`ao→refs` (img1 thêm `heo→refs`), mỗi `imgi.out→vi.refs`, mỗi `pi→imgi.prompt`, `pvid→vi.prompt`.
- [ ] **Step 4: Commit.** `feat(template): reel món ăn ảnh-trước (khoá nhân vật) thay bản cũ`.

---

### Task 7: Cổng tổng + chạy thử LIVE 2 cảnh + commit cuối

**Files:** không đổi code; nghiệm thu.

- [ ] **Step 1:** `powershell -ExecutionPolicy Bypass -File scripts/check.ps1`. Expected: xanh toàn bộ (Rust fmt/clippy×2/test + dep + FE typecheck/test/build).
- [ ] **Step 2: Chuẩn bị ref:** copy/ghi chú đường dẫn `image/mau.png`, `image/trangphuc.png`, `image/heo.png` để nạp vào 3 node upload (nv←mau, ao←trangphuc, heo←heo).
- [ ] **Step 3: Chạy thử LIVE 2 CẢNH (XIN PHÉP NGƯỜI DÙNG trước — tốn credit).** Tạo biến thể 2 cảnh (chỉ `img1→v1`, `img2→v2` → concat) hoặc chạy template và chỉ xét 2 cảnh đầu. Bật tài khoản MuseAI (`enabled=1`). Chạy.
- [ ] **Step 4: Nghiệm thu:** trích frame bằng ffmpeg → kiểm **cùng mặt + cùng đầm yếm nâu** ở cả 2 cảnh, cảnh 1 có con heo. Nếu đạt, báo người dùng để chạy full 6; nếu lệch, chỉnh prompt (Task 6) và lặp.
- [ ] **Step 5: Commit** mọi chỉnh prompt/fixture phát sinh. Cập nhật `MEMORY.md` dự án (trạng thái + mốc nghiệm thu).
