# Ghi nhớ kỹ thuật — provider `lvc-grok`

- 2026-10-01: tạo provider. xAI `POST /v1/images/generations` (OpenAI-compatible), header
  `Authorization: Bearer <KEY>`, body `{model,prompt,n,response_format:"b64_json"}`, phản hồi
  `data[].b64_json`. Endpoint mặc định `https://api.x.ai`.
- 2026-10-01: model mặc định `grok-2-image-1212`. xAI image API KHÔNG nhận `size`/`quality`/`style`
  (khác OpenAI DALL·E) — chỉ `model`/`prompt`/`n`/`response_format`/`user`. Vì vậy node không có
  tham số tỉ lệ khung.
- 2026-10-01: xAI CHƯA có API video công khai (knowledge cutoff 2026-01) → KHÔNG có client/host/node
  video Grok. Nếu xAI ra API video sau này: thêm `video.rs` + `GrokVideoHost` + node, không phá ảnh.
- 2026-10-01: CHƯA chạy live (không có key). Hình dạng khớp tài liệu công khai xAI; test wiremock.
