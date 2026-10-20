# Thiết kế: Reel MuseAI "ảnh khoá trước → video sau" (đồng nhất nhân vật)

Ngày: 2026-10-20. Trạng thái: CHỜ DUYỆT.

## Ý định & vì sao

Template reel món ăn hiện dùng MuseAI tạo video trực tiếp từ **2 ảnh tham chiếu xung đột**
(ảnh nhân vật mặc sơ mi trắng + ảnh đầm trên ma-nơ-canh). Bằng chứng từ 3 lần chạy thật
(2026-10-19):

- `run_e412e8bf` (23:40): 5/6 cảnh **đúng đầm xanh + đúng mặt + có hook dắt heo** — tốt.
- `run_a4579fb6` (01:19): 2/4 cảnh đầm xanh, còn lại trôi sang áo trắng; **mặt luôn đồng nhất**.
- `run_3148ce7c` (22:55): 0/6 cảnh đầm — toàn áo len kem.

Kết luận: **MuseAI khoá MẶT tốt**, nhưng **TRANG PHỤC trôi** vì agent chat của Muse thấy 2 bộ đồ
khác nhau và hay khoá nhầm bộ sơ mi trắng của ảnh nhân vật; ảnh ma-nơ-canh khó "mặc" lên người.
Đây là lỗi **method + prompt**, không phải giới hạn của MuseAI (người dùng xác nhận Muse làm được).

Giải pháp người dùng chọn: với mỗi cảnh, **tạo một ẢNH TĨNH khoá trước** (dễ kiểm soát hơn video)
từ các ref sạch, rồi **dựng VIDEO 3s từ đúng ảnh tĩnh đó**. Node video chỉ nhận **một** ref sạch
(ảnh tĩnh vừa tạo) thay vì 2 ref mâu thuẫn → khoá cả mặt lẫn đồ.

## Mục tiêu

- Mỗi cảnh: `ref → museai_image (ảnh tĩnh khoá) → museai_video (3s) → …`; 6 cảnh → `concat` → `save`.
- **Mỗi cảnh có ảnh tĩnh RIÊNG** (người dùng chốt) — cùng nhân vật/đồ nhưng khác động tác/bối cảnh.
- Ref chung nạp 1 lần: **ảnh cô gái**, **ảnh trang phục**, **ảnh động vật (con heo)**.
- Giữ nhịp **mỗi cảnh ~3s, chuyển cảnh nhanh** như video mẫu `video/mau_afflate.mp4`.
- Prompt mỗi cảnh **nói thẳng + lặp**: "đúng cô gái trong ảnh, GIỮ NGUYÊN mặt/tóc + ĐẦM XANH
  SATIN, [hành động], selfie UGC dọc 9:16".

## Không làm (non-goals)

- KHÔNG đổi sang provider khác (Seedance/Veo/Google) — ở lại MuseAI (người dùng yêu cầu).
- KHÔNG làm "image-to-video first-frame thật" — Muse là agent chat, ref là mềm; ta tối ưu trong
  giới hạn đó (ref sạch đơn + prompt khoá).
- KHÔNG dựng bộ ghép/sửa ảnh cục bộ (PIL/compositing) — Muse tự sinh ảnh từ ref + prompt.

## Chống mục tiêu (anti-goals)

- KHÔNG đưa lại nhiều ref xung đột vào node video (đó là gốc lỗi cũ).
- KHÔNG log bí mật (cookie/token/prompt đầy đủ) — luật #3 CLAUDE.md.
- KHÔNG giả mạo fingerprint TLS/JA3 (curl_cffi/hyprcurl…) — đã từ chối nhiều lần (xem
  `crates/providers/lvc-museai/CLAUDE.md`).

## Phần ĐÃ quyết (từ hội thoại)

1. **Ảnh riêng mỗi cảnh** (không dùng 1 ảnh hero chung).
2. **Fallback**: cảnh không cần con heo → bỏ ref đó; không có ref trang phục riêng → coi ảnh cô
   gái đã gồm trang phục, lấy ảnh cô gái làm ref chính.
3. **Nâng cấp node tạo ảnh MuseAI để NHẬN ảnh ref** (hiện chỉ có cổng `prompt`). Cơ chế Muse
   (`/chat/stream` nhúng ảnh base64 trong `items`) đã hỗ trợ ảnh cho cả tạo ảnh — `generate_video`
   đã dùng, chỉ cần mirror sang `generate_image`.
4. Node video mỗi cảnh **chỉ nhận 1 ref = ảnh tĩnh của cảnh đó** (ref sạch, không mâu thuẫn).

## Ràng buộc kỹ thuật

- **ADR-R014**: node = 1 crate, chỉ phụ thuộc `lvc-node-api`/`lvc-core`/`crates/providers/*`/thư
  viện ngoài. Thêm node = crate mới + 1 dòng registry. Ở đây KHÔNG thêm kind mới — chỉ **sửa node
  `museai_image` hiện có** (thêm cổng `refs`) → **tăng `manifest.version` + tự migrate params**.
- Provider `lvc-museai` THUẦN, không biết khái niệm node; thêm field `images` vào `ImageJob` +
  `GenerateImageParams`, truyền vào `chat_stream` (đã có sẵn tham số `images`). Ngân sách ref
  ~11 MB tổng, ≤3,5 MB mỗi ảnh (attachments.rs) — áp dụng y như video.
- `lvc-core` thuần; MSRV 1.85 (không let-chains); clippy pedantic `-D warnings`.
- Fixture: sinh lại `LVC_WRITE_FIXTURES=1 cargo test -p node-museai-image` rồi
  `... -p lvc-app commands::fixtures`. Frontend `app/src/graph/ports.ts` thêm cổng `refs` cho
  `museai_image` (khớp manifest) — có test ghim (`ipc_fixtures`/`labels`).
- `kind` giữ nguyên (`museai_image`) — không migrate kind.
- Chạy thật tốn credit MuseAI — hỏi người dùng trước mỗi lần chạy live; dev dùng fake-driver.

## Để lại cho người làm (builder)

- Câu chữ prompt chính xác cho ảnh và cho video từng cảnh (khoá "same woman + same dress").
- Cách định tuyến ref chung (3 node `image_upload`) tới cổng `refs` của từng `museai_image`
  (cảnh có heo nối thêm ref heo; cảnh không có thì thôi).
- Node `museai_image` đổi hình dạng params? (chỉ thêm cổng input, params có thể không đổi — nếu
  không đổi `params` thì không cần bump version; cần xác nhận khi code).
- Thứ tự dựng template: 3 `image_upload` → 6×(`museai_image`→`museai_video`) → `concat` → `save`.
- (Ngoài phạm vi thiết kế này nhưng còn nợ) hoàn tất + chạy `check.ps1` + commit: lỗi node Nối
  video (adapter REMOVED_KINDS), chống xoá cạnh, nút Huỷ MuseAI (đã sửa code, chưa gate/commit).

## Quyết trong lúc review (builder — call của tôi)

- **`build_image_prompt` đang ép "product photography style"** (`client.rs:550-555`, test ghim
  `build_image_prompt_giu_nguyen_y_*` ~dòng 707). Hậu tố này đẩy Muse về ảnh chụp sản phẩm/ma-nơ-canh
  — ĐÚNG lỗi ta đang diệt. **Quyết:** sửa để ảnh khoá nhân vật yêu cầu **ảnh một NGƯỜI** (giữ phần
  "một ảnh tĩnh, không chuyển động/không video", BỎ "product photography style"). Cách làm cụ thể
  (tham số hoá `build_image_prompt`, hay tách hàm cho "character still") để builder quyết khi code;
  cập nhật test ghim tương ứng. Thuộc provider `lvc-museai` — nằm trong phạm vi "mirror sang
  generate_image", chấp nhận đụng provider + test.
- **Giữ cấu trúc 6 node museai_image RIÊNG, mỗi node 1 prompt** (KHÔNG gộp 6 prompt vào 1 node) —
  engine chạy node tuần tự, mỗi node mở bootstrap + `HatchConnection` riêng với ĐÚNG 1 job, nên
  tránh được quirk "job thứ 2 trong lô hỏng 5/5" ghi ở `lvc-museai/MEMORY.md`.

## Kho ref thật (D:\LVCMedia\lvc_affilate\image) + tối ưu prompt

Người dùng chỉ định kho ref `image/` (đều PNG, <3,5 MB — vừa ngân sách):
- `mau.png` — **character sheet 9 góc** cùng một cô gái châu Á tóc nâu dài, mặc **đầm yếm nâu +
  cao gót trắng**. Ref khoá danh tính mạnh (đa góc). ⇒ ref CHÍNH cho bước tạo ảnh.
- `trangphuc.png` — đúng **đầm yếm nâu** đó (ảnh sạch nền trắng) — KHỚP outfit trong `mau.png`
  (hết xung đột kiểu cũ). ⇒ ref trang phục.
- `heo.png` — con heo (đạo cụ hook cảnh 1). `mau2.png` — cô gái đi trong bếp (ảnh lifestyle, dự phòng).

**Nhân vật chuẩn (mọi prompt phải mô tả GIỐNG HỆT):** cô gái châu Á, tóc nâu dài gợn sóng, **đầm
yếm nâu ôm dáng, cao gót đế xuồng trắng**; thần thái tươi, body-flattering (đúng ý "ăn mặc đôn dáng").

**Tối ưu prompt (yêu cầu người dùng) — builder thiết kế theo nguyên tắc:**
- Prompt ẢNH mỗi cảnh: khoá "đúng cô gái trong ảnh ref, GIỮ NGUYÊN mặt + đầm yếm nâu + cao gót
  trắng" + mô tả **bố cục/góc máy cảnh đó** (vd hành lang dắt heo, cận tay sơ chế, bếp bốc khói,
  bê đĩa chốt) — một cảnh = một bố cục rõ, UGC dọc 9:16.
- Prompt VIDEO mỗi cảnh: "làm động ảnh này, cùng người/đồ, [chuyển động nhẹ 3s], selfie UGC",
  KHÔNG mô tả lại nhân vật mâu thuẫn với ảnh tĩnh.
- Theo nhịp shot của `video/mau_afflate.mp4`: hook dắt heo → nguyên liệu → sơ chế → nấu (khói)
  → bày đĩa → chốt bê món; mỗi cảnh ~3s, chuyển cảnh nhanh.

## Quyết với người dùng (2026-10-20)

- **Chi phí/chạy:** pipeline này sinh gấp đôi (6 ảnh + 6 video ≈ 12 lần, ~1 giờ, ~2× credit).
  Người dùng chọn **thử 2 cảnh trước** — code xong, chạy live CHỈ 2 cảnh để kiểm đồng nhất; đạt rồi
  mới chạy full 6. ⇒ Builder nên làm template dễ chạy một tập con (hoặc hướng dẫn tắt bớt cảnh), và
  bước nghiệm thu đầu tiên chỉ cần 2 cảnh.
- **Mẫu:** người dùng chọn **thay luôn mẫu cũ** — ghi đè chính `museai-reel-mon-an.json` (giữ id
  `public:museai-reel-mon-an` để vẫn là mẫu reel món ăn duy nhất trong danh sách). Bản cũ (đưa thẳng
  2 ref vào video) KHÔNG giữ lại trong kho mẫu; `run_e412e8bf` chỉ còn là **mốc nghiệm thu tham
  chiếu** (frame đã lưu ở scratchpad), không phải mẫu sống.

## Kiểm thử / nghiệm thu

- `cargo test -p lvc-museai -p node-museai-image -p lvc-app`, clippy ×2, `scripts/check.ps1`.
- **Nghiệm thu bước 1 (2 cảnh, live, xin phép):** trích frame 2 cảnh → cùng mặt + cùng đầm xanh.
- **Nghiệm thu đủ (6 cảnh):** ≥5/6 cảnh cùng mặt + cùng đầm xanh, có hook dắt heo cảnh 1, nhịp 3s —
  đạt bằng hoặc hơn `run_e412e8bf`.
