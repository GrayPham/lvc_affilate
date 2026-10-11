# P6 — Node gọi API CHÍNH THỨC (Google / Seedance / Grok) + smoke test Camoufox trung tính

Trạng thái: đang làm (bắt đầu 2026-10-01). Người dùng chốt: **node riêng từng nền tảng** (không gộp),
làm cả 3 nền tảng, Camoufox chỉ **smoke test trung tính** (KHÔNG nhắm flow.google.com).

## Mục tiêu

Người dùng tự nhập **API key + endpoint** của chính họ để tạo ảnh/video qua API CHÍNH THỨC của nhà
cung cấp — đa nền tảng. Khác hẳn node `veo3_*` (điều khiển trình duyệt Flow). Node mới gọi HTTP thuần,
không trình duyệt, không qua mặt chống bot.

## Ranh giới cứng (giữ nguyên)

- Chế độ tạo video bằng trình duyệt Camoufox vào flow.google.com: UI ghi **"Sắp có"**. KHÔNG viết
  submodule Rust điều khiển Camoufox vào Flow — cơ chế đó dựa vào qua mặt reCAPTCHA/chống bot.
- Vẫn viết **một smoke test trung tính** Rust↔Camoufox (WebDriver BiDi) mở trang vô hại (example.com)
  để đo độ ổn định điều khiển — không vào Flow, không mint captcha, không cookie tài khoản.
- Không log bí mật (API key). Key chỉ đi qua tham số hàm client, không vào struct/event/UI/log.

## Kiến trúc (ADR-R014)

### Provider crate (client dịch vụ AI, KHÔNG biết khái niệm node)

| Crate | Dịch vụ | Ảnh | Video |
|-------|---------|-----|-------|
| `crates/providers/lvc-google` | generativelanguage.googleapis.com | Imagen 3 `:predict` (base64) | Veo `:predictLongRunning` + poll |
| `crates/providers/lvc-seedance` | Volcengine Ark (`/api/v3`) | Seedream `images/generations` | Seedance `contents/generations/tasks` + poll |
| `crates/providers/lvc-grok` | api.x.ai (`/v1`, OpenAI-compatible) | `images/generations` (grok-2-image) | KHÔNG có API công khai → node báo "chưa hỗ trợ" |

Mỗi provider chia module theo trách nhiệm (luật repo): `error.rs` · `req.rs` (kiểu yêu cầu thuần) ·
`image.rs` / `video.rs` (một luồng = một file) · `client.rs` (client + điều phối) · `lib.rs` (khai
host trait + compose). Key đi qua tham số `*_with_key(api_key, base_url, req)`; endpoint lấy từ
`base_url` của credential (meta) — người dùng sửa được.

Host trait (node lấy qua `ctx.services`, engine impl): `GoogleImageHost`/`GoogleVideoHost`,
`SeedanceImageHost`/`SeedanceVideoHost`, `GrokImageHost`. Cùng khuôn `VeoImageHost`/`VeoVideoHost`
(`generate_images(run_id,node_id,jobs)->Report`, `done`/`failed` — hỏng một job không giết run).

### Node crate (6)

`node-google-image`, `node-google-video`, `node-seedance-image`, `node-seedance-video`,
`node-grok-image`, `node-grok-video` (video chỉ báo "xAI chưa có API video"). Mỗi node chỉ phụ thuộc
`lvc-node-api` + provider crate của nó. Tham số: `credential` (TÊN/label API key — resolve theo
label, KHÔNG phải id), `model` (rỗng = mặc định), `aspect`/`size`, `seed_count`/`n`.

`kind`: `google_image`, `google_video`, `seedance_image`, `seedance_video`, `grok_image`,
`grok_video`.

### Engine (impl host trait, dùng lại `TextSecretResolver`)

`crates/lvc-engine/src/apimedia.rs` (mới): mỗi host = `Engine*Host { client, resolver, downloads_dir,
cancel }`, resolve key qua `TextSecretResolver` (provider tag: Google official dùng `"gemini"`,
seedance dùng `"seedance"`, grok dùng `"grok"`), tải file về `downloads_dir/<run>_<node>_<idx>.ext`.
Chèn dịch vụ trong `dag.rs::start_run` VÔ ĐIỀU KIỆN (engine KHÔNG `match` theo kind). Client HTTP là
field Engine khởi tạo trong `new` (như `text_client`).

### Vault + frontend

- `lvc-vault`: thêm provider `"seedance"`, `"grok"` vào `PROVIDERS` + luật kind (`api_key`). Google
  official DÙNG LẠI key provider `"gemini"` (cùng API generativelanguage).
- Frontend: `CredentialProviderSchema` + `CREDENTIAL_PROVIDER_VI` + `KINDS_BY_PROVIDER` +
  `MODES_BY_PROVIDER` + `API_PROVIDERS` thêm seedance/grok. Đăng ký 6 node: `types.ts` (KindSchema +
  union + tag enum), `ports.ts` (PORTS + NODE_LABEL_VI), `rules.ts`, `refs.ts`, `store/graph.ts`
  (DEFAULT_DATA), `pages/Workflow.tsx` (nodeTypes + component), `app/src/nodes/*Node.tsx` (6 form).
  Sinh lại fixtures: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures` + manifest mỗi
  node + `app/tests/*`.

### Camoufox smoke test trung tính

`crates/lvc-browser/tests/camoufox_bidi_smoke.rs` (`#[ignore]`, live): nếu có binary Camoufox, mở
`https://example.com` qua WebDriver BiDi `ws://127.0.0.1:9333`, đọc `document.title`, đóng. Đo kết nối
+ điều khiển ổn định. KHÔNG flow.google.com, KHÔNG captcha. Tài liệu trong test ghi rõ ranh giới.

## Thứ tự làm

1. Google provider crate (template) + wiremock test. ✅ mốc chuẩn.
2. Node google_image/google_video + engine wiring + frontend + fixtures. Qua `check.ps1`.
3. Seedance + Grok provider + node (nhái template Google).
4. Vault/frontend provider mới + 6 node đăng ký đủ.
5. Camoufox smoke test trung tính + UI "Sắp có".
6. `scripts/check.ps1` XANH toàn bộ.

## Chưa chạy live

Như `lvc-genai`: chưa có key thật nên dùng wiremock; hình dạng phản hồi khớp tài liệu công khai
(knowledge cutoff 2026-01). Mỗi provider ghi `MEMORY.md` "CHƯA chạy live — cần key thật xác nhận".
Endpoint/model đều cấu hình được (base_url + param model) để người dùng tự sửa khi nhà cung cấp đổi.
