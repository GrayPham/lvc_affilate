# lvc-genai

Provider sinh **văn bản** bằng AI (Gemini, OpenAI) dùng chung giữa các node text.

- Trait dịch vụ: `TextGenHost` (engine cấp qua `NodeCtx::services`).
- Client HTTP: `GenAiClient::generate_with_key(api_key, base_url, req)`.
- Node dùng nó: `node-script-gen` (Kịch bản/Storyboard AI).

Cần **API key** (Gemini hoặc OpenAI) lưu trong kho tài khoản (provider `gemini`/`openai`, kind
`api_key`). Bí mật do engine giữ; provider chỉ nhận key qua tham số hàm, không lưu, không log.
