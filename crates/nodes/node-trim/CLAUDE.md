# Node `trim` - luat

Hop dong chung: `crates/lvc-node-api` | Luat repo: `CLAUDE.md` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, crates/providers/* (+ thu vien ngoai). **Cam** node khac, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri`.

## Hop dong vao/ra
- Vao: `in` — 1 video (required, khong many).
- Ra: `out` — 1 video da cat.
- Tai khoan: khong (`credential: None`) — chi dung ffmpeg cuc bo.

## Tham so
- `start` (f64 giay, mac dinh 0), `end` (f64 giay, 0 = cat toi het). `end` phai > `start` (hoac = 0).

## Luat rieng
- `preview` THUAN (khong I/O). `run` chi cham the gioi ngoai qua `NodeCtx`.
- That bai mot phan -> `Ok` + `failure`; that bai hoan toan -> `NodeError` (`fatal` chi khi phai dung ca run).

## Kiem tra
- `cargo test -p node-trim`
- Doi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-trim` roi sinh lai fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.