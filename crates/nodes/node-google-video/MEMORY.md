# Ghi nhớ kỹ thuật — node `google_video`

Ghi những gì học được khi làm/chạy node này (quirk nhà cung cấp, mã lỗi, quyết định nhỏ). Mỗi mục
một dòng, có ngày.

- 2026-10-11 (hoàn thiện kịch bản affiliate đa-engine): thêm override TXT theo dòng cho
  `ratio`→`aspect`/`refs`/`account`, theo khuôn `node-dola-video::jobs`/`node-veo3-video::jobs`
  (xem `jobs.rs`). Quyết định riêng cho node này:
  - `refs` TXT chỉ lấy ẢNH ĐẦU TIÊN (không merge nhiều ảnh) — khớp đúng ngữ nghĩa "i2v chỉ nhận 1
    ảnh gốc" đã có từ trước (cổng `ref` cũng chỉ lấy ảnh đầu, phần dư bị bỏ qua lặng lẽ).
  - `credential` tĩnh rỗng vẫn lỗi NGAY (giống `account` của `veo3_video`) — override TXT chỉ thay
    thế giá trị tĩnh cho từng dòng, không miễn yêu cầu "phải đặt credential tĩnh".
  - `mode="i2v"` thiếu ảnh (cả override lẫn cổng `ref`) giờ lỗi THEO TỪNG DÒNG (gom lỗi, dòng lỗi
    bị bỏ qua, dòng khác vẫn xử lý) thay vì kiểm một lần cho cả node trước khi build — vì ảnh gốc
    giờ có thể khác nhau giữa các dòng.
- Không đọc `duration`/`model`/`mode` theo dòng — Google Veo API không có field `duration` tương
  đương, `model`/`mode` vẫn hợp lý dùng chung cho cả node.
