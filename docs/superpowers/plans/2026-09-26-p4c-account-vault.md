# Kế hoạch P4c — Kho tài khoản AI (vault)

> Spec: `2026-09-25-node-modules-design.md` §7 (ADR-R017). Nối tiếp P4a (xong), P4b (chỉ lớp giao thức).
> Nghiệm thu: migration `credentials` chạy; DPAPI mã hoá/giải mã round-trip; tab "Tài khoản AI" thêm/
> nhập/kiểm tra/bật-tắt/gắn proxy cho Dola · Google · ChatGPT · API key; **bí mật KHÔNG bao giờ ra UI**;
> di trú tài khoản Dola cũ (`accounts_meta`) sang; `scripts/check.ps1` xanh.

## Nguyên tắc bảo mật (cứng)
- Bí mật (cookie JSON, API key, mật khẩu, TOTP secret) mã hoá bằng **Windows DPAPI phạm vi user**
  (`CryptProtectData`) trước khi ghi DB. Cột `secret BLOB` chỉ chứa ciphertext.
- IPC ra UI **không bao giờ** mang plaintext bí mật — chỉ `masked` (vd `••••1234`) + metadata không nhạy cảm.
- Không log plaintext bí mật ở bất kỳ tầng nào.
- Xuất/nhập kho (tuỳ chọn, cuối P4c hoặc P4d): AES-GCM + Argon2 với mật khẩu người dùng (DPAPI gắn máy).

## Crate mới: `lvc-vault`
```
crates/lvc-vault/
  Cargo.toml       deps: lvc-core, lvc-store, serde, thiserror; [target.'cfg(windows)'] windows-sys (Win32_Security_Cryptography)
  src/lib.rs       trait SecretBox { encrypt(&[u8]) -> Vec<u8>; decrypt(&[u8]) -> Result<Vec<u8>> }
  src/dpapi.rs     DpapiBox (windows) — CryptProtectData/CryptUnprotectData, entropy cố định theo app
  src/plain.rs     PlainBox (chỉ dev/test không phải windows / feature) — KHÔNG dùng bản release
  src/vault.rs     Vault: CRUD credentials qua CredentialRepo + SecretBox; trả CredentialView (đã mask)
  src/mask.rs      mask API key / cookie / mật khẩu
```
- `SecretBox` là trait để test không cần Windows (PlainBox) — bản thật luôn DpapiBox trên Windows.
- `Vault` cầm `CredentialRepo` (lvc-store) + `Arc<dyn SecretBox>`; là nơi DUY NHẤT chạm plaintext.

## lvc-store: migration 0004 + CredentialRepo
- `migrations/0004_credentials.sql`: bảng `credentials` (đúng §7 spec) + index `(provider, enabled)`.
- `CredentialRepo` (giống AccountRepo): `insert`, `list(provider?)`, `get`, `update_secret`, `set_enabled`,
  `set_proxy`, `set_status`, `set_cooldown`, `touch_used`, `delete`. Trả `CredentialRow` (secret là BLOB thô).
- Di trú Dola: khi khởi động, mỗi `accounts_meta` chưa có `credentials` tương ứng → tạo credential
  `provider=dola, kind=session, label=<name>, meta={profile_dir, browser}`; secret = cookie hiện có
  (đọc `accounts/<name>/cookies.json` nếu có) mã hoá; giữ nguyên `accounts_meta` (browser vẫn dùng profile).

## Provider verifier (không né phát hiện)
- `trait CredentialVerifier { async fn verify(&self, secret, meta) -> VerifyOutcome }`.
- Dola: đã có (`Engine::verify` qua trình duyệt) — bọc lại.
- OpenAI / Gemini: gọi endpoint list models công khai bằng API key (reqwest, không ký gì) → ok/expired/quota.
- Google/ChatGPT login: P4c chỉ lưu + kiểm định dạng; verify bằng đăng nhập để P4d/sau (cần trình duyệt).

## Tauri commands (lvc-app)
| Command | Ghi chú |
|---|---|
| `credential_list(provider?) -> Vec<CredentialView>` | view đã mask |
| `credential_add(provider, kind, label, secret, meta) -> CredentialView` | secret vào, không ra |
| `credential_update(id, {label?, secret?, meta?, enabled?, proxy_id?})` | |
| `credential_verify(id) -> VerifyOutcome` | |
| `credential_delete(id) -> bool` | |
- Giữ lệnh `account_*` cũ (Dola qua trình duyệt) hoạt động; tab mới đọc `credential_*`.

## UI: tab "Tài khoản AI"
- Nhóm theo provider: Dola · Google · ChatGPT · API key (Gemini/OpenAI…).
- Thêm/nhập cookie (Dola), dán API key, nhập email/mật khẩu/TOTP (login); bật-tắt; gắn proxy; kiểm tra.
- Bí mật hiển thị dạng `••••1234`; ô nhập là write-only (gửi đi, không đọc lại).
- Tab "Tài khoản" (Dola) hiện tại giữ nguyên hoặc gộp thành một nhóm — quyết định khi làm UI.

## Task
1. `lvc-vault` crate: SecretBox + DpapiBox + PlainBox + mask + test round-trip (PlainBox trên mọi OS; DpapiBox test `#[cfg(windows)]`).
2. lvc-store: migration 0004 + CredentialRepo + test schema/CRUD (`tests/credentials_repo.rs`).
3. lvc-vault Vault (CRUD + mask + di trú Dola) + test.
4. Verifier OpenAI/Gemini (wiremock) + bọc Dola.
5. lvc-app: commands credential_* + fixtures + di trú lúc bootstrap.
6. UI: tab Tài khoản AI + zod types + store + test.
7. Ghi nhớ/CLAUDE.md: `lvc-vault/CLAUDE.md` (luật: nơi duy nhất chạm plaintext, không log, không ra IPC).
