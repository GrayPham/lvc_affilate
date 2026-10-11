# Node `seedance_image`

Tạo ảnh bằng API chính thức Seedream (Volcengine). Độc lập: chỉ `lvc-node-api` + `lvc-seedance`.

- Vào: `prompt` (Text, bắt buộc, mỗi dòng = 1 ảnh). Ra: `out` (Image).
- Tài khoản: `credential = { provider: "seedance", kinds: ["api_key"] }`. Resolve theo LABEL.
- Tham số: `model` (rỗng = mặc định; thường phải nhập endpoint id), `aspect`, `credential`.
- Dịch vụ: `lvc_seedance::SeedanceImageHost` (engine impl). Sinh manifest:
  `LVC_WRITE_FIXTURES=1 cargo test -p node-seedance-image`.
