# Node `load_video` - luat

Hop dong chung: `crates/lvc-node-api` | Luat repo: `CLAUDE.md` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, crates/providers/* (+ thu vien ngoai). **Cam** node khac, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri`.

## Hop dong vao/ra
- Vao: (khong co) — day la node NGUON.
- Ra: `out` — video (kieu video, many).
- Tai khoan: khong (`credential: None`).

## Tham so
- `paths` (Vec<String> — UI dung 1 video), `label` (nhan tuy chon cho chip tham chieu).
- Thieu file khi chay = `NodeError::fatal` (dung ca run), giong node-image-upload.

## Luat rieng
- `preview` THUAN (khong I/O). `run` chi cham the gioi ngoai qua `NodeCtx`.
- That bai mot phan -> `Ok` + `failure`; that bai hoan toan -> `NodeError` (`fatal` chi khi phai dung ca run).

## Kiem tra
- `cargo test -p node-load-video`
- Doi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-load-video` roi sinh lai fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.