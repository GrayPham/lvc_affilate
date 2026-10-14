# Tạo giọng đọc AI (TTS)

Nhận văn bản (gõ tay hoặc nối từ node khác qua cổng `prompt`), gọi ElevenLabs hoặc MiniMax để sinh
giọng đọc, xuất file audio (mp3) nối tiếp vào node khác (vd node Lưu, hoặc ghép vào video).

- Loại: `tts`
- Nhà cung cấp: ElevenLabs hoặc MiniMax (chọn ở tham số "Nhà cung cấp") — cần API key tương ứng
  trong kho "API key" (trang Cài đặt → Credentials), provider `elevenlabs`/`minimax`.
- Mỗi dòng/cảnh nối vào cổng `prompt` sinh MỘT file audio riêng (voiceover theo cảnh).
- Chi phí: tính theo ký tự của nhà cung cấp đã chọn (xem bảng giá ElevenLabs/MiniMax) — chạy thật
  tốn phí, cần xác nhận trước khi chạy hàng loạt.
- Hợp đồng đầy đủ: `manifest.json`.
