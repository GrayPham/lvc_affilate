# Ghi nhớ kỹ thuật — provider `lvc-tts`

Ghi quirk API (mã lỗi lạ, giới hạn server, hợp đồng đổi) — mỗi mục một dòng, có ngày.

- 2026-10-13: tạo provider. Hợp đồng xác nhận qua doc chính thức (WebFetch), KHÔNG đoán:
  - ElevenLabs: `POST https://api.elevenlabs.io/v1/text-to-speech/{voice_id}` (query
    `output_format=mp3_44100_128`), header `xi-api-key`, body `{text, model_id,
    voice_settings:{speed,...}}`, trả **raw audio bytes**.
  - MiniMax T2A v2: `POST https://api.minimax.io/v1/t2a_v2`, header `Authorization: Bearer`, body
    `{model, text, stream:false, voice_setting:{voice_id,speed,vol,pitch},
    audio_setting:{sample_rate,bitrate,format,channel}}`, trả JSON — audio **hex-encode** trong
    `data.audio`, lỗi nghiệp vụ báo qua `base_resp.status_code` (có thể dù HTTP 200).
- 2026-10-13: model mặc định — ElevenLabs `eleven_multilingual_v2` (đa ngôn ngữ, hỗ trợ tiếng
  Việt), MiniMax `speech-2.8-hd`. Nếu nhà cung cấp đổi/bỏ tên model → 404/400 (ElevenLabs) hoặc
  `base_resp.status_code` lỗi (MiniMax); sửa hằng `DEFAULT_MODEL` trong `elevenlabs.rs`/`minimax.rs`.
- 2026-10-13: KHÔNG tìm thấy tham số `GroupId` trong doc T2A v2 hiện hành (khác vài API MiniMax đời
  cũ/khác yêu cầu `GroupId` ở query) — nếu chạy live gặp lỗi auth bất thường, kiểm lại xem MiniMax
  có yêu cầu `GroupId` cho tài khoản cụ thể không trước khi nghi ngờ code.
- 2026-10-13: CHƯA chạy live (không có key, cũng chưa được người dùng cho phép tốn phí — xem luật
  #2 `CLAUDE.md` gốc). Test hiện dùng wiremock, hình dạng request/response khớp doc công khai, CHƯA
  xác nhận bằng lần gọi thật.
