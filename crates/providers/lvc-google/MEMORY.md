# Ghi nhớ kỹ thuật — provider `lvc-google`

Ghi quirk API (mã lỗi lạ, giới hạn server, tên model đổi) — mỗi mục một dòng, có ngày.

- 2026-10-01: QUAN TRỌNG (xác minh live trên ai.google.dev 2026-10-01, do phiên lvc-seedancev2-1c
  cung cấp): **Imagen `:predict` ĐÃ BỊ TẮT trên Gemini Developer API (key)** — Google ghi "Imagen
  models are shut down. Use Nano Banana". Bản đầu tiên của provider dùng Imagen → ĐÃ ĐỔI sang Nano
  Banana. (Imagen có thể vẫn sống trên Vertex AI + OAuth, nhưng ta dùng API key nên không áp dụng.)
- 2026-10-01: Ảnh Nano Banana `POST /v1beta/models/<model>:generateContent`, header
  `x-goog-api-key: <KEY>`, body `contents[].parts[].text` + `generationConfig.{responseModalities:
  ["IMAGE"], imageConfig:{aspectRatio,imageSize}}`; phản hồi `candidates[0].content.parts[].
  inlineData.data` (base64). Model: `gemini-3.1-flash-image` (tới 4K), `gemini-3.1-flash-lite-image`
  (chỉ 1K), `gemini-3-pro-image`, `gemini-2.5-flash-image` (legacy). aspectRatio hợp lệ: 1:1,3:2,
  2:3,3:4,4:3,4:5,5:4,9:16,16:9,21:9. imageSize: 0.5K/1K/2K/4K (K HOA).
- 2026-10-01: Video Veo `:predictLongRunning` trả `{name}`, poll `GET /v1beta/<name>` (header
  `x-goog-api-key`) tới `done`, URI ở `response.generateVideoResponse.generatedSamples[].video.uri`.
  Model: `veo-3.1-generate-preview` / `-fast-` / `-lite-` (veo-3.0-* ĐÃ deprecated). Veo KHÔNG có
  free tier (key phải bật thanh toán). Video Google chỉ giữ ~2 ngày. 1080p/4K hoặc có ref ép 8s;
  tối đa 3 ảnh ref. personGeneration: t2v chỉ `allow_all`; i2v/ref chỉ `allow_adult`.
- 2026-10-01: model mặc định — ảnh `gemini-3.1-flash-image`, video `veo-3.1-generate-preview`.
  Người dùng override qua tham số node `model`. Tham khảo thêm spec của phiên 1c:
  `docs/superpowers/specs/2026-10-01-gemini-api-media-nodes-design.md`.
- 2026-10-01: TẢI URI video PHẢI kèm header `x-goog-api-key` (URI không tự ký) + follow redirect
  (reqwest tự follow). Dùng `client::download_video_with_key`.
- 2026-10-01: auth TOÀN BỘ qua header `x-goog-api-key` (KHÔNG dùng query `?key=` nữa — tránh lộ key
  trong log/redirect).
- 2026-10-01: CHƯA chạy live (không có key). Hình dạng request/response khớp tài liệu công khai
  (knowledge cutoff 2026-01); test dùng wiremock. Cần key thật để xác nhận ngoài đời, nhất là hình
  dạng operation video (Google có thể bọc thêm `metadata`/đổi vị trí `uri`).
- 2026-10-01: dùng CHUNG API key với Gemini text (provider vault `"gemini"`, kind `api_key`) — không
  tạo provider vault riêng cho Google official.
