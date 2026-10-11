# Tạo video Veo3

Tạo video bằng Google Labs Flow (Veo3). Mỗi dòng prompt thành một video. Chọn `mode`:

- `t2v` — text-to-video, chỉ cần prompt.
- `r2v` — reference-to-video, nối thêm 1-6 ảnh vào cổng tham chiếu (dùng chung cho mọi dòng
  prompt).

Chưa hỗ trợ tạo video từ 1 ảnh đầu (`i2v`) hay nối 2 ảnh đầu/cuối (`interpolation`) — để sau.
Cần chọn sẵn tài khoản Google (`account`) — chưa hỗ trợ tự chọn tài khoản.

- Loại: `veo3_video`
- Hợp đồng đầy đủ: `manifest.json`
