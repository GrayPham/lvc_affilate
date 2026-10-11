# lvc-seedance

Provider gọi **API chính thức** ByteDance/Volcengine (Ark) để tạo ảnh (Seedream) và video (Seedance).
Người dùng tự nhập API key Volcengine Ark + model/endpoint id của họ trong màn **Tài khoản AI →
API key** (nền tảng "Seedance").

- Trait dịch vụ: `SeedanceImageHost` / `SeedanceVideoHost`.
- Node dùng: `node-seedance-image`, `node-seedance-video`.
- Endpoint mặc định `https://ark.cn-beijing.volces.com`; đổi qua `base_url` của credential.
