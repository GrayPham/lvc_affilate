# Node `audio_split` - luat

Hop dong chung: `crates/lvc-node-api` | Luat repo: `CLAUDE.md` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, crates/providers/* (+ thu vien ngoai). **Cam** node khac, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri`.

## Hop dong vao/ra
- Vao: `in` — 1 video (required, khong many).
- Ra: `video` (video khong tieng, `-an -c:v copy`) + `audio` (chi tieng, `-vn`).
- Tai khoan: khong (`credential: None`) — chi dung ffmpeg cuc bo.

## Tham so
Khong co.

## Luat rieng
- `preview` THUAN (khong I/O). `run` chi cham the gioi ngoai qua `NodeCtx`.
- Nhanh `video`: luon `-c:v copy` (khong giai ma lai hinh, nhanh).
- Nhanh `audio`: uu tien `-c:a copy` (giu nguyen codec goc); loi (vd container m4a khong nhan
  codec goc) thi lui ve re-encode AAC.
- That bai mot phan (video hoac audio loi) -> `NodeError` (ca hai nhanh phu thuoc cung 1 lan doc
  input, khong tach rieng thanh cong/that bai tung nhanh trong P4a nay).

## Kiem tra
- `cargo test -p node-audio-split`
- Doi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-audio-split` roi sinh lai fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
