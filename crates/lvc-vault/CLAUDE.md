# `lvc-vault` — kho bí mật (P4c, ADR-R017)

Mã hoá bí mật tài khoản AI (cookie Dola, API key Gemini/OpenAI, mật khẩu/TOTP) trước khi ghi DB.

## Luật cứng
- **Nơi DUY NHẤT chạm plaintext bí mật.** Không module nào khác được giải mã.
- **Không bao giờ log plaintext** (cookie/key/mật khẩu). `Debug` của `CredentialRow`/`Vault` không in secret.
- **Không để plaintext ra IPC.** Lệnh Tauri trả `CredentialView` đã che (`secret_hint` = `••••1234`
  hoặc mô tả cookie). Secret chỉ đi VÀO (`add`/`update_secret`); ra ngoài chỉ khi `lease_secret`
  gọi NỘI BỘ lúc thực thi node, không qua IPC.
- **DPAPI phạm vi user** (`DpapiBox`) trên Windows — ciphertext gắn máy/tài khoản Windows, chuyển
  máy là mất (đúng ý). Test dùng `PlainBox` (KHÔNG mã hoá — chỉ test, cấm bản release).
- FFI Win32 chỉ trong `dpapi.rs` (`#![allow(unsafe_code)]`, mỗi khối có ghi chú SAFETY).

## Không làm
- KHÔNG dựng lại chữ ký chống bot / giả thiết bị (đó là việc của kênh API app — đã quyết định
  không port, xem `crates/providers/lvc-dola/MEMORY.md`).
- Verifier chỉ gọi endpoint list-models CÔNG KHAI bằng key (GET thường), không ký, không né.

## Kiểm tra
- `cargo test -p lvc-vault` (DPAPI round-trip chạy thật trên Windows; verify qua wiremock).
