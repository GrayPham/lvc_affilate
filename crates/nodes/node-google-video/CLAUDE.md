# Node `google_video`

Tạo video bằng API chính thức Google (Veo). Độc lập (ADR-R014): chỉ phụ thuộc `lvc-node-api` +
`lvc-google`. KHÔNG node khác / engine / store / browser / tauri.

- Vào: `prompt` (Text, bắt buộc), `ref` (Image, chỉ dùng khi mode=i2v — lấy ảnh đầu tiên). Ra: `out`
  (Video).
- Tài khoản: `credential = { provider: "gemini", kinds: ["api_key"] }` — key Google (cùng Gemini).
- Tham số: `mode` (t2v|i2v), `model` (rỗng = veo-3.0-generate-preview), `aspect`, `credential`
  (TÊN/label, bắt buộc). Resolve theo LABEL.
- Override TXT theo dòng (`TextItem.attrs`, khoá trùng `node_prompt::attrs`): `ratio`
  (`"16:9"|"9:16"|"1:1"` → aspect, giá trị khác IM LẶNG bỏ qua), `refs` (ẢNH ĐẦU TIÊN sau khi tách
  `\n` thay cho ảnh của cổng `ref` — vẫn đúng ngữ nghĩa "chỉ 1 ảnh"), `account` (ghi đè `credential`
  khi dòng có giá trị khác rỗng). `credential` tĩnh vẫn phải khác rỗng (lỗi `Param{field:
  "credential"}` ngay nếu không, bất kể dòng nào override). `mode="i2v"` mà dòng không có ảnh (cả
  override lẫn cổng `ref`) → lỗi `Param{field:"ref"}` RIÊNG cho dòng đó (các dòng khác vẫn xử lý).
- Dịch vụ thật: `lvc_google::GoogleVideoHost` (engine submit + poll operation + tải, tự quyết path).
- Sinh manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-google-video`.
