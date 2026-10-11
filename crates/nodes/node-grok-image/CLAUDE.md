# Node `grok_image`

Tạo ảnh bằng API chính thức xAI (Grok). Độc lập: chỉ `lvc-node-api` + `lvc-grok`.

- Vào: `prompt` (Text, bắt buộc, mỗi dòng = 1 ảnh). Ra: `out` (Image).
- Tài khoản: `credential = { provider: "grok", kinds: ["api_key"] }`. Resolve theo LABEL.
- Tham số: `model` (rỗng = grok-2-image-1212), `credential`. (xAI image API không nhận tỉ lệ khung.)
- Dịch vụ: `lvc_grok::GrokImageHost` (engine impl). xAI chưa có API video → không có node video Grok.
- Sinh manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-grok-image`.
