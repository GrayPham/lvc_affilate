# Node `google_video`

Tạo video bằng API chính thức Google (Veo). Độc lập (ADR-R014): chỉ phụ thuộc `lvc-node-api` +
`lvc-google`. KHÔNG node khác / engine / store / browser / tauri.

- Vào: `prompt` (Text, bắt buộc), `ref` (Image, chỉ dùng khi mode=i2v — lấy ảnh đầu tiên). Ra: `out`
  (Video).
- Tài khoản: `credential = { provider: "gemini", kinds: ["api_key"] }` — key Google (cùng Gemini).
- Tham số: `mode` (t2v|i2v), `model` (rỗng = veo-3.0-generate-preview), `aspect`, `credential`
  (TÊN/label, bắt buộc). Resolve theo LABEL.
- Dịch vụ thật: `lvc_google::GoogleVideoHost` (engine submit + poll operation + tải, tự quyết path).
- Sinh manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-google-video`.
