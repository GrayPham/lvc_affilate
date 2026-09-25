# Ghi nhớ kỹ thuật — node `prompt`

Ghi những gì học được khi làm/chạy node này (quirk nhà cung cấp, mã lỗi, quyết định nhỏ). Mỗi mục
một dòng, có ngày.

- 2026-09-26: tách thành module độc lập (P4a, ADR-R014).
- Vượt `batch_max_scenes` trong MỘT node Prompt → lỗi báo tại chính node Prompt (P4a); vượt khi GỘP nhiều Prompt → lỗi tại node video.
