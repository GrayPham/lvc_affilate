# Ghi nhớ kỹ thuật — `lvc-veo3`

## Vì sao có 4 trait host, không phải 2 (phát hiện 2026-09-28 khi viết node thật)

Bản đầu chỉ có `VeoCaptchaHost`/`VeoSessionHost`, ý định là node tự gọi 2 trait đó + tự lắp
payload qua `client`/`rpc::*`. Sai: `NodeCtx` (`lvc-node-api`) không có trường "thư mục làm việc"
— nhìn `node-dola-video` mới thấy `local_path` của video được quyết bên TRONG
`Engine::run_video_jobs` (phía sau `DolaVideoHost`), không phải trong node hay trong provider.
Nếu để node veo3 tự lắp payload, nó sẽ phải tự bịa đường dẫn file — sai chỗ. Sửa: thêm
`VeoImageHost`/`VeoVideoHost` làm bề mặt NODE THẬT (đúng khuôn `DolaVideoHost`), còn
`VeoCaptchaHost`/`VeoSessionHost` chỉ còn được `flow::Flow` gọi — `flow::Flow` chỉ dùng bởi PHÍA
IMPL 2 trait mới (thường `lvc-engine`, nơi biết `run_id`/`node_id`/`downloads_dir`), không phải
node. Node veo3 giờ THUẦN như `dola_video`: 1 dòng lấy host qua `ctx.services`, gọi 1 hàm.

## `flow::Flow` không tự lặp poll — bên gọi (impl `VeoVideoHost`) tự quản

`Flow::poll_video_once` chỉ làm ĐÚNG 1 lần gọi `as29s`, không sleep, không lặp — bên gọi (impl
`VeoVideoHost`, thường trong `lvc-engine`) tự viết vòng lặp + khoảng nghỉ + kiểm `CancellationToken`
giữa 2 lần, giống `dola::poll`. Lý do: `lvc-veo3` không phụ thuộc `tokio_util`/không biết khái
niệm "huỷ run" — việc đó là của engine.

## `account`/`credential_id` là tham số BẮT BUỘC, chưa có "auto" (P5)

Khác `dola_video` (param `account` rỗng = tự chọn qua `AccountPool`), node veo3 P5 yêu cầu
`credential_id` cụ thể — chưa xây "pool tài khoản" chung cho mọi provider (chỉ Dola có). Đừng
tưởng nhầm đây là thiếu sót khi review code — là phạm vi đã chốt, xem spec mục 7.

## Google đã bỏ hẳn REST+Bearer (09/2026) — đừng quay lại port `api.py`/`api_worker.py`

`Lvc_veo3_automation/src/core/config.py` hàm `flow_api_enabled()` nói rõ: aisandbox-pa REST+Bearer
đã bị Google tắt ở server ~09/2026. Đường sống DUY NHẤT (ảnh + video) là `flow.google.com`
batchexecute. Nếu thấy tài liệu/README nào (kể cả bản v1 của spec node này, đã xoá) nhắc tới
`aisandbox-pa.googleapis.com`/Bearer token — đó là thông tin CŨ, bỏ qua.

## reCAPTCHA: mint ở trình duyệt KHÁCH ẩn danh, KHÔNG mint trong phiên tài khoản

Đo thật 2 lần (24-25/09/2026, một dự án ảnh khác + xác nhận lại trên chính dự án video này): mint
token bằng code chạy TRONG phiên đã đăng nhập (extension hay Selenium mở thẳng profile account)
→ LUÔN `PUBLIC_ERROR_UNUSUAL_ACTIVITY`, bất kể payload đúng/sai. Mint ở trình duyệt KHÁCH (không
cookie tài khoản, trang tĩnh `flow.google.com/about`) → Google chấp nhận, tạo video thật được. Đây
là lý do `VeoCaptchaHost`/`VeoSessionHost` tách thành 2 trait riêng (2 ngữ cảnh trình duyệt khác
hẳn nhau) — đừng gộp lại thành 1 "VeoBrowserHost" chung, sẽ mất đi sự tách biệt cố ý này.

## Aspect enum KHÁC NHAU giữa ảnh và video — bẫy dễ nhầm nhất

- Ảnh (`rpc::image`, rpcid `ogiZ0b`): dọc = 2, ngang = 3.
- Video (mọi rpc video): dọc = 1, ngang = 2.

`model::Aspect::image_enum()`/`video_enum()` tách riêng có chủ đích — không refactor gộp thành 1
hàm nhận thêm cờ "is_video: bool" kiểu dễ gõ nhầm thứ tự tham số.

## `rpc::image_to_video` (`eb1hJf`) và `rpc::interpolation` (`nprQif`) CHƯA có cURL thật

2 rpc này port từ `veo3.js` (tham khảo trong `docs/ref` của dự án Python), KHÔNG có cURL thật của
chính dự án `Lvc_veo3_automation` xác nhận — khác với `text_to_video`/`reference_to_video`/`image`/
`upload`/`media`/`status` (đều có cURL thật, ngày cụ thể trong doc-comment). Nếu chạy thật và 2 rpc
này lỗi hình dạng payload, đừng ngạc nhiên — đây là rủi ro đã biết trước (xem plan P5 Task 5), cần
verify bằng 1 lần chạy thật rồi cập nhật lại doc-comment (bỏ chữ "chưa verify") khi đã xác nhận.

## `status` (`jwpduf`) không đáng tin cho "đã xong" — luôn dùng `media` (`as29s`)

Python ghi rõ: response `jwpduf` có `status` LUÔN là `'CAE'` dù video đã render xong thật. Chỉ
`as29s` (rpc `media`) mới đáng tin: "xong" = có URL trong response, không phải dựa vào field
`status` nào cả. Node/engine gọi `rpc::status` chỉ để lấy `operation_id`/`scene_id` phụ, quyết định
"đã xong chưa" LUÔN qua `rpc::media::extract(..).done`.

## `video_only` khi poll media video — tránh tải nhầm poster

Khi I2V/R2V còn đang render, response `as29s` có thể chứa CẢ URL poster (`/image/...`, ảnh khung
đầu) VÀ URL video thật (`/video/...` hoặc `.mp4`) cùng lúc — nếu lấy URL đầu tiên tìm được sẽ tải
nhầm poster (~180KB) rồi coi là "video xong". `rpc::media::extract(payload, video_only: true)` chỉ
nhận URL thật của video — luôn dùng `video_only: true` khi poll VIDEO (chỉ `false` cho ẢNH).

## R2V (`MZZa6b`) khác T2V ở 2 điểm dễ port sai

1. `refs` chèn vào **vị trí `[1]`** của spec (đẩy `aspect` xuống `[3]`) — không giống T2V (`aspect`
   ở `[2]`).
2. `seed_count` PHẢI = 1 cho model `*_lite`/Omni (không phải 2 như T2V có thể dùng nhiều spec) —
   đặt 2 khiến Google trả `[]`/`INVALID_ARGUMENT`. Đây là nguyên nhân gốc lỗi "No operations" của
   bản Python cũ trước khi sửa (xác nhận 08/09/2026).
3. `media_ids` (m1/m2) của R2V KHÔNG tự sinh UUID khi thiếu (giữ `null`) — khác T2V/I2V/
   Interpolation (tự sinh UUID khi thiếu). Đây là port ĐÚNG theo Python gốc, không phải thiếu sót.

## `T2V_WORKFLOW_TAIL = 2` là hằng số cố định, KHÔNG phải "số video"

Từng có hiểu nhầm trong bản Python cũ: tưởng phần tử `[1]` của đuôi inner T2V (`[workflow_id, X]`)
là số lượng video cần tạo, nên hạ `X` xuống theo `seed_count` — kết quả tool không bao giờ gửi
đúng tổ hợp web thật dùng (1 spec + đuôi 2). Xác nhận bằng cURL 23/09/2026: đuôi LUÔN = 2, không
phụ thuộc `seed_count`. Nếu sau này cần đổi, phải có cURL thật chứng minh giá trị khác.
