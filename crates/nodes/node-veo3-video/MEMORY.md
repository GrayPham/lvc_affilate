# Ghi nhớ kỹ thuật — node `veo3_video`

Ghi những gì học được khi làm/chạy node này (quirk nhà cung cấp, mã lỗi, quyết định nhỏ). Mỗi mục
một dòng, có ngày.

- 2026-09-28: tạo node (Task 11, `docs/superpowers/plans/2026-09-27-p5-veo3-nodes.md`) theo đúng
  khuôn `node-dola-video` (spec `docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md`
  ADR-R019, mục "Node & credential").
- P5 CHỈ làm 2 mode `t2v`/`r2v` — KHÔNG làm `i2v`/`interpolation` (rpc `eb1hJf`/`nprQif` chưa có
  cURL thật xác nhận, xem spec mục 7). `VideoSource` của `lvc-veo3::job` cũng chỉ có 2 biến thể
  `Text`/`Reference` nên không cần chặn gì thêm ở node — thêm mode mới sau này sẽ tự lộ ra ở
  `ModeKind`/`VideoSource` (lỗi biên dịch nếu quên khớp).
- Không có tiền lệ ở `dola_video::jobs` cho "tham số không áp dụng cho mode hiện tại" (Dola không
  có khái niệm reference-to-video riêng biệt như vậy) — tự quyết theo hướng: `mode="t2v"` mà có
  `refs` nối vào thì IM LẶNG BỎ QUA (không đọc `inputs.assets("refs")`, không lỗi). Lý do: người
  dùng đổi qua đổi lại `mode` trên cùng 1 node không nên bị lỗi vì cạnh còn treo ở cổng không dùng.
- `MAX_REFERENCE_IMAGES = 6` tự đặt trong `jobs.rs` (không tái dùng `Limits.reference_image_max_count`
  của lõi — con số đó là cấu hình chung cho MỌI node/provider ảnh tham chiếu, còn 6 là giới hạn
  CỨNG của riêng rpc `MZZa6b` phía Google, không phụ thuộc cấu hình người dùng).
- Vượt quá 6 refs khi `r2v`: dùng `GraphError::Param{field:"refs"}` (KHÔNG dùng
  `GraphError::TooManyRefs` của lõi — variant đó gắn với 1 `scene_index` cụ thể vì Dola merge refs
  RIÊNG theo từng cảnh; ở đây refs là danh sách CHUNG cho mọi job trong node, không thuộc về 1 cảnh
  nào, nên `Param` đúng ngữ nghĩa hơn).
- `account` (= `credential_id`) BẮT BUỘC không rỗng — P5 chưa có "auto" như `AccountPool` của Dola
  (tự chọn theo LRU/hạn mức cho provider bất kỳ là việc P6+, xem spec mục 7).
- 2026-10-11 (hoàn thiện kịch bản affiliate đa-engine): thêm override TXT theo dòng cho
  `ratio`→`aspect`/`refs`/`account`, theo đúng hướng đã ghi ở mục trên (không đổi hợp đồng cổng
  vào/ra, chỉ sửa `jobs.rs`). Khác Dola ở 2 điểm CÓ CHỦ Ý:
  - `ratio` không hợp lệ (khác `"16:9"`/`"9:16"`, vd `"1:1"` — Veo3 không có aspect vuông) IM LẶNG
    bỏ qua thay vì lỗi cứng, vì đây là override TUỲ CHỌN theo dòng (khác validate `aspect` tĩnh ở
    tham số node — cái đó VẪN lỗi cứng nếu sai).
  - `account` static rỗng vẫn lỗi `Param{field:"account"}` NGAY LẬP TỨC dù có dòng nào override
    hay không (giữ nguyên invariant "P5 chưa có tự chọn tài khoản" — khác Dola, nơi TXT có thể bù
    cho field tĩnh rỗng vì Dola field đó vốn optional).
  - `refs` vượt `MAX_REFERENCE_IMAGES` SAU KHI gộp giờ kiểm TỪNG DÒNG (gom lỗi, dòng lỗi bị bỏ qua
    — các dòng khác vẫn xử lý tiếp) thay vì kiểm một lần trên danh sách cổng — vì refs giờ có thể
    khác nhau giữa các dòng.
- Không đọc `duration`/`model`/`mode` theo dòng — không có field tương đương ý nghĩa ở Veo3
  (`duration`), hoặc hợp lý hơn khi giữ chung cho cả node (`model`/`mode`).
- `model` rỗng ("") trong tham số node = "tự chọn theo mode"
  (`lvc_veo3::model::default_video_model::{TEXT_TO_VIDEO,REFERENCE_TO_VIDEO}`) — khớp
  `default_params.model = ""` trong manifest, KHÔNG phải giá trị lỗi.
