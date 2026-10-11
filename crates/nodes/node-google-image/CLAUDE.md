# Node `google_image`

Tạo ảnh bằng API chính thức Google (Imagen 3). Độc lập (ADR-R014): chỉ phụ thuộc `lvc-node-api` +
`lvc-google`. KHÔNG node khác / engine / store / browser / tauri.

- Vào: `prompt` (Text, bắt buộc, map ngầm mỗi dòng = 1 ảnh). Ra: `out` (Image).
- Tài khoản: `credential = { provider: "gemini", kinds: ["api_key"] }` — key Google (cùng Gemini).
- Tham số: `model` (rỗng = imagen-3.0-generate-002), `aspect` (portrait|landscape|square),
  `credential` (TÊN/label API key, bắt buộc). Resolve theo LABEL (như `script_gen`).
- Dịch vụ thật: `lvc_google::GoogleImageHost` (engine impl, resolve key → gọi Imagen → lưu file).
- Sinh manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-google-image`.
