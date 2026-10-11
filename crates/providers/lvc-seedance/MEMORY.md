# Ghi nhớ kỹ thuật — provider `lvc-seedance`

- 2026-10-01: tạo provider. Volcengine Ark `/api/v3`. Ảnh Seedream `POST /images/generations`
  (header `Authorization: Bearer`), body `{model,prompt,size,response_format:"b64_json",watermark:false}`,
  phản hồi `data[].b64_json`. Video Seedance `POST /contents/generations/tasks` body
  `{model,content:[{type:text,text:"<prompt> --ratio 16:9"},{type:image_url,image_url:{url}}?]}` →
  `{id}`; poll `GET /contents/generations/tasks/{id}` → `status` (queued|running|succeeded|failed) +
  `content.video_url`.
- 2026-10-01: QUAN TRỌNG — Volcengine thường yêu cầu "endpoint id"/"inference access point" riêng
  theo tài khoản thay cho tên model chung. `default_model` chỉ là gợi ý; người dùng NÊN nhập model/
  endpoint của chính họ ở tham số node. Sai model → 400/404 `InvalidParameter`/`ModelNotFound`.
- 2026-10-01: ép `response_format=b64_json` (ảnh) để nhận bytes trực tiếp — không phải tải URL tạm;
  engine ghi bytes thẳng ra file. Video thì `video_url` là link TOS ký tạm, tải bằng GET thường.
- 2026-10-01: i2v truyền `image_url.url` — có thể là data URL base64 (`data:image/png;base64,...`);
  engine dựng data URL từ file local (không cần upload trước).
- 2026-10-01: CHƯA chạy live (không có key). Hình dạng khớp tài liệu công khai Volcengine (knowledge
  cutoff 2026-01); test wiremock. Cần key thật xác nhận, nhất là tên trường `video_url`/`status`.
- 2026-10-01: endpoint mặc định `https://ark.cn-beijing.volces.com`; khu vực khác (vd ap-southeast)
  đổi qua `base_url` của credential.
