# Node `concat` - luat

Hop dong chung: `crates/lvc-node-api` | Luat repo: `CLAUDE.md` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, crates/providers/* (+ thu vien ngoai). **Cam** node khac, `lvc-engine`,
`lvc-store`, `lvc-browser`, `tauri`.

## Hop dong vao/ra
- Vao: TODO
- Ra: TODO
- Tai khoan: TODO (`credential` trong manifest, vd `{ provider: "gemini", kinds: ["api_key"] }`)

## Tham so
TODO

## Luat rieng
- `preview` THUAN (khong I/O). `run` chi cham the gioi ngoai qua `NodeCtx`.
- That bai mot phan -> `Ok` + `failure`; that bai hoan toan -> `NodeError` (`fatal` chi khi phai dung ca run).

## Kiem tra
- `cargo test -p node-concat`
- Doi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-concat` roi sinh lai fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.