# LVC Shop — Đăng nhập + License Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cổng bản quyền cho LVC Shop — đăng nhập tài khoản → kích hoạt license (verify ECDSA P-256 offline), chặn vào app khi chưa hợp lệ, auto-login offline tới khi hết hạn; đổi tên hiển thị LVC Shop.

**Architecture:** Crate thuần-client mới `lvc-license` (giống `lvc-vault`, được dùng reqwest/tokio) chia 5 file một-trách-nhiệm: `verify` (PURE: canonical JSON + ECDSA + 4 lớp guard), `crypto` (ECDH+HKDF+Fernet tự viết), `device` (device_id/info), `client` (reqwest), `session` (cache mã hoá). `lvc-app` thêm lệnh Tauri + state; frontend thêm `LicenseGate` bọc `App`.

**Tech Stack:** Rust (p256/hkdf/hmac/aes/cbc/sha2/base64/serde_json/reqwest-rustls/chrono/winreg), Tauri 2, React/TS (zod).

**Spec:** `docs/superpowers/specs/2026-10-21-lvc-shop-license-login-design.md`

## Global Constraints

- MSRV 1.85 — KHÔNG let-chains. `clippy::pedantic` + `-D warnings` (gồm biến thể `-p lvc-app --features fake-driver`).
- KHÔNG log bí mật (password, license key, JWT, session plaintext, Fernet key). UI chỉ thấy trạng thái/thông điệp chung.
- rustls-only — KHÔNG crate kéo OpenSSL (KHÔNG crate `fernet`).
- `lvc-core` giữ THUẦN (không đụng tới). `lvc-license` KHÔNG phải node → không vướng ADR-R014.
- GIỮ identifier `vn.lvcmedia.seedance` + data dir + DB (chỉ đổi chuỗi hiển thị).
- Hằng số: `API_URL=https://tool.lvcmedia.vn`, `TOOL_CODE=lvc-shop`, `PUBLIC_KEY_PEM` (P-256 SPKI trong spec).
- Chữ ký BẮT BUỘC (thiếu `signature` = loại). Clock skew 86400s, KHÔNG chống lùi đồng hồ.
- Cổng chất lượng: `powershell -ExecutionPolicy Bypass -File scripts/check.ps1` XANH (chạy với `$env:CARGO_TARGET_DIR=D:/LVCMedia/_lvcrt`).

## Review Focus

- **Non-ASCII trong `data` khi verify chữ ký** (vd `license_info` có tiếng Việt): canonical phải escape `\uXXXX` chữ thường + separators có dấu cách, nếu không chữ ký FAIL → không ai kích hoạt được. → Task 1.
- **Thiếu `signature` trong phản hồi** (server giả bỏ chữ ký — đúng lỗ hổng Veo3 từng bị bẻ): phải LOẠI, không "bỏ qua kiểm". → Task 2.
- **Phát lại (nonce sai) / chép license sang máy khác (device_id sai) / sai key**: phải LOẠI. → Task 2.
- **Mở app offline với session đã hết hạn**: về màn login, không crash/không cho vào. → Task 5.
- **Mất mạng lúc kích hoạt**: trả "Lỗi kết nối" gọn, không panic. → Task 6.

---

### Task 1: Crate `lvc-license` + canonical JSON + verify chữ ký (PURE)

**Files:**
- Create: `crates/lvc-license/Cargo.toml`, `crates/lvc-license/src/lib.rs`, `crates/lvc-license/src/verify.rs`
- Modify: `Cargo.toml` (workspace: thêm member `crates/lvc-license` + deps `p256`/`hkdf`/`hmac`/`rand`/`chrono`/`winreg`)

**Interfaces:**
- Produces:
  - `pub fn canonical_json(v: &serde_json::Value) -> String` — khớp Python `json.dumps(sort_keys=True)`: sort key đệ quy, escape non-ASCII `\uXXXX` (chữ thường, surrogate pair >U+FFFF), separators `", "`/`": "`.
  - `pub fn verify_signature(pubkey_pem: &str, data: &serde_json::Value, signature_b64: &str) -> bool` — base64 chuẩn → `p256::ecdsa::Signature::from_der` → `VerifyingKey::verify(canonical_json(data).as_bytes(), &sig)`; mọi lỗi → `false`; `signature_b64` rỗng → `false`.

- [ ] **Step 1: Write failing tests** in `crates/lvc-license/src/verify.rs`
  - `canonical_sort_va_space`: `canonical_json(json!({"b":1,"a":2}))` == `{"a": 2, "b": 1}`.
  - `canonical_escape_non_ascii`: `canonical_json(json!({"n":"ăn"}))` == `{"n": "\u0103n"}`.
  - `canonical_nested_sort`: object lồng trong mảng cũng sort key.
  - `verify_chu_ky_dung_true` + `verify_chu_ky_sai_false` + `verify_signature_rong_false`: ký `data` bằng `SigningKey` sinh trong test (DER→base64), pubkey từ `VerifyingKey` tương ứng (PEM). Sửa 1 byte data → false.
- [ ] **Step 2: Run** `CARGO_TARGET_DIR=/d/LVCMedia/_lvcrt cargo test -p lvc-license verify::` → FAIL (crate/hàm chưa có).
- [ ] **Step 3: Scaffold crate** (Cargo.toml: edition/rust-version workspace, deps p256[`ecdsa`,`pem`]/serde_json/base64/sha2; lib.rs `pub mod verify;`) + thêm member & workspace deps. Implement `canonical_json` (đệ quy, tự viết escaper) + `verify_signature`.
- [ ] **Step 4: Run** test → PASS.
- [ ] **Step 5: Commit** (`feat(license): crate lvc-license + canonical JSON + verify chu ky ECDSA`).

---

### Task 2: 4 lớp guard `check_license_response` (PURE)

**Files:** Modify `crates/lvc-license/src/verify.rs`

**Interfaces:**
- Consumes: `canonical_json`, `verify_signature` (Task 1).
- Produces:
  - `pub struct LicenseInfo { pub key: String, pub expires_at: String, pub tool_id: Option<String>, pub max_devices: Option<i64>, pub device_id: String }` (Serialize/Deserialize).
  - `pub enum GuardError` (thiserror) — biến thể: `NotObject`, `MissingData`, `NoSignature`, `BadSignature`, `NotValid`, `NonceMismatch`, `MissingLicenseInfo`, `DeviceMismatch`, `KeyMismatch`, `MissingTimestamp`, `TooOld`. Thông điệp tiếng Việt khớp Veo3 (vd "phan hoi khong co chu ky").
  - `pub fn check_license_response(result: &serde_json::Value, pubkey_pem: &str, expected_nonce: &str, expected_device_id: &str, expected_key: &str, now_unix: i64, max_skew_secs: i64) -> Result<LicenseInfo, GuardError>` — thứ tự: object→`data`→signature bắt buộc→verify→`status=="success"`&`license_valid`→nonce(nếu expected≠"")→`license_info`→device_id(nếu≠"")→key(nếu≠"")→timestamp skew. Parse ISO (hỗ trợ `Z`) so với `now_unix`.

- [ ] **Step 1: Write failing tests** (helper `signed(data)->Value` dùng SigningKey test, gắn `signature`):
  - `guard_ok_tra_license_info`: response hợp lệ → `Ok(LicenseInfo{key,device_id,...})`.
  - `guard_thieu_signature_loai`: bỏ field `signature` → `Err(NoSignature)`.
  - `guard_nonce_sai_loai` / `guard_device_sai_loai` / `guard_key_sai_loai`.
  - `guard_timestamp_qua_cu_loai`: `now_unix - ts > 86400` → `Err(TooOld)`; trong 86400 → Ok.
- [ ] **Step 2: Run** `cargo test -p lvc-license verify::guard` → FAIL.
- [ ] **Step 3: Implement** `LicenseInfo`, `GuardError`, `check_license_response`.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** (`feat(license): 4 lop guard check_license_response`).

---

### Task 3: Fernet + encrypt_data (ECDH→HKDF→Fernet)

**Files:** Create `crates/lvc-license/src/crypto.rs`; Modify `lib.rs` (`pub mod crypto;`)

**Interfaces:**
- Produces:
  - `pub fn fernet_encrypt(key32: &[u8; 32], plaintext: &[u8], now_unix: i64, iv: [u8; 16]) -> String` — token Fernet urlsafe-b64: `0x80 ++ ts(8B BE) ++ iv ++ AES128CBC(key=key32[16..], iv, pkcs7(plaintext)) ++ HMAC_SHA256(key=key32[..16], tất cả phần trước)`.
  - `pub fn fernet_decrypt(key32: &[u8; 32], token: &str) -> Option<Vec<u8>>` — dùng cho session (Task 5); kiểm HMAC trước, bỏ qua TTL.
  - `pub fn encrypt_data(server_pub_pem: &str, data: &serde_json::Value) -> Result<String, CryptoError>` — ephemeral P-256 (OsRng) → ECDH `diffie_hellman` (X 32B) → `Hkdf::<Sha256>::new(None, &x).expand(b"license verification", 32)` → `fernet_encrypt(key, serde_json::to_vec(data)?, now, random_iv)` (CHÚ Ý: `serde_json::to_vec` thường, KHÔNG canonical) → `base64(ephemeral_pub_DER ++ token_bytes)`.
  - `pub enum CryptoError`.

- [ ] **Step 1: Write failing tests:**
  - `fernet_round_trip`: `fernet_decrypt(key, &fernet_encrypt(key, b"hello", 0, [0;16])) == Some(b"hello")`.
  - `encrypt_data_server_doc_lai`: sinh server keypair trong test; `encrypt_data(server_pub_pem, &data)`; đóng vai SERVER: base64-decode → tách DER(91B) ephemeral pub + token → ECDH(server_priv, ephemeral_pub) → HKDF cùng info → `fernet_decrypt` → `serde_json::from_slice` == `data`. (Chứng minh ECDH+HKDF+Fernet khớp.)
- [ ] **Step 2: Run** `cargo test -p lvc-license crypto::` → FAIL.
- [ ] **Step 3: Implement** (deps thêm: hkdf, hmac, aes, cbc, rand; AES-128-CBC qua `cbc::Encryptor<aes::Aes128>`).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** (`feat(license): Fernet tu viet + encrypt_data ECDH/HKDF`).

---

### Task 4: `device.rs` — device_id / device_info

**Files:** Create `crates/lvc-license/src/device.rs`; Modify `lib.rs`

**Interfaces:**
- Produces:
  - `pub fn device_id() -> String` — `sha256_hex( machine_uuid|hostname|processor|os )`. Windows `machine_uuid`: registry `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid` (winreg), fallback `"unknown"`. `hostname`=env `COMPUTERNAME`, `processor`=env `PROCESSOR_IDENTIFIER`, os=`"Windows"`.
  - `pub fn device_name() -> String` (= hostname).
  - `pub fn device_info() -> serde_json::Value` — `{os, os_version, machine, processor, hostname, tool_version, cpu_id, machine_id}` best-effort.

- [ ] **Step 1: Write failing tests:**
  - `device_id_hex_64_on_dinh`: `device_id()` dài 64, chỉ hex, gọi 2 lần bằng nhau.
  - `device_info_co_field_bat_buoc`: có khoá `os`,`hostname`,`processor`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement** (winreg; `sha2`). **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** (`feat(license): device_id + device_info`).

---

### Task 5: `session.rs` — cache mã hoá + auto-login hạn

**Files:** Create `crates/lvc-license/src/session.rs`; Modify `lib.rs`

**Interfaces:**
- Consumes: `crypto::{fernet_encrypt,fernet_decrypt}` (Task 3), `verify::LicenseInfo` (Task 2).
- Produces:
  - `pub struct Session { pub username: String, pub license: LicenseInfo, pub device_id: String, pub saved_at: i64 }` (Serde).
  - `pub fn session_key(device_id: &str, salt: &str) -> [u8; 32]` — `sha256(device_id[..32] ++ salt)`.
  - `pub fn save_session(path: &Path, s: &Session, salt: &str) -> std::io::Result<()>` — ghi `fernet_encrypt(key, json(s))`.
  - `pub fn load_session(path: &Path, salt: &str, now_unix: i64) -> Option<Session>` — đọc→decrypt→parse; `None` nếu thiếu/hỏng/`now_unix >= expires_at(license)`.

- [ ] **Step 1: Write failing tests** (tempdir):
  - `session_round_trip`: save rồi load (now < expires) → `Some(s)` bằng nhau.
  - `session_het_han_tra_none`: `expires_at` quá khứ → `None`.
  - `session_file_hong_tra_none`: ghi rác → `None`.
- [ ] **Step 2: Run** → FAIL. **Step 3: Implement**. **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** (`feat(license): session cache ma hoa + auto-login theo han`).

---

### Task 6: `client.rs` — reqwest authenticate / verify_license

**Files:** Create `crates/lvc-license/src/client.rs`; Modify `lib.rs`

**Interfaces:**
- Consumes: `crypto::encrypt_data`, `verify::{check_license_response,LicenseInfo}`, `device`.
- Produces:
  - `pub struct LicenseClient { api_url: String, pubkey_pem: String, tool_code: String, device_id: String, device_info: serde_json::Value, http: reqwest::Client }`
  - `pub fn LicenseClient::new(api_url: impl Into<String>, pubkey_pem: impl Into<String>, tool_code: impl Into<String>) -> Self` (tests tiêm mock url + test pubkey).
  - `pub async fn authenticate(&self, username: &str, password: &str) -> Result<String, LicenseError>` — POST form `/api/v1/auth/login` → `access_token`.
  - `pub async fn verify_license(&self, license_key: &str, auth_token: &str, username: &str) -> Result<LicenseInfo, LicenseError>` — build `verify_data` (nonce 16B hex qua rand) + `auth_data` → `encrypt_data` cả hai → POST JSON `/api/v1/verify/secure-verify` `{encrypted_payload, tool_code, auth_token}` → `check_license_response(..., now=Utc::now().timestamp(), 86400)`.
  - `fn report_tamper(&self, reason: &str)` best-effort (không await/không throw).
  - `pub enum LicenseError { Network, BadCredentials, VerifyFailed(String), Guard }` (`Display` → thông điệp UI chung; KHÔNG kèm bí mật).

- [ ] **Step 1: Write failing tests** (`wiremock`):
  - `authenticate_200_tra_token`: mock `/api/v1/auth/login` → `{access_token:"t"}` → `Ok("t")`.
  - `authenticate_401_bad_credentials` → `Err(BadCredentials)`.
  - `verify_license_phan_hoi_ky_dung_ok`: test tự sinh server keypair; client dùng pubkey đó; mock `/verify/secure-verify` đọc nonce từ `encrypted_payload`?? — thay vì giải mã, mock trả `data` chứa `nonce` LẤY TỪ request KHÔNG khả thi → **thiết kế test:** cho `check_license_response` chấp nhận `expected_nonce=""` khi bỏ kiểm nonce? KHÔNG. Thay vào: test gọi trực tiếp `verify_license` với mock trả response đã ký cho một `nonce` cố định, và client sinh nonce ngẫu nhiên → nonce sẽ lệch. → **Giải pháp:** tách hàm thuần `build_verify_result_check` đã test ở Task 2; ở Task 6 chỉ test tầng HTTP: `authenticate` (trên) + `verify_license_mat_mang_tra_network` (mock down → `Err(Network)`) + `verify_license_500_tra_verifyfailed`. (Đường ký hợp lệ đã được Task 2 phủ thuần; đường mạng phủ ở đây.)
- [ ] **Step 2: Run** `cargo test -p lvc-license client::` → FAIL.
- [ ] **Step 3: Implement** (reqwest workspace; `chrono` cho timestamp ISO; `rand` nonce).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** (`feat(license): client reqwest authenticate + verify_license`).

---

### Task 7: Hằng số + lệnh Tauri + gate state

**Files:**
- Modify: `crates/lvc-license/src/lib.rs` (consts `PUBLIC_KEY_PEM`/`API_URL`/`TOOL_CODE` + salts + `pub fn default_client()`), `crates/lvc-license/src/session.rs` (nếu cần path helper)
- Create: `crates/lvc-app/src/commands/license.rs`
- Modify: `crates/lvc-app/Cargo.toml` (dep `lvc-license`), `crates/lvc-app/src/commands/mod.rs` (`pub mod license;`), `crates/lvc-app/src/lib.rs` (invoke_handler + manage `LicenseState`), `crates/lvc-app/src/state.rs` (hoặc license.rs tự giữ) + `paths.rs` (thư mục session)

**Interfaces:**
- Consumes: `lvc_license::{default_client, LicenseClient, Session, save_session, load_session, device_id}`.
- Produces (lệnh Tauri, trả JSON đã che — KHÔNG bí mật):
  - `license_status() -> { activated: bool, username: Option<String>, expires_at: Option<String> }` — `load_session`; auto-login offline.
  - `license_login(username, password) -> { ok: bool }` — `authenticate`; lưu `(token, username)` vào state in-memory (KHÔNG ghi đĩa trừ khi "ghi nhớ").
  - `license_activate(license_key) -> { ok: bool, expires_at: String }` — cần đã login; `verify_license` → `save_session`.
  - `license_logout() -> { ok: bool }` — xoá file session + state.
  - `LicenseState { client: LicenseClient, session_dir: PathBuf, auth: std::sync::Mutex<Option<(String,String)>> }`.

- [ ] **Step 1: Write failing tests** `crates/lvc-app/src/commands/license.rs` (impl-level, mock server qua wiremock + tempdir):
  - `license_status_chua_kich_hoat_false`: dir rỗng → `activated=false`.
  - `license_login_sai_tra_ok_false` (mock 401).
  - `license_status_sau_save_session_true`: ghi session hợp lệ (now<exp) → `activated=true`, đúng username.
- [ ] **Step 2: Run** `cargo test -p lvc-app license` → FAIL.
- [ ] **Step 3: Implement** consts + `default_client` + lệnh + state + đăng ký `invoke_handler` + manage state trong `setup`.
- [ ] **Step 4: Run** → PASS. Chạy fixtures nếu đụng command list: `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
- [ ] **Step 5: Commit** (`feat(license): hang so + lenh Tauri license_login/activate/status/logout`).

---

### Task 8: Frontend `LicenseGate` + IPC + bọc App

**Files:**
- Create: `app/src/components/LicenseGate.tsx`
- Modify: `app/src/ipc/commands.ts` (+ `licenseStatus`/`licenseLogin`/`licenseActivate`/`licenseLogout` + zod schema), `app/src/App.tsx` (bọc `<LicenseGate>`)

**Interfaces:**
- Consumes: 4 lệnh Tauri (Task 7).
- Produces: `LicenseGate` — mount gọi `licenseStatus()`; `activated` → render `children`; chưa → form 2 bước (Bước 1 user+pass → `licenseLogin`; Bước 2 nhập mã → `licenseActivate` → set activated). Lỗi hiện thông điệp chung; đang xử lý thì disable nút.

- [ ] **Step 1: Write failing test** `app/src/components/LicenseGate.test.tsx` (vitest + mock ipc): khi `licenseStatus` trả `{activated:false}` → thấy ô "Tên đăng nhập"; khi `{activated:true}` → render children (text mốc).
- [ ] **Step 2: Run** `pnpm -C app test -- LicenseGate` → FAIL.
- [ ] **Step 3: Implement** ipc funcs + `LicenseGate` + bọc trong `App.tsx`.
- [ ] **Step 4: Run** test → PASS; `pnpm -C app typecheck`.
- [ ] **Step 5: Commit** (`feat(license): man khoa LicenseGate 2 buoc + boc App`).

---

### Task 9: Đổi tên hiển thị LVC Shop

**Files:** Modify `crates/lvc-app/tauri.conf.json` (`productName`, `app.windows[].title`), `app/index.html` (`<title>`), file sidebar branding ("LVC Affiliate"/"Affiliate video studio").

- [ ] **Step 1:** Grep `"LVC Affiliate"` + `"Affiliate video studio"` toàn repo UI/config để liệt kê điểm đổi (KHÔNG đụng `identifier`).
- [ ] **Step 2: Implement** đổi chuỗi hiển thị → "LVC Shop" (+ tagline phù hợp). GIỮ `identifier: vn.lvcmedia.seedance`.
- [ ] **Step 3: Verify** `pnpm -C app typecheck && pnpm -C app build`; grep xác nhận không còn "LVC Affiliate" trong UI; `identifier` không đổi.
- [ ] **Step 4: Commit** (`feat(brand): doi ten hien thi LVC Shop (giu identifier)`).

---

## Kết thúc
- Chạy `scripts/check.ps1` (CARGO_TARGET_DIR=_lvcrt) — TẤT CẢ XANH.
- Review toàn nhánh (reviewer fresh, model mạnh nhất) theo executing-plans.
- Bàn giao: người dùng tự chạy **kích hoạt live** bằng tài khoản+mã test của họ (nhập mật khẩu trong app, không qua tôi) và báo kết quả; nếu chữ ký fail do canonical lệch số/float → chỉnh `canonical_json` theo vector thật.
