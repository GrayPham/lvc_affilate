# Node `veo3_video` — luật

Hợp đồng chung: `crates/lvc-node-api` · Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Phụ thuộc được phép
lvc-node-api, lvc-core (gián tiếp qua lvc-node-api), lvc-veo3 (provider, + thư viện ngoài). **Cấm**
node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`. Node CHỈ biết
[`lvc_veo3::VeoVideoHost`] — KHÔNG biết `VeoCaptchaHost`/`VeoSessionHost`/`flow::Flow`/`client`/
`rpc::*` tồn tại (đó là chi tiết bên trong impl của trait, nằm ở `lvc-engine`).

## Hợp đồng vào/ra
- Vào: `prompt: Text*` (bắt buộc, nhiều cạnh — dùng cho CẢ `t2v` lẫn `r2v`), `refs: Image*` (tuỳ
  chọn — chỉ có ý nghĩa khi `mode="r2v"`, tối đa 6 ảnh).
- Ra: `out: Video*` — `Asset.meta`: `node`, `index` (= `scene_index`).
- Tài khoản: `google/login` (`credential_id` truyền qua tham số `account` — rỗng = tự chọn tài
  khoản đầu tiên còn dùng được trong kho, `VaultVeo3AccountResolver`, giống `dola_video`,
  2026-10-13).

## Tham số
`mode` (`"t2v"|"r2v"`), `model` (rỗng = mặc định theo mode), `aspect` (`"portrait"|"landscape"`),
`seed_count`, `account`.

## Luật riêng
- Map ngầm: 1 `TextItem` trên cổng `prompt` = 1 job = 1 video; `scene_index` 1..N liên tục.
- Override TXT theo dòng (`attrs`, khoá trùng `node_prompt::attrs`): `ratio` (`"16:9"`→landscape,
  `"9:16"`→portrait; giá trị khác IM LẶNG bỏ qua, giữ `aspect` tĩnh của node — KHÔNG lỗi), `refs`
  (gộp thêm vào cổng `refs`, chỉ dùng khi `mode="r2v"`, cú pháp `\n`-separated), `account` (ghi đè
  khi dòng có giá trị khác rỗng; dòng không override dùng `account` tĩnh của node). KHÔNG đọc
  `duration`/`model`/`mode` theo dòng (không có field tương đương ở Veo3 / vẫn dùng chung cho cả
  node) — xem `MEMORY.md`.
- `mode="r2v"`: MỌI job mặc định dùng ảnh của cổng `refs`, có thể gộp thêm ảnh riêng qua `refs=`
  TXT mỗi dòng (tối đa `MAX_REFERENCE_IMAGES = 6` SAU KHI gộp — vượt quá →
  `GraphError::Param{field:"refs"}` cho đúng dòng đó, các dòng khác vẫn xử lý tiếp).
- `mode="t2v"`: `refs` (cổng lẫn TXT) bị BỎ QUA hoàn toàn, KHÔNG lỗi dù có nối ảnh vào.
- `mode` khác `"t2v"|"r2v"` → `GraphError::Param{field:"mode"}` (không panic).
- `account` tĩnh (tham số node) RỖNG — KHÔNG lỗi (2026-10-13, trước đây bắt buộc): mỗi job dùng
  `item_account` (override TXT nếu dòng có, không thì giá trị tĩnh) truyền xuống
  `VeoAccountResolver`; rỗng ở đó nghĩa là TỰ CHỌN tài khoản (xem `lvc-app::veo3::
  VaultVeo3AccountResolver`).
- Chạy thật CHỈ qua `VeoVideoHost` (`ctx.services`) — không tự mở trình duyệt/gọi mạng.
- Thất bại một phần → `Ok` + `failure = report.failed.join(",")`.
- P5 CHƯA làm `i2v`/`interpolation` (2 rpc đó chưa có cURL thật xác nhận, xem spec mục 7).

## Kiểm tra
- `cargo test -p node-veo3-video` — gồm test ghim `manifest.json`.
- Đổi manifest có chủ đích: `LVC_WRITE_FIXTURES=1 cargo test -p node-veo3-video` rồi commit
  `manifest.json`, sau đó sinh lại fixture UI:
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
