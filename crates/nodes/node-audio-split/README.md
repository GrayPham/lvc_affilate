# Tách audio↔video

Tách một video thành hai file song song: video không tiếng + file audio riêng, bằng ffmpeg.

- Vào: `in` — 1 video (bắt buộc).
- Ra: `video` — video không tiếng (`.mp4`); `audio` — file audio (`.m4a`).
- Không tham số.
- Cần `ffmpeg` trong PATH (hoặc đặt env `LVC_FFMPEG`). Không tải mạng, không tài khoản, không tốn credit.

- Loại: `audio_split`
- Hợp đồng đầy đủ: `manifest.json`
