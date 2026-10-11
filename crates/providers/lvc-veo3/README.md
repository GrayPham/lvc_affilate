# `lvc-veo3`

Client cho Google Labs Flow/Veo3 — nền tảng phía sau 2 node `veo3_image` (tạo ảnh) và `veo3_video`
(tạo video: text-to-video, image-to-video, reference-to-video, interpolation).

## Cách hoạt động (tóm tắt cho người dùng, chi tiết kỹ thuật xem `CLAUDE.md`/`MEMORY.md`)

1. Cần 1 tài khoản Google đã đăng nhập, lưu trong tab "Tài khoản AI" (provider `google`).
2. Mỗi lần tạo ảnh/video: hệ thống tự mint 1 mã xác minh (reCAPTCHA) qua một trình duyệt tạm
   ẩn danh (không đụng gì tới tài khoản của bạn), rồi gửi yêu cầu tới Google bằng phiên đăng nhập
   đã lưu.
3. Tạo ảnh trả kết quả ngay. Tạo video cần chờ (thường vài chục giây tới vài phút) — hệ thống tự
   kiểm tra tiến độ cho tới khi có video.

## Chi phí

Google tính theo "credit" của tài khoản — model rẻ nhất (`*_lite_low_priority`) miễn phí (0
credit) nhưng có thể chậm hơn; model nhanh/chất lượng cao tốn credit thật. Chọn model trong tham
số node.

## Rủi ro cần biết

- reCAPTCHA có thể bị Google từ chối tạm thời — hệ thống tự thử lại.
- Nếu tài khoản hết credit trong ngày, cần đổi tài khoản khác hoặc chờ Google reset.
- 2 chế độ video ít dùng (ảnh-đầu-tạo-video, nối ảnh-đầu-ảnh-cuối) chưa được kiểm chứng đầy đủ
  bằng dữ liệu thật — có thể cần điều chỉnh sau lần chạy thật đầu tiên.
