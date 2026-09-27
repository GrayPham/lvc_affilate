# Ghi nhớ kỹ thuật — provider `lvc-genai`

Ghi quirk API (mã lỗi lạ, giới hạn server, tên model đổi) — mỗi mục một dòng, có ngày.

- 2026-09-27: tạo provider. Gemini `POST /v1beta/models/<model>:generateContent?key=<KEY>` (key ở
  QUERY, không header); OpenAI `POST /v1/chat/completions` + `Authorization: Bearer <KEY>`.
- 2026-09-27: model mặc định — Gemini `gemini-2.0-flash`, OpenAI `gpt-4o-mini`. Nếu nhà cung cấp
  đổi/bỏ tên model → 404/400; sửa hằng `DEFAULT_MODEL` trong `gemini.rs`/`openai.rs`.
- 2026-09-27: mã lỗi chuẩn hoá (`client::check_status`): 401/403 → `Auth`, 429 → `Quota`, khác →
  `Http{status,detail}`. Gemini trả 400 khi key hợp lệ nhưng body sai; 403 khi key sai/không bật API.
- 2026-09-27: CHƯA chạy live (không có key). Cần key thật để xác nhận hình dạng phản hồi ngoài đời
  đúng như `gemini::parse`/`openai::parse` (đã khớp tài liệu công khai). Test hiện dùng wiremock.
