# Node `seedance_video`

Tạo video bằng API chính thức Seedance (Volcengine). Độc lập: chỉ `lvc-node-api` + `lvc-seedance`.

- Vào: `prompt` (Text, bắt buộc), `ref` (Image, chỉ dùng khi mode=i2v — lấy ảnh đầu tiên). Ra: `out`
  (Video).
- Tài khoản: `credential = { provider: "seedance", kinds: ["api_key"] }`. Resolve theo LABEL.
- Tham số: `mode` (t2v|i2v), `model` (rỗng = mặc định; thường phải nhập endpoint id), `aspect`,
  `credential`.
- Dịch vụ: `lvc_seedance::SeedanceVideoHost` (engine submit task + poll + tải). Sinh manifest:
  `LVC_WRITE_FIXTURES=1 cargo test -p node-seedance-video`.
