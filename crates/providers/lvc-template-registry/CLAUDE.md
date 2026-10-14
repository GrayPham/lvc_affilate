# Provider `lvc-template-registry` (kho template cộng đồng)

Client đọc kho template CỘNG ĐỒNG (P7). KHÔNG biết khái niệm node, KHÔNG phụ thuộc
`lvc-node-api`/`lvc-engine`/`lvc-store`/`lvc-browser`/`lvc-app`/`tauri`/`lvc-core`. reqwest ĐƯỢC
phép (đây là crate "provider" theo nghĩa ADR-R014, dù không gắn với một node cụ thể nào — `lvc-app`
gọi thẳng, không qua `ctx.services`).

- Một GET duy nhất tới `index.json` (mảng `RemoteTemplate`) — KHÔNG auth, KHÔNG token, endpoint
  công khai. Mặc định [`client::DEFAULT_INDEX_URL`] trỏ raw content GitHub của repo mẫu
  `GrayPham/lvc-template-registry` (nhánh `master`).
- `graph` giữ nguyên `serde_json::Value` — crate này KHÔNG phụ thuộc `lvc-core`, không tự
  parse/migrate. Tầng gọi (`lvc-app::commands::templates`) chịu trách nhiệm parse thành `Graph` +
  `migrate`.
- Không có bí mật nào liên quan (không key, không cookie) — khác hẳn các provider AI
  (`lvc-google`/`lvc-seedance`/…): `RegistryError` không cần che nội dung.
- Test: `tests/http.rs` dùng `wiremock`, KHÔNG BAO GIỜ gọi `raw.githubusercontent.com` thật trong
  CI/test.
- Đổi nguồn: `RegistryClient::new(url_khac)` — cho phép khách hàng/tự host fork repo registry riêng
  mà không cần build lại code (xem câu hỏi mở P7b trong spec về việc đưa URL này vào Settings UI,
  hiện tại MVP đang hard-code mặc định, chưa cấu hình được qua UI).
