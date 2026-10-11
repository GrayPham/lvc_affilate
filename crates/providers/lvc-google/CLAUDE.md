# Provider `lvc-google` (API chính thức Google)

Client dịch vụ AI dùng chung cho node Google (ADR-R014). **KHÔNG biết khái niệm node**
(không `lvc-node-api`), KHÔNG `lvc-engine`/store/browser/app/tauri. reqwest ĐƯỢC phép.

- API `generativelanguage.googleapis.com`, key ở QUERY `?key=` (giống Gemini text của `lvc-genai`).
- Ảnh: Imagen 3 `POST /v1beta/models/<model>:predict` → `predictions[].bytesBase64Encoded` (base64).
- Video: Veo `POST /v1beta/models/<model>:predictLongRunning` → `{name}`; poll `GET /v1beta/<name>`
  tới `done`, lấy `response.generateVideoResponse.generatedSamples[].video.uri`; tải URI kèm `?key=`.
- Khai host trait `GoogleImageHost`/`GoogleVideoHost` (node lấy qua `ctx.services`; engine impl).
- **KHÔNG BAO GIỜ log API key**; key chỉ qua tham số `*_with_key`, không lưu struct. `GoogleError`
  không chứa key (lỗi mạng dùng `without_url()` để giấu `?key=`).
- Chia module: `error`/`req`/`image`/`video`/`client`/`job` — mỗi file một trách nhiệm.
- Endpoint/model cấu hình được: `base_url` (credential meta) + tham số `model` (rỗng = mặc định).
