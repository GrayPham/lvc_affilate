# `lvc-node-api` — hợp đồng node

Crate NHỎ và ỔN ĐỊNH: mọi node phụ thuộc nó, đổi API ở đây = sửa mọi node.

- Chỉ phụ thuộc `lvc-core` + serde/async-trait/tokio-util/thiserror.
- Thêm kiểu dây mới: thêm biến thể `PortType` (lvc-core) + `Value` (ở đây) cùng lúc.
- `NodeExecutor::preview` phải THUẦN (không I/O) — chạy cả khi người dùng chỉ bấm "Kiểm tra".
- `Registry::prepare` là nơi DUY NHẤT gộp validate + preview; engine và lệnh `workflow_validate`
  cùng gọi nó (không có đường kiểm hợp lệ thứ hai).
