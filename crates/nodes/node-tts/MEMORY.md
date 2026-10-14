# Ghi nho ky thuat - node `tts`

Ghi nhung gi hoc duoc khi lam/chay node nay (quirk nha cung cap, ma loi, quyet dinh nho). Moi muc
mot dong, co ngay.

- 2026-10-13: tao khung bang scripts/new-node.ps1.
- 2026-10-13: hoan thien v1 — `provider`/`voice_id`/`model`/`speed`/`credential` la tham so cuoi
  (khong hardcode danh sach giong, xem CLAUDE.md). Hop dong HTTP ElevenLabs/MiniMax that nam o
  `lvc-tts` (xem MEMORY.md provider do), node nay chi dung lai qua `TtsHost`.
- 2026-10-13: CHUA chay that (chua co API key, chua duoc nguoi dung cho phep ton phi). Test dung
  FAKE `TtsHost`.