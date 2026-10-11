# Thiết kế — Node `veo3_image` + `veo3_video` (Google Labs Flow/Veo3)

> Ngày: 2026-09-27 · Nối tiếp: `2026-09-25-node-modules-design.md` (ADR-R014..R017, P4a/P4b/P4c xong,
> P4d treo). Đây là node mới theo dòng "P5+ node khác" đã dự kiến ở mục 10 spec đó — **không phụ
> thuộc P4d** (template/canvas Lumina), có thể làm độc lập nhờ ADR-R014.
> Nguồn tham chiếu: `A:\ToolLVC\Lvc_veo3_automation` (chỉ đọc) — xem mục 2 để biết đúng file.
> Trạng thái: **DỰ THẢO v2 — thay thế hoàn toàn v1 (2026-09-27 sáng)**. V1 dựa trên
> `api_worker.py`/`api.py`/`python_image_api.py` (REST + Bearer qua `aisandbox-pa.googleapis.com`)
> — sau khi đọc sâu hơn `src/core/flow_*.py`, phát hiện **Google đã bỏ hẳn đường REST/Bearer đó vào
> 09/2026**; đường ĐANG SỐNG DUY NHẤT là `flow.google.com` batchexecute. Mục 0 và mục 2 viết lại
> hoàn toàn theo đường mới; ADR-R018/R019 cũng viết lại. Chưa duyệt lại — xem mục 9.

## 0. Sửa 2 giả định sai (v1 đã sửa giả định #1, v2 sửa thêm giả định #2 nghiêm trọng hơn)

**#1 (giữ từ v1)**: `src/workers/api_worker.py` chỉ xử lý VIDEO. Tạo ảnh nằm ở client khác
(`python_image_api.py`).

**#2 (mới, quan trọng hơn)**: Cả `api_worker.py`/`api.py` (video, REST+Bearer) LẪN
`python_image_api.py` (ảnh, REST+Bearer) đều là **đường CŨ, ĐÃ CHẾT**. `src/core/config.py` dòng
776-778 và hàm `flow_api_enabled()` (dòng 1201-1211) ghi rõ: *"Flow.google.com batchexecute (API
mới 09/2026): tạo ảnh/video chạy qua flow.google.com. Aisandbox cũ ĐÃ BỎ HOÀN TOÀN → flow là đường
DUY NHẤT, BẮT BUỘC bật."* `src/ui/tabs/workers/image_gen_worker.py` dòng 139-154 xác nhận: worker
gọi `flow_api_enabled()` (luôn `True` trừ khi ép bằng env dev `LVC_FLOW_API=0`) rồi ưu tiên
`get_shared_flow_client()` (đường flow), chỉ rơi về `PythonImageApiClient` khi flow bị tắt bằng tay.

→ **Toàn bộ mục 2 dưới đây port từ `src/core/flow_*.py` (video) và `src/core/flow_image_api.py` +
`flow_batchexecute.py` (ảnh)** — không phải từ `api.py`/`python_image_api.py` như v1. `api.py`/
`api_worker.py`/`python_image_api.py` (REST+Bearer) chỉ còn giá trị tham khảo lịch sử (model/error
enum vẫn đúng, nhưng đường gọi thì bỏ).

## 1. Mục tiêu (không đổi so với v1)

1. Node `veo3_image`: tạo ảnh (text→ảnh, có thể kèm ảnh tham chiếu) qua Google Labs "Flow".
2. Node `veo3_video`: tạo video (T2V/I2V/interpolation/R2V) qua Veo3.
3. Theo ADR-R014 (node độc lập) và tinh thần ADR-R015 (Dola): không tự giả lập lớp chống-bot ngoài
   trình duyệt thật — xem ADR-R018 mục 3 (viết lại).
4. Tài khoản: provider `google` đã có trong `lvc-vault` (kind `login`), không cần đổi schema.

## 2. Nguồn Python thật — đường batchexecute (đọc trực tiếp `src/core/flow_*.py`, không qua subagent)

### 2.1 Giao thức `flow.google.com` batchexecute (`src/core/flow_batchexecute.py`, 524 dòng, THUẦN)

Google dùng khung RPC nội bộ "BOQ AiSandboxAngularFrontend" (không phải REST/JSON thường):

```
POST https://flow.google.com/_/AiSandboxAngularFrontend/data/batchexecute
     ?rpcids=<id>&source-path=<path>&bl=<build_label>&f.sid=<session_id>&hl=vi&_reqid=<n>&rt=c
Body (x-www-form-urlencoded): f.req=<outer_json>&at=<xsrf_token>&
  outer = [[[rpcid, inner_json_as_STRING, null, "generic"]]]   -- inner bị json-hoá 2 lần (lồng)
Auth: cookie session (flow.google.com) + `at` (XSRF, đọc từ trang, KHÔNG còn Bearer).
reCAPTCHA: nhúng THẲNG trong `inner` (mỗi rpc có 1 block cố định), KHÔNG còn ở header/clientContext.

Response (wire): )]}'\n\n<len>\n<json_array>\n<len>\n<json_array>...
  mỗi json_array chứa envelope ["wrb.fr", rpcid, "<payload_json_string>", null,null,null,"generic"]
  -> giải mã 2 lần (parse ngoài lấy chuỗi, parse trong lấy payload thật).
```

`flow_batchexecute.py` là module THUẦN (không I/O) — encode/decode + build payload theo từng rpc.
Việc GỌI thật (gắn cookie/at) do lớp khác lo (mục 2.3). Toàn bộ payload dưới đây đã được xác nhận
bằng cURL thật chụp từ trình duyệt (ghi ngày trong code — 03/09 đến 26/09/2026).

**Bảng rpcid đã biết** (dòng 27-35):

| rpcid | Việc | Đồng bộ/bất đối xứng | Xác nhận |
|---|---|---|---|
| `YhhmEf` | Text-to-video (T2V) | Bất đối xứng (operation → poll) | cURL 03/09, 05/09, 23/09 |
| `MZZa6b` | Reference-to-video (R2V, 1-6 ảnh) | Bất đối xứng | cURL 03/09, 07/09, 08/09 |
| `eb1hJf` | Image-to-video (I2V, 1 ảnh đầu) | Bất đối xứng | Port từ `veo3.js` (docs/ref) — **chưa có cURL thật xác nhận** |
| `nprQif` | Interpolation (ảnh đầu + ảnh cuối) | Bất đối xứng | Port từ `veo3.js` — **chưa có cURL thật xác nhận** |
| `p0UkFb` | Upscale video (1080p/4k) | Bất đối xứng | cURL 06/09, 15/09 |
| `jwpduf` | Poll trạng thái operation | — | cURL 03/09; **status luôn `'CAE'` dù đã xong** — KHÔNG dùng để quyết định "xong", chỉ `as29s` mới đáng tin |
| `as29s` | Lấy 1 media theo id (ảnh hoặc video) | Đồng bộ (1 lần gọi) | cURL 03/09 — nguồn sự thật DUY NHẤT cho "đã xong" (có URL = xong) |
| `maseQ` | Upload ảnh người dùng (base64) → media_id | Đồng bộ | cURL 03/09 |
| `ogiZ0b` | Tạo ảnh (GEM_PIX_2/Nano Banana) | **Đồng bộ** (trả URL ngay) | cURL 03/09, 09/09 |

**Chi tiết payload quan trọng** (đọc trực tiếp code, dòng cụ thể trong `flow_batchexecute.py`):

- `_recaptcha_block(project_id, token)` (114-118): block cố định dùng CHUNG cho ảnh+video:
  `[null,22,null,null,null,PROJECT_ID,null,null,null,null,[TOKEN,1]]`.
- **T2V** (130-156): `inner = [specs, recap_block, [workflow_id, workflow_tail]]`; mỗi spec =
  `[[null,null,[[[prompt]]]], model, aspect, null, [null,null,null,null,m1,m2]]`. `workflow_tail`
  CỐ ĐỊNH = 2 (không phải "số video" — xác nhận cURL 23/09, đừng nhầm như bản cũ). `seed_count` =
  số spec (số biến thể); model `*_lite`/Omni chỉ nhận **1 spec/lần** (đặt 2 → server trả `[]`).
- **R2V** (159-189): khác T2V ở chỗ chèn `refs` vào **vị trí [1]** của spec (đẩy `aspect` xuống
  [3]): `spec = [promptblock, refs, model, aspect, null, [ids]]`, `refs = [[null, media_id], ...]`.
  ⚠️ `seed_count` PHẢI = 1 cho model `*_lite` (không phải 2 như T2V) — xác nhận 08/09, đây là
  nguyên nhân gốc lỗi "No operations" của r2v trong bản cũ.
- **I2V** (203-224) / **Interpolation** (227-247): chèn block ảnh
  `_image_ref_block(media_id) = [null, media_id, null,null,null, CROP]` (CROP cố định
  `[null, 0.3418, 1, 0.6582]`, port từ `veo3.js`, **chưa verify bằng cURL thật của dự án này**) vào
  vị trí [4] (I2V, 1 ảnh) hoặc [4]+[5] (interpolation, ảnh đầu+cuối).
- **Aspect ratio ENUM KHÁC NHAU giữa ảnh và video** — bẫy quan trọng nhất khi port:
  - Video (`video_aspect_enum`, dòng 250-258): 9:16 (dọc) = **1**, 16:9 (ngang) = **2**.
  - Ảnh (`build_image_payload`, dòng 314-332): 9:16 (dọc) = **2**, 16:9 (ngang) = **3**.
  Không được dùng chung 1 enum cho cả hai.
- **Ảnh** (314-332): `spec = [null,null,(refs|null),seed,aspect,model,null,recap,[[[prompt]]],
  null,null,null,m1,m2]`; `inner = [null,[spec],1,recap,[workflow_id]]`. `refs` (ảnh tham chiếu đã
  upload, gắn theo PROJECT hiện tại — đổi project phải upload lại) =
  `[[media_id,null,null,null,1], ...]`. Đồng bộ — không polling.
- **Upload ảnh** (`maseQ`, 354-375): 12 phần tử
  `[recap_block, base64, mime_type, 1, null,null,null,null, file_name, null, uuid1, uuid2]`.
- **Poll media** (`as29s`, `build_media_payload`/`extract_media`, 386-413): input = `[media_id]`
  (video: operation_id; ảnh: không cần vì ảnh đồng bộ). Output: `done = bool(url)` — **không tin
  trường `status`**. Có cờ `video_only` để bỏ qua URL `/image/...` (poster) khi đợi video I2V/R2V
  còn đang render (tránh tải nhầm poster ~180KB làm "video xong").
- **Extract operation từ response T2V/R2V** (`extract_video_operation`, 489-505):
  `operation_id = payload[2][0][3][4]`, `scene_id = payload[2][0][0]`,
  `workflow_id = payload[2][0][3][5]`, `project_id = payload[2][0][-1]` — vị trí mảng CỐ ĐỊNH,
  dễ vỡ nếu Google đổi shape (đã có tiền lệ ở đường REST cũ) → port kèm test ghim y hệt payload mẫu.

### 2.2 reCAPTCHA — mint trong trình duyệt KHÁCH (ẨN DANH), KHÔNG phải trong phiên tài khoản

Đây là phát hiện quan trọng nhất cho kiến trúc Rust — đảo NGƯỢC giả định ban đầu của tôi (v1 nghĩ
là "mint+redeem trong CÙNG 1 trình duyệt đã login"). Thực tế đã đo đạc 2 lần (24-25/09/2026, cả ở
dự án ảnh riêng và xác nhận lại trên chính dự án video này — `flow_cdp_captcha/browser.py`
dòng 8-16, `flow_cdp_captcha/config.py` dòng 4-10):

> Mint token bằng code chạy TRONG phiên ĐÃ ĐĂNG NHẬP của tài khoản (dù qua extension hay Selenium
> mở thẳng profile account) → **LUÔN** `PUBLIC_ERROR_UNUSUAL_ACTIVITY`, bất kể payload đúng hay sai.
> Mint ở một trình duyệt KHÁCH — không cookie tài khoản nào, mở trang `flow.google.com/about` (chỉ
> có trang trơn, không có app Flow, không có widget reCAPTCHA của Flow để token bị ràng vào) — thì
> Google CHẤP NHẬN token đó, kể cả khi dùng để tạo video thật (`abra_t2v_4s/6s/8s` tạo được thật
> 25/09). Bấm TAY trên trình duyệt tài khoản luôn được — khác biệt là NƠI mint (khách vs tài khoản)
> + CÁCH đúc (code vs người bấm), không phải payload.

Cơ chế cụ thể (`flow_cdp_captcha/{minter,browser,config}.py`):
- Chrome KHÁCH: profile tạm dùng-một-lần (`profiles/temp_profile/guest_<uuid>`), KHÔNG headless
  (headless bị nhận diện ngay), KHÔNG minimize (đổi `visibilityState` = tín hiệu xấu) — đẩy cửa sổ
  ra ngoài màn hình bằng `--window-position=-32000,-32000` để ẩn mà vẫn "visible".
- CDP `Page.setBypassCSP` (BẮT BUỘC gọi TRƯỚC `driver.get(...)`) để vượt CSP/Trusted Types của
  `flow.google.com`, mở `https://flow.google.com/about`, tự chèn script
  `https://www.google.com/recaptcha/enterprise.js?render=<site_key>`, chờ `grecaptcha.enterprise`
  sẵn sàng, gọi `grecaptcha.enterprise.execute(site_key, {action})`.
- `SITE_KEY = '6LdsFiUsAAAAAIjVDZcuLhaHiDn5nnHVXVRQGeMV'` (hằng số cố định, dùng chung ảnh+video).
- Token dài ~2200-2700 ký tự khi hợp lệ; `MIN_TOKEN_LEN=500` — ngắn hơn coi như rác.
- 1 profile khách xoay sau tối đa 25 token hoặc 3 lần liên tiếp ra token ngắn (`should_rotate`).
- Chỉ giữ **1 trình duyệt khách sống, dùng lại tuần tự** cho nhiều lần mint (không phải nhiều
  instance chạy song song, không phải dịch vụ/extension nào) — `flow_cdp_captcha/pool.py` (tên
  file Python dùng chữ "pool" nhưng bản chất là 1 luồng thợ riêng giữ 1 trình duyệt, vì Selenium
  không thread-safe; các lời gọi mint khác xếp hàng chờ). Có hàng đợi lệnh `mint/rotate/open/
  release`, tự dọn Chrome khách mồ côi khi app tắt ngang. **Tránh dùng chữ "pool" khi mô tả lại
  bằng tiếng Việt** — dễ hiểu lầm là nhiều trình duyệt/dịch vụ chạy song song hoặc kho token đã mint
  sẵn; thực tế chỉ có 1 trình duyệt tại một thời điểm, và token luôn mint MỚI ngay lúc cần (không
  bao giờ để dành, vì token 1 lần dùng, sống ~90-120s).
- **Cookie tài khoản THẬT không bao giờ nạp vào trình duyệt khách này** — chỉ gắn ở lớp gọi request
  thật (mục 2.3), tách biệt hoàn toàn 2 ngữ cảnh.

### 2.3 Gửi request thật — "harvest" phiên account 1 lần rồi gọi HTTP thuần (KHÔNG cần trình duyệt
sống mỗi lần gọi)

`src/core/flow_account_runner.py` — đường ĐANG SỐNG mặc định (`get_shared_flow_client`, dòng
522-572, được `image_gen_worker.py` gọi trực tiếp):

1. **Mở 1 lần** (`FlowAccountRunner.open()`, dòng 110-267): Selenium mở đúng profile Chrome của
   account (đã đăng nhập Google), vào `flow.google.com/project/<id>`, poll tối đa 30s đến khi đọc
   được `window.WIZ_global_data.SNlM0e` (= `at`, XSRF token) — cũng lấy kèm `cfb2h` (= `bl`, build
   label) và `FdrFJe` (= `f.sid`, session id). Lấy cookie SỐNG của domain `flow.google.com` (gồm
   `httpOnly`) qua `driver.get_cookies()`. **Sau đó ĐÓNG trình duyệt ngay** (`runner.close()`).
2. **Cache theo email** (`_SHARED_FLOW_CLIENTS`, dòng 498-499): các lần gọi sau DÙNG LẠI
   `at`/`bl`/`fsid`/cookie đã lấy — không mở lại trình duyệt trừ khi cache trống hoặc 401.
3. **Gọi thật = HTTP thuần** (`post_mode="direct"`, dùng `curl_cffi` bên Python — tương đương
   `reqwest` bên Rust): POST tới URL batchexecute, header `Cookie: <cookie đã harvest>`, body
   `f.req=...&at=<at đã harvest>&`, reCAPTCHA token (mint riêng theo mục 2.2) nhúng trong `f.req`.
   **KHÔNG cần trình duyệt sống cho mỗi lần generate/poll/upload** — chỉ cần trình duyệt cho bước
   harvest (1) và bước mint captcha (2.2, trình duyệt KHÁC, không phải trình duyệt account).
4. Guard bắt buộc (dòng 558-564): chỉ cache khi cookie có ÍT NHẤT 1 trong
   `SID, __Secure-1PSID, __Secure-3PSID, __Secure-1PSIDTS` — có `at` nhưng cookie rỗng (AV/khoá
   profile chặn đọc cookie DB) vẫn phải coi là lỗi, không cache "half-baked" (lỗi thật đã gặp: cache
   nhiễm → mọi request sau đều 401 tới khi restart app).
5. 401 → `drop_shared_flow_client(email)` rồi harvest lại từ đầu.

**Hệ quả kiến trúc Rust quan trọng**: pha "gọi request thật" (build payload, POST, decode, extract)
là HTTP thuần — viết bằng `reqwest` trong PROVIDER crate (`lvc-veo3`) như `lvc-dola`/`lvc-genai` đã
làm, KHÔNG cần đưa vào `lvc-browser`. Chỉ 2 việc thật sự cần CDP/trình duyệt:
(a) mint reCAPTCHA (trình duyệt khách, không cần tài khoản), (b) harvest `at/bl/fsid/cookie` của
1 account (trình duyệt profile account, chỉ cần lúc đầu hoặc khi hết hạn).

### 2.4 Bảng lỗi (`src/core/flow_error_catalog.py`, 234 dòng, THUẦN) — 1 bảng dùng chung ảnh+video

Thay 5 bảng lỗi rời rạc cũ (mỗi chỗ tự chép, luôn thiếu mã mới) bằng 1 bảng + khớp theo HỌ tên mã:

- `_BANG_LOI`: mã CHÍNH XÁC (đã gặp thật trong log) → `(phân loại, câu tiếng Việt)`. Phân loại 3
  loại: `HARD` (thử lại y hệt luôn hỏng — hết hạn mức ngày/vi phạm nội dung/sai ngôn ngữ/model
  không có quyền), `TRANSIENT` (nghỉ rồi thử lại được — 403/reCAPTCHA/quá tải/nhịp), `AUTH` (hỏng ở
  PHIÊN — nạp lại cookie, retry vô ích tới khi sửa).
- `_HO_LOI`: khớp theo MẢNH TÊN (substring) cho mã CHƯA TỪNG GẶP — Google thêm mã liên tục, hầu hết
  là biến thể của họ đã biết (vd bất kỳ mã chứa `UNUSUAL_ACTIVITY` → TRANSIENT, chứa `QUOTA` → HARD,
  chứa `EXPIRED`/`UNAUTH` → AUTH).
- Cũng nhận diện mã KHÔNG PHẢI `PUBLIC_ERROR_*`: HTTP 4xx/5xx trần, mã gRPC trần trong khung
  `wrb.fr` lỗi (`["wrb.fr","<rpc>",null,null,null,[N],"generic"]` → `GRPC_<N>`), và 1 nhãn riêng của
  TOOL (`"LỌC NỘI DUNG"`) cho case `as29s` trả `NOT_FOUND` không kèm lý do (Google render xong rồi
  XOÁ hậu kiểm — không phải lỗi tool).
- Phân biệt thêm `is_prompt_fault` (lỗi do PROMPT/ẢNH → đổi account cũng hỏng y hệt) khỏi lỗi HARD
  do TÀI KHOẢN (quota/model-access-denied → đổi account chạy được) — quan trọng để engine biết khi
  nào nên xoay account vs khi nào nên dừng cả job.
- Mã đáng chú ý khi port `error.rs`: `PUBLIC_ERROR_UNUSUAL_ACTIVITY` (TRANSIENT, thiếu/sai token),
  `..._TOO_MUCH_TRAFFIC`/`HIGH_TRAFFIC` (TRANSIENT), `..._PER_MODEL_DAILY_QUOTA_REACHED`/
  `..._USER_QUOTA_REACHED`/`..._MODEL_ACCESS_DENIED` (HARD, đổi account), `..._UNSAFE_GENERATION`/
  `SEXUAL_CONTENT`/`VIOLENCE_FILTER`/`HATE_SPEECH`/`MINOR`/`MINOR_UPLOAD`/
  `PROMINENT_PEOPLE_UPLOAD`/`PROMINENT_PEOPLE_FILTER_FAILED`/`DANGER_FILTER`/`NON_ENGLISH_PROMPT`
  (HARD, sửa nội dung), `..._AUDIO_FILTERED` (TRANSIENT — video vẫn ra, chỉ mất tiếng), GRPC_3
  (INVALID_ARGUMENT, HARD — model/tham số sai HOẶC token đã dùng rồi), GRPC_5 (NOT_FOUND, HARD —
  video bị xoá hậu kiểm), GRPC_7/16 (AUTH), GRPC_8 (TRANSIENT — RESOURCE_EXHAUSTED, rate limit).

### 2.5 Ghi chú phụ

1. `python_image_api.py`/`api.py`/`api_worker.py` (REST+Bearer, `aisandbox-pa.googleapis.com`) là
   đường CHẾT theo `flow_api_enabled()` — vẫn giữ trong repo Python làm fallback dev
   (`LVC_FLOW_API=0`) nhưng **không port sang Rust** (Google đã tắt ở server, port cũng vô dụng).
   Model enum (`image_model_registry.py`, `config.py VEO3_MODELS`/`DURATION_MODELS`) VẪN dùng được
   vì model key (`GEM_PIX_2`, `veo_3_1_t2v_lite_low_priority`, `abra_r2v_8s`, …) là khái niệm của
   Google, không đổi theo đường gọi.
2. `eb1hJf` (I2V) và `nprQif` (interpolation) **chưa có cURL thật xác nhận** trong dự án này (chỉ
   port từ `veo3.js` tham khảo) — port sang Rust nhưng đánh dấu rõ "chưa verify", ưu tiên test kỹ
   T2V/R2V/ảnh (có cURL thật) trước.
3. Ảnh tham chiếu gắn THEO PROJECT — đổi project (mỗi account/mỗi lần mở mới) phải upload lại,
   không tái dùng `media_id` cũ giữa các project.
4. Video/ảnh tải về từ `flow-content.google/...`/`googleusercontent`/`usercontent` — SẠCH, không
   watermark (khác Sora). Không cần bước xoá watermark.
5. `at`/`bl`/`f.sid` là trạng thái PHIÊN của trang, không phải bí mật lâu dài như cookie — hết hạn
   nhanh hơn cookie, cần harvest lại thường xuyên hơn (không chỉ khi 401).

## 3. Quyết định kiến trúc (viết lại)

### ADR-R018 (v2) — Hai ngữ cảnh trình duyệt tách biệt: mint captcha (khách, ẩn danh) và harvest
phiên account (thật, một lần); gọi request thật bằng HTTP thuần

Không phải "mint+redeem trong CÙNG 1 trình duyệt" như v1 viết — mà NGƯỢC LẠI, mint captcha PHẢI
tách khỏi phiên account (mục 2.2), còn phiên account chỉ cần "harvest" một lần rồi gọi HTTP thuần
(mục 2.3). Rust:

- `lvc-browser::veo3::captcha` — 1 trình duyệt KHÁCH (profile tạm, không cookie tài khoản), dùng
  lại tuần tự qua CDP để mint token reCAPTCHA (xem mục 2.2 — KHÔNG gọi đây là "pool", dễ hiểu lầm
  thành nhiều trình duyệt/dịch vụ). KHÔNG cần credential Google nào — dùng chung cho MỌI account,
  MỌI node.
- `lvc-browser::veo3::harvest` — mở profile account (qua `lvc-vault`/quản lý profile như Dola),
  đọc `at/bl/fsid` + cookie, trả về `FlowSession`, rồi có thể đóng trình duyệt. Chạy lại khi cache
  hết hạn/401 — KHÔNG chạy cho mỗi lần generate.
- Request thật (build payload, POST batchexecute, decode, extract, upload, poll) là **HTTP thuần
  bằng `reqwest` trong provider `lvc-veo3`** — không phải `lvc-browser` — nhận `FlowSession` (cookie
  + at/bl/fsid, đã harvest) và `recaptcha_token` (đã mint) làm tham số, giống cách `lvc-genai` nhận
  `api_key` làm tham số (không tự resolve secret, không log secret).

Vẫn giữ đúng tinh thần ADR-R015 (không tự giả lập lớp chống-bot bằng thư viện/thuật toán riêng):
reCAPTCHA luôn mint bằng trình duyệt Chrome THẬT (CDP điều khiển Chrome thật, không phải giả lập
protocol reCAPTCHA), chỉ khác NGỮ CẢNH (khách vs account) so với giả định ban đầu.

### ADR-R019 (cập nhật 2026-09-28 — thêm 2 trait cấp NODE sau khi thấy vấn đề `local_path`) — Một
provider crate, chia file theo rpc; **4 trait host, không phải 2**

Phát hiện khi bắt tay viết node thật: `NodeCtx` KHÔNG có trường "thư mục làm việc" — `dola_video`
không tự quyết `local_path` (việc đó nằm TRONG `Engine::run_video_jobs`, phía sau `DolaVideoHost`).
Nếu để node `veo3_image`/`veo3_video` tự gọi `VeoCaptchaHost`/`VeoSessionHost` + tự lắp payload
(như bản trước của mục này viết), node sẽ phải tự đoán đường dẫn file — sai luật. Sửa: thêm
`VeoImageHost`/`VeoVideoHost` (bề mặt NODE thật, giống `DolaVideoHost`), còn `VeoCaptchaHost`/
`VeoSessionHost` chỉ còn được gọi BỞI phía impl 2 trait mới (thường là `lvc-engine`) — node không
biết 2 trait cũ tồn tại, đúng khuôn `dola_video` (node → 1 trait duy nhất → `ctx.services`).

```
crates/providers/lvc-veo3/
  src/lib.rs        mod + 4 trait host: VeoCaptchaHost (mint token, KHÔNG cần credential) ·
                     VeoSessionHost (harvest/cache phiên 1 credential_id) · VeoImageHost/VeoVideoHost
                     (bề mặt NODE — nhận Vec<ImageJob|VideoJob>, trả …JobsReport; impl tự quyết
                     local_path theo run_id/node_id/scene_index, KHÔNG nằm trong job.rs)
  src/job.rs         ImageJob/ImageJobDone/ImageJobsReport, VideoJob/VideoSource/VideoJobDone/
                     VideoJobsReport — khớp khuôn SceneJob/JobsReport của lvc-dola
  src/flow.rs        Flow: ghép VeoCaptchaHost + VeoSessionHost + client + rpc/* thành các hàm
                     "1 lần gọi xong việc" (generate_image, upload_ref_image, submit_video,
                     poll_video_once — KHÔNG tự lặp poll, để phía gọi tự quản huỷ/khoảng nghỉ —
                     download) — CHỈ dùng BỞI impl VeoImageHost/VeoVideoHost, KHÔNG phải node
  src/wire.rs        encode_freq/decode_batchexecute/build_url — khung batchexecute THUẦN
  src/error.rs       VeoError + bảng lỗi (mục 2.4): mã chính xác + họ-lỗi substring + phân loại
                     Hard/Transient/Auth + is_prompt_fault
  src/model.rs       enum model/aspect (ảnh: 2=portrait/3=landscape; video: 1=portrait/2=landscape
                     — KHÔNG gộp chung), bảng VEO3_MODELS/DURATION_MODELS (đủ dùng, không cần đủ
                     100% mọi biến thể ngày đầu)
  src/rpc/           MỘT FILE MỘT RPCID (module-split-rule): text_to_video.rs (YhhmEf) ·
                     reference_to_video.rs (MZZa6b) · image_to_video.rs (eb1hJf, đánh dấu chưa
                     verify) · interpolation.rs (nprQif, chưa verify) · media.rs (as29s) ·
                     status.rs (jwpduf) · upload.rs (maseQ) · image.rs (ogiZ0b)
                     — mỗi file: build_payload(...) THUẦN + extract(...) THUẦN, test bằng payload
                     mẫu trích nguyên văn từ flow_batchexecute.py (đã verify bằng cURL thật)
  src/client.rs      gọi HTTP thật (reqwest): nhận FlowSession + recaptcha_token, build request,
                     POST, decode, trả payload đã parse cho rpc/* xử lý tiếp
```

`lvc-browser::veo3` impl phần THỰC THI của `VeoCaptchaHost`/`VeoSessionHost` — 2 module con
`captcha.rs`/`harvest.rs` (KHÔNG phải impl trực tiếp `lvc_veo3::*` — 2 module đó tự đứng, không phụ
thuộc `lvc-veo3`, xem mục 8 cập nhật). `lvc-engine` mới là nơi impl cả 4 trait, dịch giữa
`HarvestedSession`↔`FlowSession`, dùng `flow::Flow` để impl `VeoImageHost`/`VeoVideoHost`.

### Node & credential (aspect enum sửa lại đúng theo mục 2.1)

| kind | Vào | Ra | Params chính | Tài khoản |
|---|---|---|---|---|
| `veo3_image` | `prompt: Text` (bắt buộc), `refs: Image*` | `out: Image*` | model (NARWHAL/GEM_PIX_2/HARBOR_SEAL), aspect (portrait=2/landscape=3), seed, **account** (id credential, BẮT BUỘC — chưa có "auto" như Dola, xem mục 7) | `google/login` |
| `veo3_video` | `prompt: Text*` (t2v) hoặc `refs: Image*` (r2v 1-6 ảnh) — **chưa làm i2v/interpolation ở P5** (xem mục 7) | `out: Video*` | mode (t2v/r2v), model, aspect (portrait=1/landscape=2), seed, **account** (id credential, bắt buộc) | `google/login` |

`veo3_video` map ngầm giống `dola_video`: 1 job = 1 video, `scene_index` liên tục. `veo3_image`
đồng bộ — không cần state machine polling trong node (gọi 1 lần, trả ảnh ngay). Node CHỈ gọi
`VeoImageHost`/`VeoVideoHost` qua `ctx.services` (xem ADR-R019 cập nhật) — không tự lắp payload,
không biết `VeoCaptchaHost`/`VeoSessionHost` tồn tại, giống `dola_video` chỉ biết `DolaVideoHost`.

## 4. Bố cục crate mới (cập nhật)

```
crates/
  providers/lvc-veo3/     protocol + client HTTP thuần + flow.rs orchestration + 4 trait host
                          (mục 3, ADR-R019) — reqwest CHO PHÉP (giống lvc-dola/lvc-genai); test
                          bằng payload/response mẫu trích cURL thật; KHÔNG phụ thuộc
                          lvc-node-api/lvc-browser/tauri
  lvc-browser/src/veo3/   CHỈ 2 việc cần CDP (mục 3, ADR-R018): captcha.rs (mint, KHÔNG cần
                          credential), harvest.rs (đọc at/bl/fsid/cookie 1 account) — tự đứng,
                          KHÔNG phụ thuộc lvc-veo3 (giống dola/ không phụ thuộc lvc-dola)
  nodes/node-veo3-image/  hợp đồng node ảnh — CHỈ lấy VeoImageHost qua ctx.services, không biết
                          gì khác của lvc-veo3
  nodes/node-veo3-video/  hợp đồng node video — CHỈ lấy VeoVideoHost qua ctx.services
```

`lvc-engine` là nơi DUY NHẤT phụ thuộc cả `lvc-browser` và `lvc-veo3`: impl `VeoCaptchaHost`/
`VeoSessionHost` bằng `lvc-browser::veo3` (dịch `HarvestedSession`↔`FlowSession`), rồi impl
`VeoImageHost`/`VeoVideoHost` bằng `lvc-veo3::flow::Flow` (ghép 2 host trên) + tự quyết
`local_path` theo `run_id`/`node_id`/`scene_index` (giống `Engine::run_video_jobs` quyết đường dẫn
cho Dola) — chèn cả 4 vào `Services` mỗi lần chạy.

## 5. Rủi ro (cập nhật)

| Rủi ro | Giảm thiểu |
|---|---|
| `eb1hJf`/`nprQif` (I2V/interpolation) chưa có cURL thật xác nhận — port từ `veo3.js` có thể sai vị trí trường | Làm SAU t2v/r2v/ảnh (đã verify); đánh dấu rõ "chưa verify" trong code + test; xác nhận bằng 1 lần chạy thật trước khi coi là ổn định |
| Vị trí trường trong response là index mảng cố định (`extract_video_operation` v.v.) — Google từng đổi shape 1 lần ở đường REST cũ | Test ghim (golden) bằng payload mẫu trích nguyên văn từ `flow_batchexecute.py`/cURL thật; tách rõ "gọi" khỏi "parse" |
| Guest-CDP profile bị Google từ chối ngẫu nhiên (không do tài khoản) | Port đúng cơ chế xoay profile (`should_rotate`, tối đa 25 token/profile hoặc 3 token ngắn liên tiếp) |
| Harvest phiên account cần mở Chrome thật với profile đã login Google — rủi ro khoá tài khoản nếu automation lộ liễu | Test tay đầu tiên trên 1 tài khoản Google phụ; port nguyên các cờ chống-dấu-bot đã có (`disable-blink-features=AutomationControlled`, ẩn `navigator.webdriver`) — đã kiểm chứng không gây đăng xuất thiết bị |
| `at`/`bl`/`f.sid` hết hạn nhanh hơn cookie — cache "half-baked" (at có, cookie rỗng) từng gây 401 dây chuyền tới khi restart | Port đúng guard mục 2.3 điểm 4 (bắt buộc có ≥1 cookie đăng nhập thật mới cache) |
| Model danh sách rất dài (`DURATION_MODELS`), dễ port thiếu | Không cần đủ 100% ngày đầu — chỉ cần đủ model phổ biến (`*_lite_low_priority`, `abra_*`) để node chạy được, bổ sung dần |

## 6. Lộ trình (P5, giữ khung 4 giai đoạn nhưng nội dung Task 3/4 đổi hẳn)

| Giai đoạn | Phạm vi | Nghiệm thu |
|---|---|---|
| P5a | `lvc-veo3`: `wire.rs`, `error.rs`, `model.rs`, `rpc/*` (payload/extract THUẦN, ưu tiên t2v/r2v/ảnh/upload/media/status — có cURL thật) | test đơn vị khớp payload/response mẫu trích nguyên văn Python; không gọi mạng |
| P5b | `lvc-veo3::client` (reqwest, nhận FlowSession+token) + `rpc/image_to_video`, `rpc/interpolation` (đánh dấu chưa verify) | wiremock giả `flow.google.com`; test round-trip encode→giả server→decode |
| P5c | `lvc-browser::veo3::{captcha,harvest}` (CDP thật) | chạy thử tay: (1) mint captcha bằng profile khách — KHÔNG cần tài khoản, rủi ro gần bằng 0; (2) harvest 1 account phụ — hỏi người dùng trước |
| P5d | `node-veo3-image` + `node-veo3-video` + đăng ký registry + UI tối thiểu | `cargo test` xanh; 2 node hiện trong palette, chạy được với Fake host |

## 7. Không làm ở P5

Port REST/Bearer cũ (`api.py`/`api_worker.py`/`python_image_api.py`) — đã chết ở server Google,
không có giá trị. Xuất/nhập kho tài khoản, xoá watermark (không cần), template Public/Private
(P4d), panel UI riêng kiểu Lumina (dùng form sinh từ schema trước), upscale video (`p0UkFb` — để
sau, không phải luồng tạo chính). **Chọn tài khoản tự động ("auto")** khi tham số `account` rỗng
(kiểu Dola `AccountPool`) — P5 yêu cầu `account` là id credential cụ thể, không rỗng; tự chọn theo
LRU/hạn mức cho provider bất kỳ (không riêng Dola) là việc lớn hơn phạm vi node này, để P6+.

## 8. Việc cần làm lại so với quyết định 2026-09-27 sáng (mục 9 cũ)

Quyết định "song song ảnh+video", "tên kind `veo3_image`/`veo3_video`", "có tài khoản Google phụ",
"bỏ đường A/C của Python" (mục 9 v1) **vẫn giữ nguyên** — chỉ MỘT điểm đổi: "đường B (browser-JS-
bridge)" trong quyết định #4 v1 giờ hiểu đúng là ADR-R018 v2 (2 ngữ cảnh trình duyệt tách biệt +
gọi HTTP thuần), không phải "1 trình duyệt vừa mint vừa gọi" như hiểu nhầm ban đầu. Không cần hỏi
lại người dùng — bản chất quyết định (không tự giả lập chống-bot, dùng trình duyệt thật) không đổi,
chỉ đổi CÁCH áp dụng.

## 9. Trạng thái duyệt

**Chưa hỏi lại người dùng cho v2** (khác v1 đã "đã duyệt 2026-09-27") — vì mục 8 kết luận quyết
định gốc không đổi, chỉ đổi cơ chế kỹ thuật bên trong ADR-R018 mà người dùng chưa cần quyết thêm.
Nếu người dùng muốn xem lại, trọng tâm cần xác nhận là: (a) chấp nhận kiến trúc "trình duyệt khách
ẩn danh mint captcha + harvest phiên account 1 lần + gọi HTTP thuần" thay cho "1 trình duyệt làm hết
mọi việc trong 1 context" như v1; (b) thứ tự P5a→P5d (thuần trước, CDP thật sau, giống nhịp đã làm
với Dola P4a→P4b).
