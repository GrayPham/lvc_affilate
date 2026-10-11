---
name: module-split-rule
description: "Hard rule — split large crates/modules into many small one-responsibility files in folders, never one giant file"
metadata:
  node_type: memory
  type: feedback
  originSessionId: b4b2bd1c-66e2-4458-b00c-9c21b20ba6f1
  modified: 2026-09-26T13:56:30.546Z
---

Trong repo lvc_seedancev2, mỗi crate/module lớn (node, provider, mảng "đăng nhập"…) phải chia thành
THƯ MỤC nhiều file nhỏ, mỗi file MỘT trách nhiệm/kịch bản ("scenario"); `lib.rs`/`mod.rs` chỉ ghép
mod, không chứa logic dài. Mỗi nền tảng/luồng là module con riêng, độc lập (vd `login/dola.rs`,
`login/gmail.rs`, `login/facebook.rs`, `login/apple.rs`, `login/gemini.rs`, `login/cookie.rs`).

**Why:** người dùng yêu cầu code dễ debug, sửa/hỏng một luồng không lan sang luồng khác; "một tính
năng = một file" thay vì dồn hết vào một file khổng lồ. Đây là LUẬT (đã ghi vào CLAUDE.md).

**How to apply:** khi thêm tính năng lớn, tạo thư mục module + nhiều file nhỏ theo luồng; thêm nền
tảng = thêm 1 file + 1 dòng đăng ký; hàm thuần tách riêng để test; giữ độc lập phụ thuộc
(`scripts/check.ps1` `cargo tree -e normal`). Xem [[p4b-no-antibot-signing]] cho ranh giới đăng nhập
nền tảng: chỉ làm đường hợp lệ (cookie/đăng nhập thủ công), KHÔNG tự động vượt chống-bot.
