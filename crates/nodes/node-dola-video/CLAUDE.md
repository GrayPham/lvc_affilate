# Node `dola_video` — luật

Hợp đồng chung: `crates/lvc-node-api` · Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Phụ thuộc được phép
lvc-node-api, lvc-core, lvc-dola (+ thư viện ngoài). **Cấm** node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`.

## Hợp đồng vào/ra
- Vào: `prompt: Text*` (bắt buộc, nhiều cạnh — gộp theo thứ tự cạnh), `refs: Image*`.
- Ra: `out: Video*` — `Asset.meta`: `node`, `index` (= scene_index), `account`, `conversation_id`, `watermarked`.
- Tài khoản: `dola/session` (pool do host quản lý).

## Tham số
`mode, model, ratio, duration, logo, account` (khớp `NodeKind::Video` P3 — workflow cũ đọc được).

## Luật riêng
- Map ngầm: 1 `TextItem` = 1 cảnh = 1 video; `scene_index` 1..N liên tục.
- Gộp refs: cổng `refs` TRƯỚC, TXT `refs=` SAU, bỏ trùng giữ lần đầu, kiểm `reference_image_max_count`.
- TXT thắng mặc định node — TRỪ `account`: node thắng nếu khác rỗng.
- Chạy thật CHỈ qua `DolaVideoHost` (`ctx.services`) — không tự mở trình duyệt/gọi mạng/đụng DB.
- Thất bại một phần → `Ok` + `failure = "node <id>: canh 1,3 that bai"`.

## Kiểm tra
- `cargo test -p node-dola-video` — gồm test ghim `manifest.json`.
- Đổi manifest có chủ đích: `LVC_WRITE_FIXTURES=1 cargo test -p node-dola-video` rồi commit `manifest.json`,
  sau đó sinh lại fixture UI: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
