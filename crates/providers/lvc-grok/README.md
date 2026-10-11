# lvc-grok

Provider gọi **API chính thức** xAI (Grok) để tạo ảnh. Người dùng tự nhập API key xAI trong màn
**Tài khoản AI → API key** (nền tảng "Grok").

- Trait dịch vụ: `GrokImageHost`.
- Node dùng: `node-grok-image`.
- Endpoint mặc định `https://api.x.ai`; đổi qua `base_url` của credential.
- xAI chưa có API video công khai → không có node video Grok (xem MEMORY.md).
