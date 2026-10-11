# Provider `lvc-veo3` — luật

Client Google Labs Flow/Veo3 (tạo ảnh + video) dùng chung cho `node-veo3-image`/`node-veo3-video`.
Xem `CLAUDE.md` gốc mục "Node độc lập" + "Chia module bên TRONG". Spec:
`docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md` (v2 — v1 đã bỏ, xem mục 0).

## Phụ thuộc được phép

`async-trait`, `serde`, `serde_json`, `thiserror`, `reqwest` (I/O mạng — provider LÀ client dịch
vụ AI), `uuid`, `regex`, `form_urlencoded`, `rand`. dev: `tokio`, `wiremock`. **Cấm**
`lvc-node-api`, node khác, `lvc-engine`/`lvc-store`/`lvc-browser`/`lvc-app`/`tauri` (check.ps1 ép
bằng `cargo tree -e normal`).

## Trách nhiệm

- `wire`: khung batchexecute (encode `f.req`, decode `wrb.fr`) — THUẦN.
- `error`: bảng lỗi Google (mã chính xác + họ-lỗi substring + HTTP/gRPC trần) — THUẦN.
- `model`: enum model/aspect. **Aspect ảnh và video là 2 enum SỐ KHÁC NHAU** — xem `model.rs`.
- `rpc/*`: build_payload/extract THUẦN, một file một rpcid.
- `client`: gọi HTTP thật (reqwest) — nhận `FlowSession` + `recaptcha_token` làm tham số, KHÔNG tự
  resolve secret.
- `flow`: ghép `VeoCaptchaHost`+`VeoSessionHost`+`client`+`rpc::*` thành hàm "1 lần gọi xong việc"
  (`generate_image`, `upload_ref_image`, `submit_video`, `poll_video_once`, `download`) — CHỈ dùng
  bởi impl `VeoImageHost`/`VeoVideoHost` (thường `lvc-engine`), KHÔNG dùng trực tiếp bởi node.
- `job`: kiểu `ImageJob`/`VideoJob`/…`JobsReport` cho `VeoImageHost`/`VeoVideoHost` — khớp khuôn
  `SceneJob`/`JobsReport` của `lvc-dola`.
- Khai **4 trait host** (KHÔNG phải 2 — xem ADR-R019 bản 2026-09-28):
  - `VeoCaptchaHost` (mint reCAPTCHA, KHÔNG cần credential), `VeoSessionHost` (harvest/cache phiên
    1 `credential_id`) — `lvc-browser::veo3` impl PHẦN THỰC THI (KHÔNG trực tiếp trait này —
    `lvc-browser` không phụ thuộc `lvc-veo3`, xem `lvc-browser/src/veo3/CLAUDE.md`).
  - `VeoImageHost`/`VeoVideoHost` — BỀ MẶT NODE THẬT (giống `DolaVideoHost`), nhận
    `Vec<ImageJob|VideoJob>` trả `…JobsReport`. Node CHỈ biết 2 trait này qua `ctx.services` —
    KHÔNG biết `VeoCaptchaHost`/`VeoSessionHost`/`flow::Flow` tồn tại. `local_path` trong
    `…JobDone` do PHÍA IMPL (thường `lvc-engine`) tự quyết theo `run_id`/`node_id`/`scene_index`
    (giống `Engine::run_video_jobs` — `lvc-veo3` không biết "thư mục làm việc" là gì).

## An toàn bí mật (BẮT BUỘC)

- `FlowSession.cookie_header`/`at` chỉ đi qua tham số hàm, KHÔNG log, KHÔNG in ra lỗi/`Debug` đầy
  đủ ở tầng gọi cấp cao (test/dev có thể `{:?}` tạm nhưng KHÔNG commit code log cookie).
  `recaptcha_token` cũng vậy — không log (token 1 lần dùng, lộ ra không hại bằng cookie nhưng vẫn
  không cần thiết phải log).
- Lỗi (`VeoError`) không chứa cookie/token đầy đủ — `body` lỗi HTTP có thể chứa; nếu sau này thêm
  cắt ngắn body thì làm ở `client.rs` (giống `lvc-genai::error::Http.detail` cắt ≤300 ký tự).

## Chia file (một trách nhiệm/file)

`lib.rs` (mod + 2 trait + `FlowSession`) · `wire.rs` · `error.rs` · `model.rs` · `client.rs` ·
`rpc/mod.rs` (helper dùng chung: `recaptcha_block`, `new_uuid`, `image_ref_block`,
`find_media_url`/`find_video_url`/`find_dims`, `extract_video_operation`) · `rpc/text_to_video.rs`
· `rpc/reference_to_video.rs` · `rpc/image_to_video.rs` (chưa verify cURL thật) ·
`rpc/interpolation.rs` (chưa verify cURL thật) · `rpc/image.rs` · `rpc/upload.rs` · `rpc/media.rs`
· `rpc/status.rs`.

## Kiểm tra

- `cargo test -p lvc-veo3` — unit thuần (wire/error/model/rpc/*) + `client.rs` dùng wiremock
  (KHÔNG BAO GIỜ gọi `flow.google.com` thật trong test).
- Đọc `MEMORY.md` trước khi sửa `rpc/image_to_video.rs`/`rpc/interpolation.rs` — 2 rpc này port từ
  `veo3.js` tham khảo, chưa có cURL thật của dự án xác nhận.
