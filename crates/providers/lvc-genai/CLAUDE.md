# Provider `lvc-genai` — luật

Client sinh TEXT dùng chung (Gemini, OpenAI) cho các node text. Xem `CLAUDE.md` gốc mục
"Node độc lập" + "Chia module bên TRONG".

## Phụ thuộc được phép
`async-trait`, `serde`, `serde_json`, `thiserror`, `reqwest` (I/O mạng — provider LÀ client dịch vụ AI).
dev: `tokio`, `wiremock`. **Cấm** `lvc-node-api`, node khác, `lvc-engine`/`lvc-store`/`lvc-browser`/
`lvc-app`/`tauri`, `sqlx`, `chromiumoxide` (check.ps1 ép bằng `cargo tree -e normal`).

## Trách nhiệm
- Khai trait dịch vụ host `TextGenHost` (node lấy qua `ctx.services.get::<Arc<dyn TextGenHost>>()`).
- Chứa client HTTP thật `GenAiClient::generate_with_key(api_key, base_url, req)`.
- KHÔNG biết khái niệm node. KHÔNG resolve secret — engine làm việc đó rồi truyền `api_key` vào.

## An toàn bí mật (BẮT BUỘC)
- `TextGenReq` chỉ mang TÊN credential, KHÔNG mang key.
- `api_key` chỉ đi qua tham số hàm, KHÔNG lưu vào struct, KHÔNG log.
- Gemini: key qua query `?key=` → dùng `e.without_url()` khi map lỗi mạng (đừng in URL).
- Lỗi (`TextGenError`) không chứa key/prompt đầy đủ; `Http.detail` cắt ngắn ≤ 300 ký tự.

## Chia file (một trách nhiệm/file)
`lib.rs` (mod + trait) · `req.rs` · `error.rs` · `prompt.rs` (THUẦN) · `client.rs` · `gemini.rs` ·
`openai.rs`. Thêm nhà cung cấp mới = thêm 1 file + 1 nhánh trong `client::generate_with_key`.

## Kiểm tra
- `cargo test -p lvc-genai` (unit + `tests/http.rs` dùng wiremock — KHÔNG gọi API thật).
