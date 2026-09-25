# Node `save` — luật

Hợp đồng chung: `crates/lvc-node-api` · Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Phụ thuộc được phép
lvc-node-api (+ thư viện ngoài). **Cấm** node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`.

## Hợp đồng vào/ra
- Vào: `video: Video*` HOẶC `image: Image*` (`require_any`).
- Ra: không có.

## Tham số
`out_dir: String`.

## Luật riêng
- COPY, không di chuyển. Tên `<run_id>_<node_nguồn>_<index>.mp4` — dùng id node NGUỒN (tránh trùng tên khi nhiều node video cùng đổ vào).
- Cạnh có `values = None` (nguồn không chạy) → `failure = "<id>: luu loi"`.

## Kiểm tra
- `cargo test -p node-save` — gồm test ghim `manifest.json`.
- Đổi manifest có chủ đích: `LVC_WRITE_FIXTURES=1 cargo test -p node-save` rồi commit `manifest.json`,
  sau đó sinh lại fixture UI: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
