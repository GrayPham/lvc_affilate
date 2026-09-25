# Node `prompt` — luật

Hợp đồng chung: `crates/lvc-node-api` · Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Phụ thuộc được phép
lvc-node-api, lvc-core (+ thư viện ngoài). **Cấm** node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`.

## Hợp đồng vào/ra
- Vào: không có.
- Ra: `out: Text*` — mỗi dòng TXT (bỏ dòng trống/`#`) là một `TextItem`.
- `TextItem.attrs`: override theo dòng `ratio, duration, model, mode, logo, refs (phân cách \n), account`.

## Tham số
`text: String` — TXT nhiều dòng, cú pháp `prompt | ratio=16:9 | dur=10 | refs=a.jpg,b.jpg | acc=ten`.

## Luật riêng
- Kiểm bằng `parse_txt` (bắt lỗi ngay tại node) rồi xuất dòng thô `parse_lines_raw` — hai hàm dùng chung `parse_line`, cùng luật bỏ dòng → cùng số dòng. Đổi luật bỏ dòng ở một bên là vỡ bất biến.
- Khoá `attrs` là HỢP ĐỒNG với node tiêu thụ (`dola_video` khai lại cùng chuỗi, không import crate này).

## Kiểm tra
- `cargo test -p node-prompt` — gồm test ghim `manifest.json`.
- Đổi manifest có chủ đích: `LVC_WRITE_FIXTURES=1 cargo test -p node-prompt` rồi commit `manifest.json`,
  sau đó sinh lại fixture UI: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
