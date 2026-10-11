# Provider `lvc-seedance` (API chính thức ByteDance/Volcengine)

Client dịch vụ AI dùng chung cho node Seedance (ADR-R014). KHÔNG biết khái niệm node, KHÔNG
engine/store/browser/app/tauri. reqwest ĐƯỢC.

- Volcengine Ark `/api/v3`, key header `Authorization: Bearer`.
- Ảnh Seedream `images/generations` (ép `response_format=b64_json` → bytes). Video Seedance
  `contents/generations/tasks` + poll `tasks/{id}` → `content.video_url`.
- Khai host trait `SeedanceImageHost`/`SeedanceVideoHost`.
- KHÔNG BAO GIỜ log API key. Chia module: `error`/`req`/`image`/`video`/`client`/`job`.
- Model/endpoint cấu hình được (tham số `model` + `base_url` credential). Xem MEMORY.md (endpoint id).
