# lvc-google

Provider gọi **API chính thức** của Google để tạo ảnh (Imagen 3) và video (Veo). Người dùng tự nhập
API key Google (cùng key với Gemini) trong màn **Tài khoản AI → API key**.

- Trait dịch vụ: `GoogleImageHost` / `GoogleVideoHost` (engine cấp qua `NodeCtx::services`).
- Node dùng: `node-google-image`, `node-google-video`.
- Endpoint mặc định `https://generativelanguage.googleapis.com`; đổi được qua `base_url` của credential.

Khác hẳn provider `lvc-veo3` (điều khiển trình duyệt Flow). Đây là HTTP thuần, không trình duyệt.
