# Provider `lvc-museai` — luật

Client giao thức Hatch/Noise của `muse.ai`, port từ
`E:\ProjectAILVC\Lvc_veo3_automation\src\core\museai\protocol\` (tham khảo, **CHỈ ĐỌC — không bao
giờ sửa đường dẫn đó**, xem luật #1 CLAUDE.md gốc). Xem `CLAUDE.md` gốc mục "Node độc lập" + "Chia
module bên TRONG".

## Cơ chế tạo video/ảnh (ĐỌC TRƯỚC KHI SỬA) — BA giai đoạn, CHỈ MỘT dùng trình duyệt

Quyết định với người dùng (2026-10-11): sinh video đi qua **API thật** (giao thức Hatch/Noise nội
bộ của muse.ai) chứ không click UI web — nhưng **tuyệt đối không giả mạo fingerprint TLS/JA3**
(`curl_cffi`/"hyprcurl"/tương đương) để né chống-bot, dù đổi tên thư viện nào — đã từ chối rõ ràng
nhiều lần với người dùng, xem lý do ở mục "Bối cảnh rủi ro" dưới đây.

**2026-10-12 — ẢNH TĨNH cũng đi qua CÙNG cơ chế**: xác nhận qua test THẬT (gửi prompt xin ảnh tĩnh
qua `/chat/stream` thật) rằng muse.ai KHÔNG có endpoint/capability riêng cho ảnh — cùng
`/chat/stream`, phân biệt video/ảnh bằng Ý PROMPT (agent tự quyết `kind:"video"` hay `kind:"image"`
trong phản hồi). Xem `protocol::client::generate_image`/`build_image_prompt`.

```
[1] Đăng nhập (1 lần, tay)        [2] Lấy 2 token bootstrap       [3] Sinh video (API thật)
    Trình duyệt THẬT mở ra,    →      VẪN trình duyệt đó, chỉ   →     KHÔNG còn trình duyệt.
    người dùng tự gõ OTP            fetch() 2 API nhỏ của chính      noise.rs/wire.rs/
    → cookie/profile lưu lại        muse.ai (gateway_url/            transport.rs ở DAY —
    (hạ tầng chung, xem              admission/notary) qua            Noise XX qua WebSocket
    `node-museai-video/CLAUDE.md`)   `lvc_browser::ops::eval_json`    `wss://hatch.metaaivm.com/
                                     — KHÔNG click gì, chỉ đọc        v1/noise`.
                                     JSON trả về.
```

Chi tiết đầy đủ từng giai đoạn (bao gồm lý do kỹ thuật VÌ SAO giai đoạn [2] cần trình duyệt thật
thay vì gọi HTTP thẳng) nằm ở `crates/nodes/node-museai-video/CLAUDE.md` mục "Cơ chế tạo video" —
đọc file đó trước khi nối node vào provider này.

## Bối cảnh rủi ro

Đây là client **reverse-engineered không chính thức** cho một giao thức RPC độc quyền (ghi chú
ngay trong `protocol/__init__.py` của nguồn tham khảo: "unofficial reverse-engineered Muse.ai/Hatch
Noise-protocol client"). Dự án này có **tiền lệ bị bộ lọc an toàn Anthropic chặn 2 lần** cho việc
tương tự (port chữ ký chống-bot Dola — xem `docs/superpowers/memory/p4b-no-antibot-signing.md`).

Khi khảo sát `auth.py` (nguồn tham khảo) phát hiện nó dùng `curl_cffi.requests.AsyncSession(
impersonate="chrome", ...)` — giả mạo fingerprint TLS/JA3 của Chrome thật để qua mặt chống-bot ở
CÁC ENDPOINT HTTP của `muse.ai` (bootstrap/token/login). Đây là **detection evasion rõ ràng**,
KHÔNG port — người dùng đề xuất đổi sang "hyprcurl"/thư viện tương tự, đã từ chối vì bản chất kỹ
thuật giống hệt dù đổi tên crate. Giải pháp đã chốt: để TRÌNH DUYỆT THẬT (giai đoạn [1]/[2] ở trên)
làm việc với các endpoint có chống-bot đó — không giả mạo gì vì nó là trình duyệt thật.

`transport.rs` (cổng Hatch WebSocket, `wss://hatch.metaaivm.com`) **KHÔNG cần giải pháp này** —
nguồn tham khảo tự xác nhận dùng thư viện `websockets` THƯỜNG (không `curl_cffi`) cho cổng này,
nên port bằng `tokio-tungstenite` chuẩn (`rustls`) là đủ, không có gì phải né.

## Phụ thuộc được phép

`async-trait`, `thiserror`, `sha2`, `hmac`, `x25519-dalek`, `aes-gcm`, `rand`, `serde`,
`serde_json`, `tokio`, `tokio-tungstenite` (CHỈ bản chuẩn — `rustls-tls-webpki-roots`, KHÔNG impersonate),
`futures`, `uuid`. **Cấm tuyệt đối**: `curl_cffi`/`hyprcurl`/`boring`/`rquest`-impersonate hay bất kỳ
crate giả mạo TLS fingerprint nào (xem "Bối cảnh rủi ro"). **Cấm**: `lvc-node-api`, node khác,
`lvc-engine`/`lvc-store`/`lvc-browser`/`lvc-app`/`tauri`, `sqlx` (check.ps1 ép bằng
`cargo tree -e normal`).

## Chia module (một file = một trách nhiệm, khớp cấu trúc nguồn tham khảo)

`protocol/` — mirror `E:\...\museai\protocol\`:
- `noise.rs` — bắt tay `Noise_XX_25519_AESGCM_SHA256`. Đây là **giao thức mã hoá CHUẨN CÔNG KHAI**
  (Noise Protocol Framework, noiseprotocol.org — cùng họ WireGuard/Signal), KHÔNG phải chữ ký
  chống-bot riêng nền tảng (khác bản chất Dola X-Gorgon/Argus/Ladon). Xong, có test đầy đủ (bắt tay
  trọn vẹn với responder tự viết trong test, không phụ thuộc server thật).
- `wire.rs` — codec protobuf tối giản (varint/length-delimited, không cần `protoc`). Xong, có test.
- `errors.rs` — kiểu lỗi (`MuseError`). Xong.
- `transport.rs` — `HatchConnection` (kết nối RPC qua WebSocket + Noise, `tokio-tungstenite`
  CHUẨN). Xong, build/clippy sạch — CHƯA có test tích hợp (cần server thật hoặc mock WebSocket,
  chưa làm).
- `auth.rs` — CHỈ phần ĐỌC bootstrap (`MuseBootstrap` struct, `vm_id()`/`websocket_url()`) — KHÔNG
  có `MuseAuth`/`curl_cffi`/luồng OTP (đã thay bằng đăng nhập tay, xem "Cơ chế tạo video"). Xong,
  có test.
- `media.rs` — `VideoRef`/`ImageRef` + `extract_session_ids`/`extract_video_refs`/
  `extract_image_refs` (duyệt cây JSON tìm tham chiếu video/ảnh, THUẦN — 2 cặp SONG SONG, KHÔNG
  dùng chung struct dù hình dạng giống hệt, để không đụng code video đã chạy thật ổn định). Xong,
  có test (gồm case gộp ref trùng qua nhiều bí danh, test hình dạng phản hồi ẢNH thật đã xác nhận).
- `filesystem.rs` — `MuseFilesystem` CHỈ phần tải (`raw`/`idea_media`/`idea_media_from_url`/
  `download`) — KHÔNG port `stat`/`list`/`rename`/`delete`/`upload_file`. Ảnh tham chiếu KHÔNG đi
  qua `upload_file`: muse.ai nhúng ảnh base64 thẳng vào `items` của `/chat/stream` (xem
  `attachments.rs`), nên các hàm ghi của `filesystem.py` vẫn không cần tới. Xong, có test.
- `client.rs` — `generate_video`/`generate_image` đầu-cuối (gửi `chat_stream`, đợi video/ảnh mới
  qua poll `history`, tải xuống). Nhận `HatchConnection` ĐÃ kết nối sẵn — không tự login/bootstrap.
  `generate_image` tự gắn hướng dẫn "ảnh tĩnh, không phải video" vào CUỐI prompt qua
  `build_image_prompt` (xem ghi chú 2026-10-12 ở trên) — caller KHÔNG cần tự thêm từ khoá gì. Xong,
  có test cho phần thuần (`filename_for`/`filename_for_image`/`build_image_prompt`); phần RPC CHƯA
  có test tích hợp (cần mock WebSocket hoặc server thật).
- `attachments.rs` — ảnh tham chiếu gửi kèm prompt (port từ `attachments.py`, 2026-10-14). Ảnh đi
  base64 TRONG `items` của `/chat/stream`, đứng TRƯỚC item text. Ngân sách ~11 MB ảnh thô cho cả
  yêu cầu (gói Noise ~16 MB, base64 phình ~4/3); mỗi ảnh ≤ 3,5 MB. KHÔNG port phần nén/thu nhỏ
  bằng PIL — workspace không có thư viện xử lý ảnh, ảnh quá cỡ bị từ chối kèm hướng dẫn thay vì
  kéo cả bộ codec vào. Xong, có test. Không port
  `routes.py`'s `ROUTES` dict — bản thân code tham khảo (`client.py`/`filesystem.py`) cũng không
  dùng nó, chỉ hardcode path literal; chỉ giữ lại hằng `CHAT_CAPABILITIES` (inline trong
  `client.rs`).

## Luật riêng

- KHÔNG dùng crate `snow` (thư viện Noise tổng quát) — cố ý viết tay từng bước để khớp CHÍNH XÁC
  bundle đã bắt được, dễ so khớp lại với Python khi debug (ghi chú gốc cũng nêu lý do này).
- Mọi hàm `pub fn` trả `Result` phải có `# Errors`; hàm có `.expect()`/có thể panic phải có
  `# Panics` HOẶC tái cấu trúc để không panic (ưu tiên cách 2 — xem cách `noise.rs`/`wire.rs` dùng
  `try_from(..).unwrap_or(..)` thay vì `as`/`.expect()` ở input không tin cậy).
- KHÔNG log/in bất kỳ khoá riêng, session token, OTP, hay payload giải mã nào (luật #3 CLAUDE.md
  gốc) — kể cả trong test, chỉ dùng dữ liệu giả.

## Kiểm tra

- `cargo test -p lvc-museai`, `cargo clippy -p lvc-museai --all-targets -- -D warnings`,
  `cargo tree -e normal -p lvc-museai` (xác nhận không dính `lvc-store`/`lvc-engine`/`sqlx`/bất kỳ
  crate impersonate nào).
