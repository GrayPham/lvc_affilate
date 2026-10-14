# Node `note` — luật

Hợp đồng chung: `crates/lvc-node-api` · Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Phụ thuộc được phép
lvc-node-api, lvc-core (+ thư viện ngoài). **Cấm** node khác, `lvc-engine`, `lvc-store`,
`lvc-browser`, `tauri`.

## Hợp đồng vào/ra
- Vào: không có.
- Ra: không có.
- Tài khoản: không cần (`credential: None`).

Node này KHÔNG tham gia luồng dữ liệu — không cổng nghĩa là không thể nối dây tới/từ node khác.
Đây là annotation thuần trên canvas: người tạo template gõ lưu ý/cách dùng/các bước setup, người
load template đọc node này là dùng được ngay, không cần hỏi lại.

## Tham số
`text: String` — nội dung ghi chú (textarea nhiều dòng, không parse/validate gì thêm).

## Luật riêng
- `preview`/`run` đều no-op thực sự: không đọc `ctx.services`, không ghi file, không gọi provider.
  Không tốn thời gian/credit khi chạy workflow dù có bao nhiêu node `note`.
- Cố tình để `inputs`/`outputs` rỗng thay vì gắn một cổng "giả" — giữ đúng tinh thần "node chỉ biết
  đầu vào/đầu ra thật của nó" (ADR-R014); một cổng không ai nối tới sẽ chỉ gây nhiễu UI.

## Kiểm tra
- `cargo test -p node-note`
- Đổi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-note` rồi sinh lại fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
