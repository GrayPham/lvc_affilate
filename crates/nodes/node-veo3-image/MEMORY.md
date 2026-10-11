# Ghi nhớ kỹ thuật — node `veo3_image`

Ghi những gì học được khi làm/chạy node này (quirk nhà cung cấp, mã lỗi, quyết định nhỏ). Mỗi mục
một dòng, có ngày.

- 2026-09-28: tạo node (Task 10, `docs/superpowers/plans/2026-09-27-p5-veo3-nodes.md`) theo đúng
  khuôn `node-dola-video`/`node-veo3-video` (spec
  `docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md` ADR-R019, mục "Node & credential").
- `Cargo.toml [dependencies]` CHỈ có `lvc-node-api`/`lvc-veo3` (+ async-trait/serde/serde_json) —
  KHÔNG `lvc-core` trực tiếp. `PreviewCtx.limits`/`NodeCtx.limits` vẫn dùng được (đọc field
  `usize` như `cx.limits.batch_max_scenes` mà không cần `use lvc_core::Limits` — Rust cho phép truy
  cập field public của một type dù crate gốc không phải dependency trực tiếp, chỉ cấm ĐẶT TÊN type
  đó). Cuối cùng quyết định KHÔNG dùng `Limits` luôn (xem mục dưới) nên không cần thủ thuật này,
  nhưng ghi lại vì có thể cần lại nếu sau này thêm giới hạn dùng `Limits` của lõi.
- Quyết định KHÔNG áp `Limits.batch_max_scenes`/`reference_image_max_count` của lõi cho node này —
  giống `node_veo3_video` (không phải giống `dola_video`): 2 con số đó là cấu hình chung Dola, còn
  Google Flow (`ogiZ0b`) không công bố trần refs riêng, và số ảnh/lần chạy để server tự chặn nếu
  vượt quá thay vì đoán một con số cứng không có nguồn xác nhận. Nếu sau này có cURL thật xác nhận
  trần thật của `ogiZ0b`, thêm check bằng `GraphError::Param{field:"refs"}` (giống cách
  `node_veo3_video` chặn `MAX_REFERENCE_IMAGES = 6` cho R2V) — KHÔNG dùng `GraphError::TooManyRefs`
  vì variant đó gắn 1 `scene_index` cụ thể, còn refs ở đây là danh sách CHUNG cho mọi job.
- Không có override TXT theo dòng (`attrs` như `model=`/`account=` của `node_prompt::attrs`) — bảng
  thiết kế ADR-R019 chỉ liệt `model/aspect/seed/account` là tham số NODE, không có khoá TXT tương
  ứng cho ảnh. Cân nhắc tái dùng khoá `attrs::MODEL`/`attrs::ACCOUNT` chung (đã có sẵn, dùng bởi
  `dola_video`) nhưng bỏ vì rủi ro va giá trị: một `TextItem` dùng chung cho cả `dola_video` VÀ
  `veo3_image` (fan-out cùng 1 node Prompt) có thể mang `model=seedance-2.0` (dành cho Dola) — nếu
  `veo3_image` cũng đọc khoá `model` đó sẽ báo lỗi `GraphError::Param` sai lầm. Nếu sau này cần
  override theo dòng, đặt khoá RIÊNG (không tái dùng `attrs::MODEL`) hoặc yêu cầu người dùng tách
  node Prompt riêng cho từng loại node tiêu thụ.
- `seed` rỗng (`None`) → job dùng `0` (không sinh seed ngẫu nhiên trong `build_jobs` — hàm này phải
  THUẦN/tất định vì cả `preview` và `run` đều gọi; sinh số ngẫu nhiên ở đây sẽ làm `preview` không
  còn thuần và có thể lệch giữa lần preview và lần run thật).
- Ảnh tạo ĐỒNG BỘ (rpc `ogiZ0b` trả URL ngay, không polling) — khác `veo3_video` (bất đối xứng, host
  tự poll). Node không cần biết gì thêm về chuyện này (đã gói trong `VeoImageHost::generate_images`).
- `width`/`height` trong `ImageJobDone` là `Option<i64>` (Google có thể không trả kích thước) —
  chỉ gắn vào `Asset.meta` khi `Some` (giống cách `node-image-upload` gắn `meta::LABEL` có điều
  kiện), không chèn chuỗi rỗng khi thiếu.
