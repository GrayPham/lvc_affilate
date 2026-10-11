# Provider `lvc-grok` (API chính thức xAI)

Client dịch vụ AI cho node Grok (ADR-R014). KHÔNG biết khái niệm node, KHÔNG engine/store/browser/
app/tauri. reqwest ĐƯỢC.

- xAI `/v1/images/generations` (OpenAI-compatible), key header `Authorization: Bearer`, ép
  `response_format=b64_json` → bytes.
- Khai host trait `GrokImageHost` (chỉ ảnh — xAI chưa có API video công khai).
- KHÔNG BAO GIỜ log API key. Chia module: `error`/`req`/`image`/`client`/`job`.
- Model/endpoint cấu hình được (tham số `model` + `base_url` credential).
