# lvc-license — ghi nhớ kỹ thuật

## Mô hình đe doạ (chống server giả / chống crack)

Nói thẳng giới hạn: client chạy trên máy kẻ tấn công thì KHÔNG có biện pháp nào tuyệt đối. Mục tiêu
là nâng chi phí để crack tốn công hơn mua license, và chặn các cách crack rẻ tiền nhất.

| Cách tấn công | Biện pháp | Ở đâu |
|---|---|---|
| Server giả trả phản hồi "hợp lệ" | Chữ ký ECDSA P-256 BẮT BUỘC + nonce lặp lại + gắn `device_id` + khớp key + skew thời gian | `verify::check_license_response` |
| Server giả qua hosts-file / DNS | Client sản phẩm `https_only`, không theo chuyển hướng | `LicenseClient::new_hardened` |
| MITM bằng CA tự cài (Fiddler, mitmproxy) | `rustls-tls` dùng root CA ĐÓNG GÓI SẴN, KHÔNG đọc kho chứng chỉ Windows → CA lạ không được tin | `Cargo.toml` reqwest `default-features = false` |
| MITM bằng proxy hệ thống | `no_proxy()` bỏ qua proxy hệ thống/`HTTP(S)_PROXY` | `new_hardened` |
| **Thay public key trong exe** (cách crack thực tế nhất) | Vân tay SHA-256 của key, che bằng XOR, kiểm ở NHIỀU chỗ | `integrity::embedded_key_intact` |
| Vá giao diện để bỏ qua màn khoá | Cổng BACKEND: lệnh làm việc thật gọi `LicenseState::require_licensed` | `commands/license.rs` + các lệnh `run_start`, `script_run`… |
| Lùi đồng hồ để kéo dài hạn | `last_seen` chỉ tiến + quyền trong bộ nhớ dùng `Instant` (đơn điệu) | `session::time_trustworthy`, `Grant` |
| Kích hoạt 1 lần rồi tách mạng dùng mãi | `MAX_OFFLINE_SECS` (7 ngày) tính từ lần kích hoạt online gần nhất | `session::time_trustworthy` |
| Dịch ngược / đọc symbol | `[profile.release]` LTO fat + strip | `Cargo.toml` gốc |

## Quy ước

- Cổng backend ở lớp WRAPPER `#[tauri::command]` (không phải `_impl`) để test `_impl` không cần cấp
  quyền. Lệnh làm việc MỚI (chạy workflow, gọi AI, dựng video…) PHẢI thêm tham số
  `license: tauri::State<'_, LicenseState>` và gọi `license.require_licensed()?` đầu hàm.
- Quyền (`Grant`) CHỈ ở bộ nhớ backend. KHÔNG tin cờ nào từ giao diện.
- Lỗi cho UI luôn CHUNG (`VERIFY_FAILED`, `LICENSE_REQUIRED`) — không nói lớp nào chặn.
- `integrity.rs`: khi ĐỔI public key phải tính lại `FP_MASKED`; test `van_tay_khop_key_nhung` đỏ là
  tín hiệu quên bước này (công thức che ghi ở doc hằng số).

## Cố ý KHÔNG làm (và vì sao)

- **Anti-debug (`IsDebuggerPresent`)**: chống được tay ngang, nhưng công cụ chuyên dụng
  (ScyllaHide…) qua được trong vài phút, trong khi dễ báo nhầm người dùng có debugger và cần thêm
  `unsafe`. Lợi ích thấp.
- **Pin chứng chỉ TLS riêng của máy chủ**: server giả không ký được phản hồi nên pin chỉ thêm lớp
  phụ; rủi ro khoá cứng toàn bộ người dùng khi nhà cung cấp xoay chứng chỉ. Cần biết kế hoạch xoay
  chứng chỉ của `tool.lvcmedia.vn` trước khi làm.

## Việc cần phía MÁY CHỦ để chống crack triệt để

Mọi thứ trên client đều có thể vá. Biện pháp mạnh nhất cần server: token ngắn hạn ký bởi server cấp
lại mỗi lần chạy workflow / định kỳ (heartbeat), và server từ chối khi license bị thu hồi. Hiện
client chỉ kiểm tra OFFLINE tối đa 7 ngày giữa hai lần gặp server.

## Đánh đổi đã chọn

- `no_proxy()` + `https_only`: máy chỉ ra internet qua proxy bắt buộc sẽ KHÔNG đăng nhập được. Nếu
  có khách báo lỗi kết nối chỉ khi ở mạng công ty, đây là nghi phạm đầu tiên.
- `MAX_OFFLINE_SECS = 7 ngày`: người dùng offline lâu hơn phải đăng nhập + nhập key lại (cần mạng).
- Chỉnh đồng hồ lùi quá 1 giờ so với lần mở app trước → bị coi là lùi giờ, phải đăng nhập lại.
