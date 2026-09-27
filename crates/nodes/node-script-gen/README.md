# Kịch bản AI (`script_gen`)

Sinh **kịch bản nhiều phân cảnh** bằng AI text (Gemini hoặc OpenAI). Mỗi phân cảnh là một dòng ở
cổng ra `out` — nối thẳng vào cổng `prompt` của node **Tạo video Dola** để tạo hàng loạt video theo
storyboard.

- **Vào**: `brief` (Text, tuỳ chọn) — mô tả sản phẩm/chủ đề. Bỏ trống thì dùng tham số `topic`.
- **Ra**: `out` (Text, nhiều dòng) — mỗi dòng một cảnh.
- **Tham số**: `topic`, `num_scenes` (mặc định 3), `language` (mặc định `vi`), `style`, `system`
  (chỉ dẫn hệ thống tuỳ chọn), `credential` (tên API key trong kho), `provider` (`gemini`/`openai`).

## Cần API key

Node cần một **API key** trong kho tài khoản: provider `gemini` hoặc `openai`, kind `api_key`. Ghi
đúng TÊN (label) key vào tham số `credential`. Bí mật do backend giữ — node/UI không bao giờ thấy
key; engine tự đọc từ vault khi chạy. Chi phí do nhà cung cấp AI tính (không tốn credit Dola).

- Loại: `script_gen`
- Hợp đồng đầy đủ: `manifest.json`
