# Đặc tả — Node `gemini_video` + `gemini_image` (Google Gemini API chính thức, dùng API key)

> Ngày: 2026-10-01 · Trạng thái: **BẢN NHÁP — chờ người dùng duyệt (mục 11)**, chưa viết code.
> Tác giả tài liệu: viết cho bạn (chủ repo) + phiên Claude sau sẽ thực thi.
> Liên quan: node `script_gen` (đã dùng API key từ kho — khuôn mẫu chính), `veo3_image`/`veo3_video`
> (P5, cùng loại node media nhưng đi đường trình duyệt). Hai node mới ở đây **không đụng** P5.
> Nguồn API: ai.google.dev — Veo (`/docs/veo`), Nano Banana (`/docs/image-generation`),
> Pricing (`/docs/pricing`), Migrate to Interactions (`/docs/migrate-to-interactions`) — đọc 2026-10-01.

---

## 1. Mục tiêu & phạm vi

Thêm **hai node độc lập** (ADR-R014) tạo media qua **API chính thức của Google**, người dùng tự
điền **API key** (không cần trình duyệt, không cần tài khoản đăng nhập, không cần proxy):

| Node | kind (khoá vĩnh viễn) | Việc | Model mặc định |
|---|---|---|---|
| Tạo video Gemini | `gemini_video` | Veo 3.1 text→video / image→video qua REST `:predictLongRunning` | `veo-3.1-fast-generate-preview` |
| Tạo ảnh Gemini | `gemini_image` | Nano Banana (text→ảnh / sửa ảnh) qua REST `/v1beta/interactions` | `gemini-3.1-flash-image` |

**Vì sao thêm đường này** (bổ trợ, không thay P5):
- API key là đường **rõ ràng hợp lệ**, không chạm lớp chống-bot — phù hợp CLAUDE.md luật 3 & tinh
  thần ADR-R015.
- Không tốn chi phí vận hành trình duyệt, chạy được headless trên server, ổn định về lâu dài.
- Đổi lại: **tốn tiền theo giây video / theo ảnh** (mục 9) và **Veo không có free tier** — cần key đã
  bật thanh toán. Người dùng chấp nhận chi phí này khi chọn node API.

**Không làm ở bản này:** video extension/interpolation Veo (để sau), Imagen (**Google đã tắt**, dùng
Nano Banana thay), chọn key "auto" khi để trống (đã có sẵn ở resolver — xem mục 6, giữ nguyên), panel
UI riêng kiểu Lumina (dùng form sinh từ schema như các node hiện tại).

---

## 2. Dữ kiện API đã xác minh (2026-10-01) — nền để port, không đoán

### 2.1 Veo 3.1 — video (`gemini_video`)

- **Model id** (preview, đang sống): `veo-3.1-generate-preview`, `veo-3.1-fast-generate-preview`,
  `veo-3.1-lite-generate-preview`. `veo-3.0-*` đã **deprecated**.
- **Tạo** (long-running): `POST https://generativelanguage.googleapis.com/v1beta/models/{model}:predictLongRunning`
  header `x-goog-api-key: <KEY>`, `Content-Type: application/json`. Body:
  ```json
  { "instances": [{ "prompt": "…",
      "image":     { "inlineData": { "mimeType": "image/png", "data": "<base64>" } },
      "lastFrame": { "inlineData": { "mimeType": "image/png", "data": "<base64>" } },
      "referenceImages": [{ "image": { "inlineData": { "mimeType": "image/png", "data": "<base64>" } },
                            "referenceType": "asset" }] }],
    "parameters": { "aspectRatio": "16:9|9:16", "durationSeconds": 4,
      "resolution": "720p|1080p|4k", "personGeneration": "allow_all|allow_adult",
      "numberOfVideos": 1, "seed": 0 } }
  ```
  Trả về `{ "name": "<operation_name>" }`.
- **Hỏi trạng thái**: `GET https://generativelanguage.googleapis.com/v1beta/{operation_name}` (cùng
  header key). Xong khi `.done == true`; URL video ở
  **`.response.generateVideoResponse.generatedSamples[0].video.uri`**.
- **Tải**: `GET <uri>` **kèm header `x-goog-api-key`** + theo redirect (`curl -L`). Video chỉ lưu
  trên server Google **2 ngày** → tải ngay.
- **Ràng buộc**: thời lượng 4/6/8s; 8s bắt buộc khi 1080p/4k hoặc khi có ảnh tham chiếu; tối đa **3**
  ảnh tham chiếu. `personGeneration`: t2v chỉ `allow_all`; i2v/ref chỉ `allow_adult`; EU/UK/CH/MENA
  chỉ `allow_adult`. Có `seed` (không đảm bảo tất định). **Không có `negativePrompt`**.
- **Lỗi/kiểm duyệt**: "Veo 3.1 đôi khi chặn video do bộ lọc an toàn… bạn **không bị tính phí** khi
  video bị chặn." Khi `done=true` mà không có `generatedSamples` → coi là bị lọc (lỗi mềm, báo cảnh
  đó hỏng, không làm hỏng run).
- **Độ trễ**: tối thiểu ~11s, tối đa ~6 phút giờ cao điểm → nhịp poll 10s, trần ~40 lần.

### 2.2 Nano Banana — ảnh (`gemini_image`)

- **Model id** (production): `gemini-3.1-flash-image` (tới 4K), `gemini-3.1-flash-lite-image`
  (chỉ 1K), `gemini-3-pro-image` (tới 4K), `gemini-2.5-flash-image` (legacy). **Imagen đã tắt.**
- **Endpoint mới — Interactions**: `POST https://generativelanguage.googleapis.com/v1beta/interactions`
  header `x-goog-api-key`. Body text→ảnh:
  ```json
  { "model": "gemini-3.1-flash-image",
    "input": [{ "type": "text", "text": "…prompt…" }],
    "response_format": { "type": "image", "mime_type": "image/jpeg",
                         "aspect_ratio": "16:9", "image_size": "2K" } }
  ```
  Sửa ảnh / có ảnh tham chiếu: thêm phần tử `{ "type": "image", "mime_type": "image/png",
  "data": "<base64>" }` vào `input`.
- **Tỉ lệ**: 1:1, 3:2, 2:3, 3:4, 4:3, 4:5, 5:4, 9:16, 16:9, 21:9. **Kích thước**: 512px(0.5K), 1K,
  2K, 4K (chữ K hoa). Số ảnh tham chiếu tối đa khác theo model (3.1 Flash: 10 vật thể + 4 nhân vật +
  3 style; 3 Pro: 6+5+3; Flash-Lite: 14 vật thể).
- **Đọc kết quả**: base64 ảnh nằm trong mảng output (phần tử `{"type":"image","mime_type":…,"data":
  "<base64>"}`). **Rủi ro cần kiểm bằng curl thật trước khi ghim parser** (xem R2, mục 10) — tài liệu
  không in nguyên văn một response body đầy đủ, chỉ in dạng phần tử. Endpoint cũ
  `models/{model}:generateContent` **vẫn được hỗ trợ đầy đủ** (chưa có hạn tắt) → dùng làm phương án
  dự phòng/đối chứng nếu `/interactions` trả shape khác mong đợi.

### 2.3 Kho tài khoản hiện có đã sẵn sàng

- `lvc-vault` provider `gemini` đã cho `kind == "api_key"` (`vault.rs:104`); tab "API key" trên UI đã
  tạo/kiểm được key Gemini; `verify.rs` gọi `GET /v1beta/models` với key ở query để kiểm.
- Engine đã có khuôn resolve key an toàn: `TextSecretResolver` + `lvc-app` `VaultTextResolver`
  (`lease_secret` → `ResolvedSecret{api_key, base_url}`), key **chỉ ở RAM giữa resolve và gọi HTTP**,
  không ra event/log/UI. Hai node mới **dùng lại nguyên khuôn này** (mục 6).

---

## 3. Luật kiến trúc áp dụng (ADR-R014 + chia module)

Một node = một crate. Client dịch vụ chung = một provider. Engine **không** `match` theo kind. Thêm
node = crate mới + 1 dòng `registry.rs` + 1 dòng `Cargo.toml` của `lvc-app`.

```
crates/providers/lvc-gemini-media/     MỚI — client REST Veo + Nano Banana (reqwest), THUẦN với node
  src/lib.rs        mod + 2 trait host (GeminiVideoHost / GeminiImageHost) + kiểu job
  src/error.rs      GeminiMediaError (Auth 401/403 · Quota 429 · Blocked(lọc an toàn) · Http · Decode)
  src/model.rs      enum model video/ảnh + default + bảng ràng buộc (độ dài/res hợp lệ, max ref)
  src/req.rs        VideoReq / ImageReq (THUẦN) — số field tối thiểu, có #[serde] đúng tên API
  src/video.rs      build body predictLongRunning (THUẦN) + extract operation_name + extract uri
  src/image.rs      build body interactions (THUẦN) + extract base64 (+ nhánh generateContent dự phòng)
  src/client.rs     GeminiMediaClient (reqwest): submit video / poll operation / download /
                    generate image — NHẬN api_key qua tham số, KHÔNG giữ trong struct, KHÔNG log
  src/job.rs        VideoJob/VideoJobDone/VideoJobsReport, ImageJob/… (khớp khuôn lvc-veo3::job)
  CLAUDE.md MEMORY.md README.md manifest sinh tự động

crates/nodes/node-gemini-video/        MỚI — chỉ lấy GeminiVideoHost qua ctx.services
crates/nodes/node-gemini-image/        MỚI — chỉ lấy GeminiImageHost qua ctx.services
```

**Vì sao provider MỚI, không nhét vào `lvc-genai`:** `lvc-genai` là provider sinh **text** (trait
`TextGenHost`, trả `String`). Media (nhị phân, long-running, download) khác hẳn hình dạng → provider
riêng, đúng luật "một file một trách nhiệm". `lvc-gemini-media` **không** phụ thuộc `lvc-veo3` (hai
đường độc lập) — chỉ trùng *khuôn* `job.rs`, không chia sẻ code.

`lvc-engine` là nơi duy nhất impl 2 trait host: resolve key qua `TextSecretResolver` (đã có, provider
`"gemini"`), tự quyết `local_path` theo `run_id/node_id/scene_index` (giống `veo3.rs:206-211`:
`<downloads>/{run}_{node}_{scene}.mp4|.png`), rồi gọi `GeminiMediaClient`. Chèn 2 service vào
`Services` mỗi run (cạnh `VeoImageHost`/`VeoVideoHost`, `dag.rs:414`).

---

## 4. Hợp đồng node

### 4.1 `gemini_video`

| Mục | Giá trị |
|---|---|
| Vào | `prompt: Text*` (bắt buộc; mỗi `TextItem` = 1 video, map ngầm như `veo3_video`), `refs: Image*` (0-3, dùng chung mọi job — có ảnh ⇒ chế độ i2v/ref) |
| Ra | `out: Video*` |
| Params | `model` (rỗng=mặc định), `aspect` (`landscape`→16:9 / `portrait`→9:16), `duration` (4/6/8), `resolution` (720p/1080p/4k), `fast` (bool, gợi ý) , `seed` (i64), `account` (label API key `gemini`, rỗng = key bật đầu tiên) |
| Credential | provider `gemini`, kind `api_key` |

Kiểm THUẦN trong `build_jobs` (báo lỗi sớm, không gọi mạng): 8s nếu 1080p/4k hoặc có refs; refs ≤ 3;
`personGeneration` suy tự động (có refs → `allow_adult`, không → `allow_all`).

### 4.2 `gemini_image`

| Mục | Giá trị |
|---|---|
| Vào | `prompt: Text*` (mỗi item = 1 ảnh), `refs: Image*` (0..max theo model, dùng để sửa/tham chiếu) |
| Ra | `out: Image*` |
| Params | `model` (rỗng=mặc định), `aspect` (enum 10 tỉ lệ), `image_size` (0.5K/1K/2K/4K), `account` |
| Credential | provider `gemini`, kind `api_key` |

### 4.3 Checklist frontend (dễ quên — tổng hợp từ các lần thêm node trước)

Fixture Rust **không** tự lan sang frontend. Mỗi kind mới phải sửa tay:
`app/src/ipc/types.ts` (enum tag + schema params), `app/src/graph/ports.ts` (PORTS + nhãn VI),
`app/src/graph/rules.ts`, `app/src/graph/refs.ts` (switch không default), `app/src/store/graph.ts`
(DEFAULT_DATA), 3 test (`labels/connect/rules`), và **component React trong `nodeTypes`**
(`app/src/pages/Workflow.tsx`) — **không có fallback renderer**; thiếu là node tàng hình trên canvas.
Mẫu: `Veo3VideoNode.tsx`/`ScriptGenNode.tsx` (NodeShell + form inline).

---

## 5. Trình tự chạy (một job video)

```
node.run → lấy GeminiVideoHost → host.generate_videos(run_id, node_id, jobs)
  với mỗi job (tuần tự, 1 cảnh hỏng không làm hỏng cảnh khác — giống veo3.rs):
    1. resolve api_key theo job.account (TextSecretResolver, provider "gemini")
    2. (nếu có refs) đọc file → base64 từng ảnh
    3. POST :predictLongRunning → operation_name   (Blocked/Auth/Quota → fail cảnh này)
    4. lặp: GET operation (10s/lần, ≤40 lần) tới .done==true
         - có generatedSamples[0].video.uri → sang 5
         - done nhưng rỗng → "cảnh N: bị bộ lọc an toàn chặn (không mất phí)"
         - quá hạn → "cảnh N: hết giờ chờ"
    5. GET uri (header key, -L) → ghi <downloads>/{run}_{node}_{scene}.mp4
    6. done.push(scene_index, local_path)
```

Ảnh (`gemini_image`) đồng bộ: POST `/interactions` → nhận base64 → ghi `.png`/`.jpg` → xong (không
poll). Lỗi 429 → `Quota`; 401/403 → `Auth` (báo rõ "kiểm tra API key / đã bật thanh toán chưa").

---

## 6. An toàn bí mật (BẮT BUỘC, theo CLAUDE.md luật 3)

- Node chỉ mang **label** key trong job (`account`), không bao giờ mang key.
- Engine resolve qua `VaultTextResolver` đã có (`provider="gemini"`), key chỉ ở RAM; `base_url`
  override lấy từ `meta` credential (cho proxy nội bộ nếu cần).
- `GeminiMediaError` **không** chứa key, không chứa base64 ảnh, không chứa prompt đầy đủ (cắt ≤120
  ký tự khi cần log). `client.rs` dùng header `x-goog-api-key` — **không** đưa key vào URL/query (khác
  `verify.rs` chỉ để kiểm), để key không lọt vào log truy cập.
- UI chỉ thấy key dạng che `••••1234` (đã có ở tab API key).

---

## 7. Lộ trình

| GĐ | Phạm vi | Nghiệm thu |
|---|---|---|
| G1 | `lvc-gemini-media`: `model/req/video/image/error` (THUẦN) | unit test khớp body/extract với mẫu curl mục 2; không gọi mạng |
| G2 | `client.rs` (reqwest) + round-trip | wiremock giả 2 endpoint (submit→poll→download; interactions); test shape lỗi 401/429/blocked |
| G3 | 2 node crate + `job.rs` + build_jobs THUẦN + kiểm ràng buộc | `cargo test` xanh với Fake host |
| G4 | wiring `lvc-engine` (2 trait host, resolve key, local_path) + registry + Cargo.toml lvc-app | `cargo tree -e normal` sạch; `check.ps1` xanh |
| G5 | frontend (mục 4.3) + 1 template mẫu `prompt → gemini_video → save` | `pnpm typecheck/test/build` xanh; node hiện trên canvas |
| G6 | **chạy thật** (hỏi bạn trước — tốn tiền): 1 ảnh Nano Banana (rẻ ~$0.03-0.07) để verify parser, rồi 1 video Veo Lite 4s | ra file thật; ghim golden shape vào test |

Thứ tự ảnh-trước-video ở G6 vì ảnh **rẻ hơn ~100 lần** và đồng bộ → verify parser `/interactions`
(rủi ro R2) với chi phí tối thiểu trước khi tiêu tiền cho video.

---

## 8. Phụ thuộc & ranh giới (kiểm bằng `cargo tree -e normal` trong check.ps1)

- `node-gemini-video`/`node-gemini-image`: chỉ `lvc-node-api`, `lvc-gemini-media`, `serde(_json)`,
  `async-trait`. **Cấm** node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`.
- `lvc-gemini-media`: `reqwest`, `serde(_json)`, `base64`, `async-trait`, `thiserror`. **Không**
  `lvc-node-api`, **không** `lvc-veo3`, **không** `tauri`.
- `lvc-engine`: thêm impl 2 trait host trong `src/gemini_media.rs` (file mới; **không** phình
  `veo3.rs` vốn đã 578 dòng).

---

## 9. Chi phí (paid tier, xác minh 2026-10-01) — để UI cảnh báo & bạn cân nhắc

**Veo 3.1 (mỗi giây video, đã gồm audio):**
| | 720p | 1080p | 4k |
|---|---|---|---|
| Standard | $0.40 | $0.40 | $0.60 |
| Fast | $0.10 | $0.12 | $0.30 |
| Lite | $0.05 | $0.08 | — |

→ 1 video Fast 4s @720p ≈ **$0.40**; Lite 4s @720p ≈ **$0.20**. **Veo không có free tier.**

**Ảnh:** 3.1 Flash: $0.045(0.5K)/$0.067(1K)/$0.101(2K)/$0.151(4K); Flash-Lite 1K: $0.034; 3 Pro:
$0.134(1K/2K)/$0.24(4K); 2.5 Flash: $0.039.

Node sẽ hiện ước tính chi phí trong form (như Python hiện số "Generating will use…") trước khi chạy.

---

## 10. Rủi ro

| # | Rủi ro | Giảm thiểu |
|---|---|---|
| R1 | Shape operation Veo đổi (index JSON `.response.generateVideoResponse.generatedSamples[0].video.uri`) | Tách "gọi" khỏi "parse"; golden test từ curl thật ở G6; log `done` keys khi không tìm thấy uri |
| R2 | `/interactions` trả body ảnh khác tài liệu (tài liệu không in full) | G6 verify bằng ảnh rẻ trước; giữ nhánh `generateContent` dự phòng trong `image.rs`; parser thử nhiều vị trí base64 rồi báo lỗi rõ |
| R3 | Model preview đổi tên/ngừng | `model.rs` 1 nguồn hằng; lỗi 404/400 → thông điệp "model không còn, đổi trong params"; ghi ngày vào MEMORY.md |
| R4 | Tốn tiền ngoài ý muốn | Veo không free tier → form hiện ước tính; **hỏi bạn trước mỗi lần chạy live** (CLAUDE.md luật 2); test luôn dùng wiremock/Fake |
| R5 | Key lộ qua log/URL | Header-only (mục 6); error không chứa key; clippy `-D warnings` |

---

## 11. Câu hỏi cần bạn chốt trước khi code

1. **Tên kind** `gemini_video` / `gemini_image` có ổn không? (khoá vĩnh viễn, đổi sau phải migrate).
   Cân nhắc `veo_api` / `nano_banana` nếu bạn muốn tên theo sản phẩm.
2. **Model mặc định**: video `veo-3.1-fast-generate-preview` (rẻ, nhanh), ảnh `gemini-3.1-flash-image`
   — đồng ý? Hay muốn mặc định Lite cho video để rẻ nhất?
3. **Chạy thật G6**: bạn cấp 1 API key Gemini **đã bật thanh toán** để tôi verify parser (ảnh ~$0.05
   + video Lite ~$0.20)? Tôi sẽ hỏi lại ngay trước khi bấm chạy.
4. Có cần **endpoint cũ `generateContent`** làm mặc định cho ảnh (ổn định hơn, tài liệu rõ hơn) thay
   vì `/interactions` mới không?

Chốt xong 4 điểm trên là tôi bắt đầu G1.
