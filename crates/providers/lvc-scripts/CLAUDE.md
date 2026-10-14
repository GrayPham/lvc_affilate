# Provider `lvc-scripts` — luật

Hợp đồng dịch vụ LƯU kịch bản (KHÔNG phải provider AI — không có client HTTP nào ở đây). Xem
`CLAUDE.md` gốc mục "Node độc lập" + "Chia module bên TRONG".

## Phụ thuộc được phép
`async-trait`, `thiserror`. **Cấm** `lvc-node-api`, node khác, `lvc-engine`/`lvc-store`/
`lvc-browser`/`lvc-app`/`tauri`, `sqlx` (check.ps1 ép bằng `cargo tree -e normal`).

## Trách nhiệm
- Khai trait dịch vụ host `ScriptSinkHost` (node `script_gen` lấy qua
  `ctx.services.get::<Arc<dyn ScriptSinkHost>>()`) + kiểu dữ liệu thuần `ScriptDraft`.
- KHÔNG biết khái niệm node. KHÔNG tự đụng DB — engine impl trait này bằng `lvc_store::ScriptRepo`
  (xem `crates/lvc-engine/src/scriptsink.rs`), provider chỉ là HỢP ĐỒNG.

## Kiểm tra
- `cargo test -p lvc-scripts` (chỉ có kiểu dữ liệu + trait, không có logic I/O để test riêng ở
  đây — test hành vi thật nằm ở `lvc-engine` (impl) và `node-script-gen` (gọi dịch vụ)).
