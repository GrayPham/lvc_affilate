# Thiết kế — Kho template "remote" (GitHub + Vercel, JSON + thumbnail miễn phí)

> Ngày: 2026-10-11 · Nối tiếp: `2026-09-25-node-modules-design.md` (ADR-R016 — template = graph JSON,
> Public nhúng binary) · Liên quan: P4d (kho template Public/Private đã xong), P6 (node API chính thức,
> vừa merge — `a30d848`).
> Trạng thái: **P7a + P7b đã code xong** (2026-10-11) — xem cập nhật đầy đủ trong plan
> `docs/superpowers/plans/2026-10-11-p7-remote-template-registry.md`. Một quyết định đổi so với
> bản nháp này khi làm thật: **bỏ Vercel** (ADR-R019 không còn áp dụng) — app gọi thẳng
> `raw.githubusercontent.com` một file `index.json` duy nhất, không qua lớp trung gian. Phần còn
> lại (ADR-R018, R020 tinh thần, R021, R022) vẫn đúng. Chỉ còn 2 việc thủ công phía người dùng
> (repo GitHub phải Public + push `index.json`) — xem plan mục "Việc còn lại".

## 1. Bối cảnh / câu hỏi gốc

Người dùng muốn: tạo 1 repo GitHub lưu JSON template (+ ảnh thumbnail) cho khách hàng, host một API
*tạm* trên Vercel trả JSON + thumbnail, để nút "Kho mẫu" trong app gọi lên lấy template MIỄN PHÍ mà
không cần nhúng cứng vào binary hay build lại app mỗi lần có mẫu mới.

Đã xác nhận với người dùng:
- Dữ liệu là **template workflow demo công khai** (graph JSON của app, không phải dữ liệu thật của
  khách hàng/account) → an toàn để đưa lên repo public, miễn tuân ADR-R016 ("Template không chứa bí
  mật: tham số account luôn là rỗng/`auto`").
- Giai đoạn này **chỉ soạn code + hướng dẫn** — KHÔNG tự chạy `gh repo create`/`vercel deploy` thay
  người dùng (hành động public, khó undo). Người dùng tự tạo tài khoản/push/deploy khi sẵn sàng.

## 2. Hiện trạng kho template (đã xác minh bằng cách đọc code, không suy đoán)

- `TemplateScopeSchema` hiện chỉ có `"public" | "private"` (`app/src/ipc/types.ts:844`).
- **Public**: 16 file nhúng `include_str!` trong `crates/lvc-app/src/commands/templates.rs:55-120`
  (mảng `PUBLIC`), parse lại mỗi lần gọi `template_list`/`template_get` — **không có bước nạp DB lúc
  khởi động** như spec P4d từng giả định. Format file: `{name, category, description, graph}`
  (`crates/lvc-app/templates/affiliate-before-after.json`), không có `id`/`thumbnail` (Rust tự gắn
  `id = "public:<ten-file>"`, `thumbnail = ""`).
- **Private**: bảng SQLite `templates` (`crates/lvc-store/migrations/0005_templates.sql`) —
  `id, name, description, graph, thumbnail, updated_at`; không có cột `scope`/`category` (Rust gắn
  cứng `scope="private"`, `category` mặc định "Cơ bản"). `thumbnail` là **data URL PNG ≤200KB**, do
  UI chụp canvas.
- `TemplateMeta` (field bắt buộc, `app/src/ipc/types.ts:848-857`): `id, name, description, scope,
  category, thumbnail, updatedAt`. `TemplateDoc = {meta, graph}` — `graph` dùng chung schema Workflow
  v2 toàn app.
- UI (`TemplateGallery.tsx:197-207`) render `<img src={meta.thumbnail}>` trực tiếp — chuỗi `src` bất
  kỳ (data URL hay HTTP URL thật đều chạy được, không cần sửa UI).
- Ghi/xoá bị chặn khi `id` bắt đầu `"public:"` (`TEMPLATE_READONLY`,
  `crates/lvc-app/src/commands/templates.rs:234-239,294-299`).

## 3. Quyết định kiến trúc

### ADR-R018 — Remote là nguồn thứ 3, đọc qua Rust (không fetch từ frontend)

Lý do: (a) tránh CORS trong webview Tauri, (b) cho phép cache offline trong SQLite (app vẫn mở được
kho mẫu khi mất mạng — chỉ thiếu phần "mới nhất"), (c) giữ đúng thói quen repo: HTTP ra ngoài luôn
nằm ở tầng Rust (provider/engine), frontend chỉ gọi IPC. KHÔNG thêm `fetch()` trong
`TemplateGallery.tsx`/`store/templates.ts`.

### ADR-R019 — Registry ngoài là repo RIÊNG, không phải crate trong workspace này

Repo GitHub mới (vd `lvc-template-registry`), KHÔNG nằm trong `d:\LVCMedia\lvc_affilate`:
- Nó không phải code Rust/Tauri của app — là một dự án Vercel (Node, serverless function) độc lập,
  chu kỳ phát hành khác hẳn (đẩy template mới không cần build lại app).
- Repo PUBLIC (để endpoint đọc tự do, miễn phí, không cần token) — nội dung chỉ gồm JSON template +
  ảnh thumbnail, không có gì khác đáng giữ kín.
- Cấu trúc (xem mục 4) tối giản: không framework, chỉ Vercel Serverless Functions + static files —
  Vercel tự host file trong `public/` làm CDN ảnh, khỏi cần dịch vụ ảnh riêng.

### ADR-R020 — Hợp đồng dữ liệu HTTP khớp `TemplateMeta`/`TemplateDoc`, nhưng registry KHÔNG biết `scope`

Giống triết lý "provider không biết khái niệm node" (ADR-R014 mục 3): registry ngoài chỉ trả
`{id, name, description, category, thumbnail, updatedAt, graph}` — KHÔNG có trường `scope`; Rust gắn
`scope = "remote"` khi map về `TemplateMeta`, như đã làm với Public (`scope="public"` gắn ở tầng
command, không có trong file JSON nguồn).

```
GET /api/templates            -> [{ id, name, description, category, thumbnail, updatedAt }, ...]
GET /api/templates/:id        -> { id, name, description, category, thumbnail, updatedAt, graph }
```

- `thumbnail`: URL tuyệt đối tới ảnh host trên chính Vercel project (`https://<project>.vercel.app
  /thumbs/<id>.svg|png`), KHÔNG dùng `raw.githubusercontent.com` làm nguồn chính (rate-limit thấp,
  đổi nhánh là vỡ link) — ảnh nằm trong repo GitHub (để khách xem trực tiếp trên GitHub) NHƯNG được
  Vercel serve lại qua domain của chính nó.
- `updatedAt`: epoch ms, lấy từ field `updated_at` trong file JSON nguồn (người biên tập set tay) —
  không dùng mtime file (git không giữ mtime ổn định qua clone/CI).
- `graph`: JSON Workflow v2, `account` của mọi node cần tài khoản PHẢI là `""` (đúng như public hiện
  tại) — review bằng mắt trước khi gộp PR vào registry (không có linter tự động ở P7a).

### ADR-R021 — Cache offline trong SQLite, không tái dùng bảng `templates` (private)

Bảng mới `remote_templates_cache` (migration riêng, P7b — chưa viết): `id, name, description,
category, thumbnail, graph, updated_at, fetched_at, source_url`. Tách khỏi bảng `templates` vì ngữ
nghĩa khác hẳn: private là do NGƯỜI DÙNG ghi (CRUD), remote cache là do APP ghi lại kết quả fetch
(read-through cache, người dùng không sửa/xoá trực tiếp — xoá là do dọn cache hoặc mất mạng thì vẫn
đọc được bản cũ). `template_delete_private` tiếp tục chỉ đụng bảng `templates`, không đổi.

Luồng `template_list("remote")`: gọi HTTP `GET /api/templates` (timeout ngắn, vd 3s) → nếu thành
công, upsert cache rồi trả; nếu lỗi mạng → trả thẳng cache cũ (không báo lỗi cứng, chỉ log cảnh báo)
→ nếu cache cũng rỗng mới trả lỗi cho UI hiển thị "Không tải được kho mẫu, thử lại".

### ADR-R022 — Vị trí code Rust: provider mới `lvc-template-registry`, không phải node

Theo đúng luật ADR-R014 (provider là client dịch vụ dùng chung, không biết "node"): thêm
`crates/providers/lvc-template-registry` (client HTTP thuần: `list()`, `get(id)`, map lỗi mạng) —
KHÔNG đặt trong `crates/nodes/*` vì đây không phải một loại node trên canvas, mà là tính năng của
command `template_*` hiện có (giống việc Public đọc từ binary, Private đọc từ DB). `lvc-app` gọi
provider này trực tiếp trong `commands/templates.rs` (như cách `lvc-app` gọi `lvc-store` cho Private).

## 4. Cấu trúc repo registry ngoài (đã scaffold ở P7a — xem mục 6)

```
lvc-template-registry/            (repo GitHub RIÊNG, ngoài workspace này)
  package.json                    chỉ để Vercel nhận diện Node runtime, không build step
  vercel.json                     cors header cho /api/*, route tĩnh /thumbs/*
  api/
    templates/
      index.js                    GET -> đọc templates/*.json, trả mảng meta (không trả `graph`)
      [id].js                     GET -> đọc templates/<id>.json, trả đủ `graph`
  templates/
    ugc-unboxing-demo.json        {id, name, description, category, thumbnail, updatedAt, graph}
  public/
    thumbs/
      ugc-unboxing-demo.svg       placeholder — thay bằng PNG/SVG thật khi có ảnh
  README.md                       hướng dẫn tạo repo + deploy Vercel + thêm mẫu mới + wire vào app
```

## 5. Luật bảo mật áp dụng (nhắc lại CLAUDE.md, không đổi)

1. Không có bí mật nào (cookie, API key, proxy) được phép xuất hiện trong `templates/*.json` của
   registry — mọi tham số `account`/`credential` phải để trống. Review bằng mắt mỗi PR (P7a, chưa
   có CI lint; có thể thêm script kiểm tra ở P7b nếu cần).
2. `lvc-core` vẫn THUẦN — client HTTP nằm ở `crates/providers/lvc-template-registry`, không rò vào
   `lvc-core`.
3. Request tới registry **không gửi secret nào** (không cookie, không API key app) — endpoint công
   khai đọc, không cần auth.

## 6. Việc đã làm ở P7a (xem plan `2026-10-11-p7-remote-template-registry.md`)

Scaffold repo ngoài tại `d:\LVCMedia\lvc-template-registry` (ngoài workspace `lvc_affilate`, sẵn sàng
`git init` + `gh repo create` + `vercel` khi người dùng muốn) — gồm 1 template mẫu + placeholder
thumbnail + 2 serverless function + README hướng dẫn từng bước. **Chưa đụng code app**
(`lvc_affilate` không có thay đổi nào ở P7a).

## 7. Việc CHƯA làm (P7b — cần duyệt riêng trước khi code)

- Migration SQLite `remote_templates_cache`.
- `crates/providers/lvc-template-registry` (client + test wiremock).
- Sửa `crates/lvc-app/src/commands/templates.rs`: nhánh `scope == "remote"`, map lỗi, đọc URL
  endpoint từ cấu hình (không hard-code — người dùng có thể đổi sang registry riêng của họ).
- Cấu hình URL endpoint: thêm vào `SettingsForm`/bảng settings hiện có (không tạo bảng riêng) — mặc
  định trỏ tới Vercel URL của registry mẫu, người dùng tự đổi được.
- `app/src/ipc/types.ts`: `TemplateScopeSchema` thêm `"remote"`.
- `TemplateGallery.tsx`: thêm tab thứ 3 "Miễn phí" (hoặc gộp vào Public — cần quyết định UX, xem
  mục 8 câu hỏi mở).
- Fixtures: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures` sau khi đổi command.

## 8. Câu hỏi mở (chưa chốt, hỏi người dùng trước khi làm P7b)

1. Tab UI: "Miễn phí"/"Remote" là tab riêng thứ 3, hay gộp hiển thị cùng tab "Công khai" (Public +
   Remote cùng danh sách, phân biệt bằng nhãn nhỏ)?
2. Endpoint registry: hard-code 1 URL mặc định trong code, hay để trống bắt người dùng tự điền trong
   Settings trước khi tab "remote" hiện dữ liệu?
3. Có cần giới hạn số lượng / dung lượng cache offline không (vd tối đa N mẫu, hoặc theo dung lượng)?
4. Ảnh thumbnail: SVG đủ hay cần PNG thật (ảnh chụp canvas) — nếu PNG, ai tạo ảnh (người dùng tự chụp
   rồi bỏ vào repo, hay cần script render)?

## 9. Quyết định đã chốt (2026-10-11)

- Nguồn: HTTP qua Rust, không fetch frontend (ADR-R018).
- Repo: ngoài workspace, public, không framework (ADR-R019).
- Hợp đồng dữ liệu: registry không biết `scope`, Rust tự gắn (ADR-R020).
- Cache: bảng riêng `remote_templates_cache`, không tái dùng bảng private (ADR-R021).
- Vị trí code: provider mới, không phải node (ADR-R022).
- Thứ tự: **P7a (scaffold repo ngoài) → P7b (tích hợp app, cần duyệt riêng)**.
