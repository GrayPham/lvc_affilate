# Ghi nhớ kỹ thuật — `lvc-template-registry`

- 2026-10-11: Repo registry mẫu `github.com/GrayPham/lvc-template-registry` lúc tạo P7a từng để
  PRIVATE theo mặc định của GitHub (người dùng chỉ `git push` chưa `gh repo create --public`) —
  khiến `raw.githubusercontent.com`/API công khai trả 404 dù `git` (qua credential máy) đọc được
  bình thường. Luôn nhắc người dùng xác nhận repo đã PUBLIC trước khi coi tính năng remote là "xong".
- Thiết kế ban đầu (spec P7, ADR-R019/R020) định dùng Vercel làm lớp trung gian + 2 endpoint
  (`/api/templates`, `/api/templates/:id`) đọc nhiều file trong `templates/`. Khi thực hiện P7b,
  người dùng chọn "gọi thẳng GitHub, không qua Vercel" → đổi sang 1 file `index.json` DUY NHẤT ở gốc
  repo (mảng đầy đủ `{id,name,description,category,thumbnail,updated_at,graph}`), tránh N+1 request
  và tránh phải vận hành/deploy Vercel. Scaffold Vercel (`api/`, `vercel.json`) trong repo registry
  vẫn còn đó như phương án dự phòng nếu sau này cần CDN/cache tốt hơn, nhưng KHÔNG phải đường chính.
- MVP P7b KHÔNG đưa URL registry vào `AppSettings`/Settings UI (tránh phải sửa struct `AppSettings`
  dùng chung rất nhiều test ghim cứng danh sách field) — `DEFAULT_INDEX_URL` hard-code trong
  `client.rs`. Nếu sau này cần cấu hình được, cân nhắc thêm field `template_registry_url` vào
  `AppSettings` (nhớ cập nhật MỌI struct literal đầy đủ trong test — xem
  `crates/lvc-core/src/settings.rs` mục `roundtrip_to_map_roi_from_map_bang_gia_tri_goc`).
