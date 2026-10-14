# Node `seedance_video`

Tạo video bằng API chính thức Seedance (Volcengine). Độc lập: chỉ `lvc-node-api` + `lvc-seedance`.

- Vào: `prompt` (Text, bắt buộc), `ref` (Image, chỉ dùng khi mode=i2v — lấy ảnh đầu tiên). Ra: `out`
  (Video).
- Tài khoản: `credential = { provider: "seedance", kinds: ["api_key"] }`. Resolve theo LABEL.
- Tham số: `mode` (t2v|i2v), `model` (rỗng = mặc định; thường phải nhập endpoint id), `aspect`,
  `credential`.
- Override TXT theo dòng (`TextItem.attrs`, khoá trùng `node_prompt::attrs`): `ratio`
  (`"16:9"|"9:16"|"1:1"` → aspect, giá trị khác IM LẶNG bỏ qua), `refs` (ẢNH ĐẦU TIÊN sau khi tách
  `\n` thay cho ảnh của cổng `ref`), `account` (ghi đè `credential` khi dòng có giá trị khác rỗng).
  `credential` tĩnh vẫn phải khác rỗng (lỗi ngay nếu không, bất kể dòng nào override). `mode="i2v"`
  mà dòng không có ảnh (cả override lẫn cổng `ref`) → lỗi `Param{field:"ref"}` RIÊNG cho dòng đó.
- Dịch vụ: `lvc_seedance::SeedanceVideoHost` (engine submit task + poll + tải). Sinh manifest:
  `LVC_WRITE_FIXTURES=1 cargo test -p node-seedance-video`.
