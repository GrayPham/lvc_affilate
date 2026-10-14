# Agent kịch bản affiliate

Sinh kịch bản video nhiều phân cảnh CHUYÊN cho quảng cáo affiliate (gắn link hoa hồng) bằng AI
text (Gemini hoặc OpenAI) — tự chèn công thức Hook mở đầu → Lợi ích/điểm đau → CTA cuối, không cần
tự viết chỉ dẫn hệ thống.

- Đầu vào: cổng `brief` (nối từ node Prompt/node khác) hoặc gõ trực tiếp ở tham số "Sản phẩm /
  chương trình affiliate".
- Đầu ra: cổng `out`, mỗi dòng một cảnh — nối thẳng vào cổng `prompt` của node tạo video (vd Dola).
- Nhà cung cấp AI: chọn giữa các API key (Gemini/OpenAI) đã có trong kho Tài khoản AI — chỉ hiện
  nền tảng nào thực sự có API key.
- Loại: `script_agent`
- Hợp đồng đầy đủ: `manifest.json`