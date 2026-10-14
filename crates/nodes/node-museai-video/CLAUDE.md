# Node `museai_video` — luật

Hợp đồng chung: `crates/lvc-node-api` | Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Trạng thái (2026-10-13)

**ĐÃ cài đặt xong, chạy thật thành công nhiều lần** (kể cả qua UI thật, không chỉ script) —
`run()` gọi `Arc<dyn MuseaiVideoHost>` (engine impl ở `crates/lvc-engine/src/museai.rs`), đi hết cả
3 giai đoạn mô tả dưới đây. Mục "Tham số" dưới đây đã khoá — KHÔNG còn "dự kiến".

## Cơ chế tạo video — BA giai đoạn, CHỈ MỘT giai đoạn dùng trình duyệt

Quyết định với người dùng (2026-10-11): sinh video đi qua **API thật** của muse.ai (giao thức nội
bộ Hatch/Noise, KHÔNG phải click UI web) — nhưng **tuyệt đối không giả mạo fingerprint TLS/JA3**
(`curl_cffi`/tương đương) để né hệ thống chống-bot, vì đó là detection evasion (đã bị chặn 2 lần ở
Dola, xem `docs/superpowers/memory/p4b-no-antibot-signing.md`). Giải pháp: chỉ dùng trình duyệt
THẬT (không giả mạo gì — nó LÀ thật) cho đúng 2 việc mà muse.ai tự chống-bot ở tầng HTTP; phần còn
lại (bao gồm TOÀN BỘ việc gửi prompt/nhận video) đi thẳng qua API.

```
[1] Đăng nhập (1 lần, tay)        [2] Lấy 2 token bootstrap       [3] Sinh video (API thật)
    Trình duyệt THẬT mở ra,    →      VẪN trình duyệt đó, chỉ   →     KHÔNG còn trình duyệt.
    người dùng tự gõ OTP            fetch() 2 API nhỏ của chính      Rust nối thẳng WebSocket
    (hạ tầng đăng nhập tay          muse.ai (gateway_url/           wss://hatch.metaaivm.com/
    chung đã có — Phần A/B/C,       admission/notary) qua           v1/noise, bắt tay Noise XX
    xem `login_open_interactive`)   `lvc_browser::ops::eval_json`    (`lvc_museai::protocol::
    → cookie/profile lưu lại        (Runtime.evaluate, MAIN          transport::HatchConnection`),
                                     world) — KHÔNG phải click gì,    gửi `chat_stream`, poll
                                     chỉ đọc JSON trả về.             `history`, tải video.
```

- **Giai đoạn [1]**: tái dùng nguyên hạ tầng đăng nhập tay chung (`login_open_interactive` +
  `BrowserDriver::cookies` harvest, Phần A/B/C đã làm trong phiên 2026-10-11) — platform `"museai"`
  đã đăng ký trong `crates/lvc-app/src/commands/login/platform.rs` (`login_url("museai") =
  "https://muse.ai/"`). KHÔNG cần code riêng cho node này.
- **Giai đoạn [2]**: lý do cần trình duyệt — `auth.py` bản Python tham khảo
  (`E:\ProjectAILVC\Lvc_veo3_automation\src\core\museai\protocol\auth.py`, CHỈ ĐỌC) gọi 2 endpoint
  này (`/api/hatch/vm/wake`, `/api/hatch/token`) bằng `curl_cffi` (giả Chrome TLS fingerprint) vì
  chúng bị chống-bot chặn khi gọi bằng HTTP client thường. Thay vì giả mạo, để chính trình duyệt
  THẬT (đã đăng nhập ở [1]) tự `fetch()` 2 API này — không cần giả gì vì nó là trình duyệt thật.
  Đã viết xong ở `crate::museai::fetch_bootstrap`/`fetch_bootstrap_with_session` (`lvc-engine`,
  luôn mở trình duyệt THẬT kể cả dưới `--features fake-driver` — chưa fake được, xem doc đầu file
  `museai.rs`).
- **Giai đoạn [3]**: `lvc_museai::protocol::transport::HatchConnection::connect(ws_url, locale,
  timeout)` — WebSocket chuẩn (`tokio-tungstenite` + `rustls`, xem doc đầu `transport.rs`), bắt tay
  `Noise_XX_25519_AESGCM_SHA256` (giao thức mã hoá chuẩn công khai, KHÔNG phải chữ ký riêng nền
  tảng). Sau đó `request`/`subscribe_collect` gửi `/chat/stream` (prompt), poll `/chat/history` đợi
  video xuất hiện (`lvc_museai::protocol::client::generate_video`/`wait_for_videos`), tải file qua
  `fs.raw`/media handle (`MuseFilesystem`). Đã xác nhận chạy thật thành công nhiều lần (script +
  UI thật), xem `MEMORY.md` provider.

## Phụ thuộc được phép

lvc-node-api, lvc-core, `lvc-museai` (provider, + thư viện ngoài). **Cấm** node khác, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri` — node KHÔNG được tự mở trình duyệt; giai đoạn [1]/[2] ở trên
là việc của ENGINE (dịch vụ host, node chỉ gọi qua `ctx.services`, giống cách `VeoSessionHost`/
`VeoCaptchaHost` tách biệt node khỏi `lvc-browser` cho Veo3).

## Hợp đồng vào/ra (dự kiến — CHƯA khoá)

- Vào: `prompt: Text*` (bắt buộc) — tương tự `veo3_video`/`google_video`.
- Ra: `out: Video*`.
- Tài khoản: `museai/login` (`credential_id` — profile/cookie harvest từ giai đoạn [1], KHÔNG phải
  `api_key` vì muse.ai không cấp API key công khai cho luồng này).

## Tham số (ĐÃ khoá)

`account` (credential_id) — rỗng = tự chọn tài khoản đầu tiên còn dùng được trong kho
(`VaultMuseaiAccountResolver`, `lvc-app`), giống `dola_video` (2026-10-13). **2026-10-13 — xác
nhận qua `GET /model` thật (RPC miễn phí,
không sinh video)**: muse.ai KHÔNG có tham số API nào cho tỉ lệ khung hình/thời lượng/số lượng —
`/model` chỉ trả danh sách MODEL LLM điều phối hội thoại (`auto/auto`, `ipnext/avocado-5.16-v0`...),
không liên quan gì tới thông số video. Số lượng video = số `TextItem` nối vào cổng `prompt` (đã
`.many()`), KHÔNG cần tham số "số lượng" riêng.

`aspect`/`duration` (2026-10-13, theo yêu cầu người dùng — "dùng prompt chỉ thị là được, tool
lvc-veo3-automation tham khảo đang làm thế") — THÊM VÀO `params_schema` nhưng **KHÔNG PHẢI tham số
API thật**: `build_jobs` chỉ gói vào `VideoJob.aspect`/`duration_secs`, rồi
`lvc_museai::protocol::client::build_video_prompt` CHÈN THÊM một câu chỉ thị vào CUỐI prompt
(giống cơ chế `build_image_prompt`) trước khi gửi `/chat/stream`.

**2026-10-13 — lần chạy thật ĐẦU TIÊN cho "ra 10s dù xin 30s"**, nhưng đó là do câu chỉ thị bản đầu
tự bịa (tiếng Việt, chèn kiểu "ghi chú phụ") — SAI câu, không phải muse.ai không hỗ trợ. Đã sửa
lại đúng câu ĐÃ ĐO THẬT 10/10 lần bởi `E:\ProjectAILVC\Lvc_veo3_automation` (tham khảo, CHỈ ĐỌC,
`src/core/museai/prompt_builder.py`): câu tiếng ANH `"Output video: {N} seconds, {tỉ lệ}."` nối
CUỐI prompt — xem `lvc_museai::protocol::client::build_video_prompt`. Dữ liệu đo thật của tool đó
(không phải suy đoán): muse.ai KHÔNG tuân thủ tuyệt đối với số giây NGẮN (10/10: xin 5s vẫn ra 10s
— 1 clip Muse mặc định ~10s) nhưng VỚI 15–30s thì muse.ai TỰ ĐỘNG dựng nhiều clip ghép lại (xin
30s → 4 clip 10s ghép thành 30s, ~5 phút) — nghĩa là trường `duration` CÓ tác dụng thật cho
15–30s, chỉ không đáng tin cho số giây ngắn hơn ~10s. CHƯA tự đo lại bằng chạy thật của repo này
sau khi sửa câu — xem `MEMORY.md` provider nếu cần lịch sử đầy đủ.

## Luật riêng

- `preview` THUẦN (không I/O). `run` chỉ chạm thế giới ngoài qua `NodeCtx` — dịch vụ
  `Arc<dyn MuseaiVideoHost>` (CHƯA định nghĩa, sẽ ở `lvc_museai`, engine impl như
  `DolaVideoHost`/`VeoVideoHost`).
- Thất bại một phần → `Ok` + `failure`; thất bại hoàn toàn → `NodeError` (`fatal` chỉ khi phải
  dừng cả run).
- KHÔNG BAO GIỜ thêm `curl_cffi`/thư viện giả mạo TLS fingerprint vào `lvc-museai` hay node này,
  bất kể lý do (đã từ chối rõ ràng với người dùng 2026-10-11, xem `MEMORY.md` provider).

## Kiểm tra

- `cargo test -p node-museai-video`
- Đổi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-museai-video` rồi sinh lại fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.


## Ảnh tham chiếu (cổng `refs`) — 2026-10-14

muse.ai KHÔNG có endpoint tải ảnh riêng: web client nhúng ảnh base64 thẳng vào mảng `items` của
`/chat/stream`, ảnh đứng TRƯỚC item text. Port từ `attachments.py` của bản tham chiếu sang
`lvc_museai::protocol::attachments`.

Hai điều phải nhớ:

1. **Ngân sách ~11 MB ảnh thô cho cả yêu cầu.** Gói Noise ráp lại có hạn ~16 MB và base64 phình
   dữ liệu lên ~4/3. Vượt ngưỡng thì `/chat/stream` hỏng giữa chừng với lỗi khó hiểu, nên
   `build_items` chặn ngay với câu nói rõ người dùng cần làm gì. Mỗi ảnh ≤ 3,5 MB.

2. **KHÔNG tự nén/thu nhỏ ảnh quá cỡ** (bản Python làm bằng PIL). Workspace không có thư viện xử
   lý ảnh, kéo cả bộ codec vào chỉ để co một tấm ảnh là cái giá quá đắt cho việc người dùng tự làm
   được. Ảnh quá cỡ bị từ chối kèm hướng dẫn.

Cảnh báo giống `aspect`/`duration`: muse.ai nhận ảnh làm NGỮ CẢNH cho mô hình, không phải tham số
ràng buộc — gửi được ảnh lên KHÔNG đảm bảo giữ đúng nhân vật. Chưa kiểm chứng trên tài khoản thật.
