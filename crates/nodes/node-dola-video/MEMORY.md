# Ghi nhớ kỹ thuật — node `dola_video`

Ghi những gì học được khi làm/chạy node này (quirk nhà cung cấp, mã lỗi, quyết định nhỏ). Mỗi mục
một dòng, có ngày.

- 2026-09-26: tách thành module độc lập (P4a, ADR-R014).
- Host hiện tại = `lvc-engine::dag::run_video_jobs` (luồng trình duyệt P2/P3). P4b thêm backend API app Android (spec 2026-09-25 §6) — param `backend: api|browser`.
- Dola: 2.0 tối đa 15s, 2.5 cho 30s; 30s chỉ qua fast/api với 2.5.
