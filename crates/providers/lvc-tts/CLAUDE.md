# Provider `lvc-tts` — luật

Client sinh AUDIO (text-to-speech) dùng chung (ElevenLabs, MiniMax) cho node `tts`. Xem `CLAUDE.md`
gốc mục "Node độc lập" + "Chia module bên TRONG".

## Phụ thuộc được phép
`async-trait`, `serde`, `serde_json`, `thiserror`, `reqwest` (I/O mạng — provider LÀ client dịch vụ
AI), `hex` (giải mã audio hex-encode của MiniMax). dev: `tokio`, `wiremock`. **Cấm**
`lvc-node-api`, node khác, `lvc-engine`/`lvc-store`/`lvc-browser`/`lvc-app`/`tauri`, `sqlx`,
`chromiumoxide` (check.ps1 ép bằng `cargo tree -e normal`).

## Trách nhiệm
- Khai trait dịch vụ host `TtsHost` (node lấy qua `ctx.services.get::<Arc<dyn TtsHost>>()`) —
  nhận `Vec<TtsJob>`, trả `TtsJobsReport` (thành công một phần, `failed` không chặn job khác).
- Chứa client HTTP thật `TtsClient::generate_with_key(api_key, base_url, req) -> Vec<u8>` (audio
  bytes thô, luôn mp3).
- KHÔNG biết khái niệm node. KHÔNG resolve secret, KHÔNG tự quyết `local_path` — đó là việc của
  impl `TtsHost` (`lvc-engine`, xem `crates/lvc-engine/src/tts.rs`), giống hệt cách `lvc_museai`
  tách `VideoJob`/host khỏi `local_path`.

## An toàn bí mật (BẮT BUỘC)
- `TtsJob`/`SynthReq` chỉ mang TÊN credential (`credential_id`), KHÔNG mang key.
- `api_key` chỉ đi qua tham số hàm, KHÔNG lưu vào struct, KHÔNG log.
- Lỗi (`TtsError`) không chứa key/text đầy đủ; `Http.detail` cắt ngắn ≤ 300 ký tự.

## Hai hợp đồng API khác NHAU — đọc kỹ trước khi sửa

- **ElevenLabs** (`elevenlabs.rs`): `POST /v1/text-to-speech/{voice_id}?output_format=mp3_44100_128`,
  key qua header `xi-api-key`. Phản hồi THÀNH CÔNG là **bytes audio thô** (không phải JSON) — không
  thể gọi `resp.text()` trước khi biết status 2xx (xem `client::error_for_status` — tách riêng khỏi
  `client::check_status` vì lý do này).
- **MiniMax T2A v2** (`minimax.rs`): `POST /v1/t2a_v2`, key qua header `Authorization: Bearer`.
  Phản hồi LUÔN là JSON; audio nằm **hex-encode** trong `data.audio`. **Lỗi nghiệp vụ báo qua
  `base_resp.status_code` dù HTTP status vẫn 200** — PHẢI kiểm `status_code != 0` sau khi đã qua
  `check_status` (HTTP-level), không chỉ dựa vào mã HTTP.

## Chia file (một trách nhiệm/file)
`lib.rs` (mod + trait `TtsHost`) · `req.rs` (`Provider` + `SynthReq`, KHÔNG mang `scene_index`/
`credential_id`) · `job.rs` (`TtsJob`/`TtsJobDone`/`TtsJobsReport`, mirror `lvc_museai::job`) ·
`error.rs` · `client.rs` (dispatch + `check_status`/`error_for_status`) · `elevenlabs.rs` ·
`minimax.rs`. Thêm nhà cung cấp mới = thêm 1 file + 1 nhánh trong `client::TtsClient::generate_with_key`
+ 1 biến thể `Provider`.

## Kiểm tra
- `cargo test -p lvc-tts` (unit + `tests/http.rs` dùng wiremock — KHÔNG gọi API thật).
