# P7 — Kho template "remote" (GitHub + Vercel, miễn phí)

Trạng thái: **P7a + P7b đã code xong, chờ người dùng 2 bước thủ công** (repo GitHub phải PUBLIC +
push `index.json`/`thumbs/`/`README.md` mới — xem mục "Việc còn lại" cuối file). Quyết định cuối
(2026-10-11, thay đổi so với spec gốc): **KHÔNG dùng Vercel** — app gọi THẲNG
`raw.githubusercontent.com` một file `index.json` DUY NHẤT (mảng template đầy đủ, kèm `graph`),
không qua lớp trung gian nào. Scaffold Vercel (`api/`, `vercel.json`) trong repo ngoài vẫn còn,
đánh dấu "chưa dùng/dự phòng" trong README của repo đó.
Spec: `docs/superpowers/specs/2026-10-11-remote-template-registry-design.md`.

## Mục tiêu

Thêm nguồn template thứ 3 ("remote") cho nút "Kho mẫu": app gọi lên một API Vercel đọc JSON +
thumbnail từ một repo GitHub public, miễn phí, để thêm mẫu mới không cần build lại app.

## Ranh giới P7a (đã làm trong lượt này)

- Chỉ **scaffold code + hướng dẫn** cho repo ngoài — KHÔNG tự `gh repo create`/`vercel deploy` thay
  người dùng (hành động public khó undo; người dùng xác nhận muốn tự làm).
- KHÔNG đụng code trong `lvc_affilate` (app chưa biết nguồn "remote" tồn tại).

## P7a — Scaffold repo registry ngoài

Vị trí: `d:\LVCMedia\lvc-template-registry` (sibling của `lvc_affilate`, chưa git init).

1. `package.json`, `vercel.json` (CORS cho `/api/*`).
2. `api/templates/index.js` — đọc `templates/*.json`, trả mảng meta (bỏ `graph` cho nhẹ).
3. `api/templates/[id].js` — trả đủ 1 template (gồm `graph`).
4. `templates/ugc-unboxing-demo.json` — 1 mẫu demo, graph hợp lệ với schema Workflow v2 hiện tại
   (đối chiếu `crates/lvc-app/templates/affiliate-before-after.json`), `account: ""`.
5. `public/thumbs/ugc-unboxing-demo.svg` — placeholder ảnh (SVG text, thay bằng ảnh thật sau).
6. `README.md` — hướng dẫn: tạo repo GitHub (`gh repo create`), push, import Vercel (`vercel`/
   dashboard), thêm mẫu mới, cách lấy URL để điền vào app (ở P7b).

**Nghiệm thu P7a**: chạy `vercel dev` cục bộ (hoặc `node` thuần nếu không có Vercel CLI) trả đúng
JSON cho cả 2 endpoint; README đủ để người không biết trước vẫn tự deploy được.

## P7b — Tích hợp app (ĐÃ LÀM 2026-10-11)

Quyết định rút gọn so với spec gốc (xác nhận trực tiếp qua tin nhắn người dùng): bỏ Vercel, gọi
thẳng GitHub; **không** thêm URL registry vào `AppSettings` (struct dùng chung, nhiều test ghim
cứng field — xem `MEMORY.md` của `lvc-template-registry`) — hard-code `DEFAULT_INDEX_URL`, đổi
được qua field mới `Ctx::template_registry_url` (2 nơi khởi tạo: `bootstrap.rs` production +
`test_support::ctx_with` test) nhưng CHƯA có ô nhập trong Settings UI.

1. ✅ Migration `0007_remote_templates_cache.sql` (id, name, description, category, thumbnail,
   graph, updated_at, fetched_at) + `lvc-store::RemoteTemplateRepo` (`replace_all`/`list`/`get`,
   test riêng).
2. ✅ `crates/providers/lvc-template-registry`: `RegistryClient::fetch_index()` — MỘT GET tới
   `index.json` (mảng đầy đủ, kèm `graph` — không có endpoint `/:id` riêng, khác N+1 request của
   thiết kế Vercel gốc). Test wiremock (`tests/http.rs`), không gọi GitHub thật.
3. ✅ `crates/lvc-app/src/commands/templates.rs`: nhánh `scope == "remote"` trong
   `template_list_impl`/`template_get_impl` — fetch + `replace_all` cache; lỗi mạng mà cache còn
   thì vẫn trả (không báo lỗi); cache rỗng + lỗi mạng mới trả `TEMPLATE_REMOTE_UNAVAILABLE`. `id`
   gắn tiền tố `"remote:"` (như `"public:"`) — `template_save_private`/`template_delete_private` từ
   chối id tiền tố này (`TEMPLATE_READONLY`).
4. ✅ `app/src/ipc/types.ts`: `TemplateScopeSchema` thêm `"remote"`; `commands.ts`/`store/templates.ts`
   dùng chung type `TemplateScope` thay vì literal lặp lại.
5. ✅ `TemplateGallery.tsx`: thêm tab thứ 3 "Miễn phí" (tách riêng, không gộp Public — UI còn lại
   dùng chung logic cũ vì component vốn không phân biệt scope cụ thể).
6. ✅ `format/template.ts`: thêm mã lỗi `TEMPLATE_REMOTE_UNAVAILABLE`; cập nhật test ghim
   `format_template.test.ts` (5 → 6 mã).
7. Fixtures (`template_list.json`/`template_doc.json`) KHÔNG cần sinh lại — `TemplateMeta`/
   `TemplateDoc` không đổi field, fixture xây thủ công từ struct literal, không gọi
   `template_list_impl` sống.
8. ✅ `cargo test`/`cargo clippy --all-targets -D warnings` cho `lvc-app`/`lvc-store`/
   `lvc-template-registry` xanh — gồm 1 test đối chiếu ĐÚNG graph thật sẽ push lên registry ngoài
   qua `registry.prepare` (bắt sớm nếu graph soạn tay sai kind/port). `npm run typecheck` + `npm
   run test` (356 test) xanh. Chưa chạy `scripts/check.ps1` đầy đủ (fmt/luật phụ thuộc) — xem mục
   "Việc còn lại".

**Nghiệm thu P7b**: tab "Miễn phí" hiển thị mẫu từ GitHub thật (hoặc cache khi tắt mạng);
Private/Public không đổi hành vi; không secret nào lọt ra ngoài. ⚠️ Chưa verify bằng mắt trong app
Tauri chạy thật — chỉ verify qua test tự động.

## Việc còn lại (người dùng tự làm — ngoài khả năng tự động hoá của phiên này)

1. Xác nhận `github.com/GrayPham/lvc-template-registry` đã chuyển **Public** (lúc kiểm tra có dấu
   hiệu đang Private — xem `MEMORY.md` của `lvc-template-registry`).
2. Push `index.json` + `README.md` mới (đã soạn sẵn tại `d:\LVCMedia\lvc-template-registry`, chưa
   commit) lên GitHub.
3. (Khuyến nghị) Chạy thật `cargo tauri dev --features fake-driver`, mở "Kho mẫu" → tab "Miễn phí",
   xác nhận hiển thị đúng + "Dùng mẫu" nạp được lên canvas.

## Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Repo registry bị để Private (đã gặp lúc làm P7b) | README mục 1 hướng dẫn `gh repo edit --visibility public` + cách tự kiểm bằng `raw.githubusercontent.com` |
| Registry ngoài vô tình chứa secret (ai đó PR mẫu có account thật) | Review bằng mắt mỗi PR; `account` rỗng là quy ước bắt buộc, chưa có lint tự động |
| `index.json` soạn tay sai kind/port node | Test `remote_mau_thuc_te_hop_le_voi_registry` chạy `registry.prepare` trên đúng graph sẽ push — bắt lỗi TRƯỚC khi push |
| GitHub rate-limit API không token (60 req/giờ/IP) | Dùng `raw.githubusercontent.com` (CDN, không tính vào rate-limit API) thay vì GitHub Contents API |
