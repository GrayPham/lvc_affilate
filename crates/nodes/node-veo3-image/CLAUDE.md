# Node `veo3_image` — luật

Hợp đồng chung: `crates/lvc-node-api` · Luật repo: `CLAUDE.md` gốc (mục "Node độc lập").

## Phụ thuộc được phép
lvc-node-api, lvc-core (gián tiếp qua lvc-node-api), lvc-veo3 (provider, + thư viện ngoài). **Cấm**
node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`. Node CHỈ biết
[`lvc_veo3::VeoImageHost`] — KHÔNG biết `VeoCaptchaHost`/`VeoSessionHost`/`flow::Flow`/`client`/
`rpc::*` tồn tại (đó là chi tiết bên trong impl của trait, nằm ở `lvc-engine`).

## Hợp đồng vào/ra
- Vào: `prompt: Text*` (bắt buộc, nhiều cạnh — mỗi `TextItem` = 1 ảnh), `refs: Image*` (tuỳ chọn —
  dùng CHUNG cho MỌI ảnh, không có trần cứng riêng như R2V video).
- Ra: `out: Image*` — `Asset.meta`: `node`, `index` (= `scene_index`), `width`/`height` (chỉ khi
  host trả kèm).
- Tài khoản: `google/login` (`credential_id` truyền qua tham số `account` — rỗng = tự chọn tài
  khoản đầu tiên còn dùng được trong kho, `VaultVeo3AccountResolver`, giống `dola_video`,
  2026-10-13).

## Tham số
`model` (`"NARWHAL"|"GEM_PIX_2"|"HARBOR_SEAL"`, mặc định `GEM_PIX_2`), `aspect`
(`"portrait"|"landscape"`, mặc định `portrait`), `seed` (rỗng = 0), `account`.

## Luật riêng
- Map ngầm: 1 `TextItem` trên cổng `prompt` = 1 job = 1 ảnh; `scene_index` 1..N liên tục. KHÔNG có
  override TXT theo dòng (`attrs`) — khác `dola_video`, xem `MEMORY.md`.
- Ảnh cổng `refs` dùng CHUNG cho MỌI job (không tách theo dòng TXT `refs=`).
- `model`/`aspect` sai → `GraphError::Param{field:"model"|"aspect"}` (không panic).
- `account` rỗng — KHÔNG lỗi (2026-10-13), truyền thẳng xuống `VeoAccountResolver` để tự chọn tài
  khoản.
- Không có ảnh nào (`prompt` rỗng) → `GraphError::Scene`.
- Ảnh tạo ĐỒNG BỘ (không polling) — `run` gọi `VeoImageHost::generate_images` một lần rồi trả kết
  quả ngay, không tự lặp/tự huỷ theo `ctx.cancel` (đó là việc của host).
- Chạy thật CHỈ qua `VeoImageHost` (`ctx.services`) — không tự mở trình duyệt/gọi mạng.
- Thất bại một phần → `Ok` + `failure = report.failed.join(",")`.

## Kiểm tra
- `cargo test -p node-veo3-image` — gồm test ghim `manifest.json` (trong `src/lib.rs`) + hợp đồng
  `build_jobs`/`preview`/`run` (trong `tests/contract.rs`).
- Đổi manifest có chủ đích: `LVC_WRITE_FIXTURES=1 cargo test -p node-veo3-image` rồi commit
  `manifest.json`, sau đó sinh lại fixture UI:
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
