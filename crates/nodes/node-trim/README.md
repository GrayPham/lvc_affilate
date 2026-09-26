# Cắt video

Cắt MỘT video theo khoảng thời gian `[bắt đầu, kết thúc]` (giây) bằng ffmpeg.

- Vào: `in` — 1 video (bắt buộc).
- Ra: `out` — 1 video đã cắt.
- Tham số: `start` (giây, mặc định 0), `end` (giây, `0` = cắt tới hết video).
- Ưu tiên cắt nhanh (`-c copy`); nếu lỗi (khác keyframe/thông số) tự lùi về re-encode (libx264/aac).
- Cần `ffmpeg` trong PATH (hoặc đặt env `LVC_FFMPEG`). Không tải mạng, không tài khoản, không tốn credit.

- Loại: `trim`
- Hợp đồng đầy đủ: `manifest.json`
