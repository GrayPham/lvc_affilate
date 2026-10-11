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
- Tài khoản: `google/login` (`credential_id` truyền qua tham số `account`, BẮT BUỘC — chưa có
  "auto" như Dola).

## Tham số
`mode` (`"t2v"|"r2v"`), `model` (rỗng = mặc định theo mode), `aspect` (`"portrait"|"landscape"`),
`seed_count`, `account`.

## Luật riêng
- Map ngầm: 1 `TextItem` trên cổng `prompt` = 1 job = 1 video; `scene_index` 1..N liên tục. KHÔNG
  có override TXT theo dòng (`attrs`) — khác `dola_video`, xem `MEMORY.md`.
- `mode="r2v"`: MỌI job dùng CHUNG danh sách ảnh của cổng `refs` (tối đa `MAX_REFERENCE_IMAGES = 6`
  — vượt quá → `GraphError::Param{field:"refs"}`).
- `mode="t2v"`: `refs` bị BỎ QUA hoàn toàn, KHÔNG lỗi dù có nối ảnh vào.
- `mode` khác `"t2v"|"r2v"` → `GraphError::Param{field:"mode"}` (không panic).
- `account` rỗng → `GraphError::Param{field:"account"}` (P5 chưa có tự chọn tài khoản).
- Chạy thật CHỈ qua `VeoVideoHost` (`ctx.services`) — không tự mở trình duyệt/gọi mạng.
- Thất bại một phần → `Ok` + `failure = report.failed.join(",")`.
- P5 CHƯA làm `i2v`/`interpolation` (2 rpc đó chưa có cURL thật xác nhận, xem spec mục 7).

## Kiểm tra
- `cargo test -p node-veo3-video` — gồm test ghim `manifest.json`.
- Đổi manifest có chủ đích: `LVC_WRITE_FIXTURES=1 cargo test -p node-veo3-video` rồi commit
  `manifest.json`, sau đó sinh lại fixture UI:
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
