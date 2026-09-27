# Node `audio_extract` - luat

Hop dong chung: `crates/lvc-node-api` | Luat repo: `CLAUDE.md` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, crates/providers/* (+ thu vien ngoai). **Cam** node khac, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri`.

## Hop dong vao/ra
- Vao: `in` — 1 video (required, khong many).
- Ra: `out` — 1 file audio (mp3/aac/wav tuy tham so).
- Tai khoan: khong (`credential: None`) — chi dung ffmpeg cuc bo.

## Tham so
- `format` (enum `mp3`|`aac`|`wav`, mac dinh `mp3`) — quyet dinh codec (`libmp3lame`/`aac`/
  `pcm_s16le`) va duoi file ra.

## Luat rieng
- `preview` THUAN (khong I/O). `run` chi cham the gioi ngoai qua `NodeCtx`.
- That bai mot phan -> `Ok` + `failure`; that bai hoan toan -> `NodeError` (`fatal` chi khi phai dung ca run).
- Khong thu `-c copy` (audio tach tu container video khac ext dich hau nhu luon phai encode lai).

## Kiem tra
- `cargo test -p node-audio-extract`
- Doi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-audio-extract` roi sinh lai fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
