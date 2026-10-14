---
name: rust-module-check
description: Kiểm tra code Rust trong repo lvc_affilate tuân luật kiến trúc — node độc lập (ADR-R014) và chia module nội bộ (một file = một trách nhiệm/kịch bản). Dùng trước khi commit thay đổi Rust, khi thêm node/provider mới, hoặc khi một file lib.rs/mod.rs phình to.
---

# Kiểm tra module Rust (ADR-R014 + chia module nội bộ)

Dùng skill này khi: vừa thêm/sửa code Rust trong `crates/`, chuẩn bị commit, hoặc nghi ngờ một
file đang gánh quá nhiều trách nhiệm. Luật gốc nằm ở `CLAUDE.md` tại root repo — skill này là
checklist thao tác, không thay thế việc đọc CLAUDE.md khi có xung đột.

## 1. Node độc lập (ADR-R014)

- Mỗi `crates/nodes/node-*` CHỈ được phụ thuộc (dependency thường, không tính dev-dependency):
  `lvc-node-api`, `lvc-core`, `crates/providers/*`, thư viện ngoài (crates.io).
  **Cấm**: node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `lvc-app`, `tauri`.
- `crates/providers/lvc-*` không được biết khái niệm node — không import crate node nào.
- Tài nguyên dùng chung do engine giữ → provider khai trait dịch vụ (vd `DolaVideoHost`), KHÔNG
  tự ôm state của host; engine impl trait, node lấy qua `ctx.services.get::<T>()`.
- Kiểm nhanh một crate (thay `<crate>` bằng tên thật):
  ```powershell
  cargo tree -p <crate> -e normal --prefix none --no-dedupe
  ```
  rồi soát xem có dòng nào khớp danh sách cấm ở trên không. `scripts/check.ps1` đã tự động hoá
  đúng việc này cho MỌI crate trong `crates/nodes/*` và `crates/providers/*` — luôn chạy nó trước
  khi báo "xong", đừng chỉ kiểm tay một crate rồi dừng.

## 2. Chia module bên trong (file nhỏ, một trách nhiệm)

- `lib.rs`/`mod.rs` chỉ được có `mod` + re-export/composition — nếu thấy logic (hàm dài, match
  nhiều nhánh, xử lý lỗi) nằm trực tiếp trong file này, đó là dấu hiệu cần tách ra file con.
- Mỗi nền tảng/luồng độc lập (vd từng sàn affiliate, từng provider trong node đa provider, từng
  kịch bản lỗi) nên là MỘT file riêng dưới thư mục con cùng tên nhóm chức năng — sửa/hỏng một
  luồng không lan sang luồng khác, thêm luồng mới = thêm 1 file + 1 dòng đăng ký.
- Hàm thuần (không I/O) nên tách khỏi hàm có I/O để test độc lập dễ hơn — nếu một file vừa có
  `async fn` gọi mạng vừa có logic tính toán thuần, cân nhắc tách phần tính toán ra hàm/file riêng
  kèm unit test.
- Module/crate lớn (một node, một provider, một luồng tính năng) nên có `CLAUDE.md` (luật riêng)
  + `MEMORY.md` (quirk kỹ thuật học được — mã lỗi lạ, giới hạn server, bẫy giao thức). Crate node
  mới nên tạo bằng `scripts/new-node.ps1 -Kind <kind> -Category <category>` (đã có khung sẵn)
  thay vì tự bịa cấu trúc.

## 3. Luật khác cần nhớ khi soát code Rust

- MSRV 1.85: KHÔNG dùng let-chains (`if let … && let …`). `rust-toolchain.toml` ghim 1.95 để biên
  dịch nhanh hơn, nhưng cú pháp code vẫn phải tương thích 1.85.
- `clippy::pedantic` + `-D warnings` — chạy cả hai:
  `cargo clippy --workspace --all-targets -- -D warnings` VÀ
  `cargo clippy -p lvc-app --features fake-driver --all-targets -- -D warnings`
  (feature `fake-driver` không được clippy mặc định biên dịch nên dễ mục nếu bỏ qua).
- `lvc-core` phải THUẦN: không `sqlx`/`tokio`/`reqwest`/`chromiumoxide`/`tauri`.
- Đổi hình dạng `params` của một node → tăng `manifest.version` và tự migrate trong node; `kind`
  là khoá vĩnh viễn, đổi tên phải migrate trong `lvc_core::graph::migrate`.

## Quy trình đề xuất

1. Soát các file Rust vừa đổi theo mục 2 bằng mắt — file nào > ~200-300 dòng hoặc gộp nhiều luồng
   thì đề xuất tách trước khi đi tiếp.
2. Chạy cổng đầy đủ:
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts/check.ps1
   ```
   Thêm `-SkipFrontend` nếu chỉ sửa Rust và muốn kiểm nhanh (máy không có Node/pnpm, hoặc đang
   lặp nhanh trên phần backend).
3. Chỉ báo "xong" sau khi `check.ps1` thật sự xanh — không suy luận kết quả, phải chạy thật và
   đọc output.
