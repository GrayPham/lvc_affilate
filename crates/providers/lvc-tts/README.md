# lvc-tts

Provider sinh **giọng nói** (text-to-speech) bằng AI (ElevenLabs, MiniMax) dùng chung cho node TTS.

- Trait dịch vụ: `TtsHost` (engine cấp qua `NodeCtx::services`).
- Client HTTP: `TtsClient::generate_with_key(api_key, base_url, req)` — trả bytes audio thô (mp3).
- Node dùng nó: `node-tts` (Tạo giọng đọc AI).

Cần **API key** (ElevenLabs hoặc MiniMax) lưu trong kho tài khoản (provider `elevenlabs`/`minimax`,
kind `api_key`). Bí mật do engine giữ; provider chỉ nhận key qua tham số hàm, không lưu, không log.
