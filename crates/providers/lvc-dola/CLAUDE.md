# Provider `lvc-dola` — luật

Client/dịch vụ Dola dùng chung cho mọi node Dola (`dola_video`, sau: `dola_image`).

- **Không biết khái niệm node** (không phụ thuộc `lvc-node-api`), không phụ thuộc engine/store/tauri.
- Tài nguyên host giữ (pool tài khoản, limiter, bảng `tasks`) → khai *trait dịch vụ* ở đây
  (`DolaVideoHost`), engine impl, node lấy qua `NodeCtx::services`.
- P4b: port client API app Android từ `A:\ToolLVC\lvcseedance\src\api\dola_app.py` (CHỈ ĐỌC bản
  Python). Test bằng wiremock + vector ký chụp từ Python — không bao giờ gọi dola.com trong test.
- Không log cookie/sessionid/device id/chữ ký.
