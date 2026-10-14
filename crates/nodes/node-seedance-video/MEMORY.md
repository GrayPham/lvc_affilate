# Ghi nhớ kỹ thuật — node `seedance_video`

Ghi những gì học được khi làm/chạy node này (quirk nhà cung cấp, mã lỗi, quyết định nhỏ). Mỗi mục
một dòng, có ngày.

- 2026-10-11 (hoàn thiện kịch bản affiliate đa-engine): thêm override TXT theo dòng cho
  `ratio`→`aspect`/`refs`/`account`, cùng khuôn với `node-google-video::jobs` (2 node dùng chung
  khuôn `apimedia`, xem `jobs.rs`) — `refs` chỉ lấy ẢNH ĐẦU TIÊN, `credential` tĩnh rỗng vẫn lỗi
  ngay bất kể override, lỗi thiếu ảnh `i2v` giờ theo TỪNG DÒNG thay vì một lần cho cả node.
- Không đọc `duration`/`model`/`mode` theo dòng — API Seedance không có field `duration` tương
  đương ở `VideoJob`, `model`/`mode` vẫn hợp lý dùng chung cho cả node.
