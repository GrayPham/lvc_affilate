# Ghi nhớ kỹ thuật — node `note`

Ghi những gì học được khi làm/chạy node này (quirk nhà cung cấp, mã lỗi, quyết định nhỏ). Mỗi mục
một dòng, có ngày.

- 2026-10-13: tạo khung bằng scripts/new-node.ps1.
- 2026-10-13: cố tình để `inputs`/`outputs` rỗng (khác mọi node khác trong registry, tất cả đều có
  ít nhất một cổng) — đây là node annotation thuần, không tham gia graph. `workflow_validate`
  không cần luật riêng gì cho trường hợp này: node 0 cổng không thể có cạnh nối sai nên không có gì
  để kiểm.