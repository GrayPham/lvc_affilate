# Module `lvc-browser::veo3` — luật

Lớp CDP thật cho Google Labs Flow/Veo3 (spec `docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md`
v2, ADR-R018). Xem `CLAUDE.md` gốc mục "Node độc lập" + "Chia module bên TRONG".

## Khác `dola/` ở điểm nào

- `dola/` đi qua trait `BrowserDriver` (submit/poll cho chat Dola) — `veo3/` **không** dùng trait
  đó (nó gắn với mô hình conversation/poll riêng của Dola, không hợp với captcha/harvest). `veo3/`
  gọi `chromiumoxide` trực tiếp.
- Module này **tự đứng** — không phụ thuộc crate provider `lvc-veo3` (giống `dola/` không phụ
  thuộc `lvc-dola`). `lvc-engine` là nơi duy nhất nối 2 bên (impl trait `VeoCaptchaHost`/
  `VeoSessionHost` của `lvc-veo3` bằng hàm trong module này).

## `captcha.rs` — điều bắt buộc phải giữ đúng

- Trình duyệt KHÁCH (profile tạm, KHÔNG cookie tài khoản nào) — KHÔNG BAO GIỜ nạp cookie tài
  khoản vào đây. Xem `MEMORY.md` lý do (đo thật: mint trong phiên account → luôn bị chặn).
- KHÔNG headless, KHÔNG minimize — cả hai là tín hiệu xấu cho reCAPTCHA. Ẩn bằng
  `window-position` âm (đẩy ra ngoài màn hình), không phải bằng cờ `headless`.
- `Page.setBypassCSP` PHẢI gọi TRƯỚC khi điều hướng tới trang mint (chỉ áp cho document nạp SAU).
- KHÔNG gọi module này (hoặc bất kỳ chỗ nào dùng nó) là "pool" khi viết tài liệu tiếng Việt —
  dễ hiểu lầm (xem trao đổi 2026-09-28 trong lịch sử hội thoại lưu ở đây qua git blame nếu cần).

## Kiểm tra

- `cargo test -p lvc-browser --lib veo3` — chỉ test THUẦN (dựng chuỗi JS, ngưỡng `should_rotate`)
  KHÔNG mở Chrome thật.
- Chạy thật (mở Chrome khách, mint 1 token) là hành động sống — dù rủi ro gần như 0 (không đụng
  tài khoản nào), vẫn hỏi người dùng trước khi tự chạy thử, theo luật CLAUDE.md gốc mục 2.
