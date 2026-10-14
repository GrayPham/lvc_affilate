# Ghi chú

Node ghi chú tĩnh đặt trên canvas — không tham gia luồng dữ liệu, không cổng vào/ra, không tốn
credit khi chạy. Dùng để viết lưu ý, cách dùng, hoặc các bước cần setup trước khi chạy một
template (ví dụ: "điền API key ở node X trước", "chọn thư mục lưu ở node Lưu", "đổi chủ đề ở node
Kịch bản AI rồi bấm Chạy"). Khi người khác mở template, đọc node này là biết cách dùng ngay, không
cần hỏi lại người tạo.

- Loại: `note`
- Tham số: `text` (nội dung ghi chú, nhiều dòng)
- Hợp đồng đầy đủ: `manifest.json`
