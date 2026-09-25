# Node `image_upload` — luật

Hợp đồng chung: `crates/lvc-node-api` · Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Phụ thuộc được phép
lvc-node-api (+ thư viện ngoài). **Cấm** node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`.

## Hợp đồng vào/ra
- Vào: không có.
- Ra: `out: Image*` — `Asset.meta`: `node`, `index` (1-based), `label` (nếu có).

## Tham số
`paths: [String]`, `label: String` (tuỳ chọn — nhãn chip tham chiếu, vd "character").

## Luật riêng
- Thiếu file khi CHẠY = `NodeError::fatal` (dừng cả run, giữ hành vi P3). Preview không chạm đĩa.

## Kiểm tra
- `cargo test -p node-image-upload` — gồm test ghim `manifest.json`.
- Đổi manifest có chủ đích: `LVC_WRITE_FIXTURES=1 cargo test -p node-image-upload` rồi commit `manifest.json`,
  sau đó sinh lại fixture UI: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
