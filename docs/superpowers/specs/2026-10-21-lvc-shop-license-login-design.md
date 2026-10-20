# LVC Shop — Đăng nhập + License (port 1:1 từ LVC Veo3)

> Thiết kế để DUYỆT trước khi code. Nguồn tham chiếu (CHỈ ĐỌC):
> `E:\ProjectAILVC\Lvc_veo3_automation` (Python/PyQt5).

## Ý định & vì sao

LVC Shop (hiện tên hiển thị "LVC Affiliate") cần **cổng bản quyền**: mở app phải đăng nhập tài
khoản rồi kích hoạt license mới vào được màn chính, giống hệt tool LVC Veo3. Server cấp phép đã có
sẵn tại `https://tool.lvcmedia.vn` với **cùng API như Veo3**, chỉ khác:
- `tool_code = "lvc-shop"` (Veo3 là `LvcVeo301`).
- Public key verify (ECDSA P-256) riêng của LVC Shop (người dùng đã cung cấp).

Đây là **port 1:1 phía CLIENT** sang Rust/Tauri 2. Server (ký license bằng private key) KHÔNG nằm
trong phạm vi này — chỉ gọi tới nó.

## Mục tiêu

1. Màn khoá khi mở app: Bước 1 đăng nhập `username`+`password`; Bước 2 nhập mã license
   `XXXX-XXXX-…`. Chưa có license hợp lệ thì KHÔNG render được màn chính.
2. Verify phản hồi server bằng **đúng 4 lớp guard** của Veo3 (chữ ký bắt buộc → nonce → device_id →
   khớp key + thời gian), bằng public key LVC Shop nhúng sẵn.
3. Sau kích hoạt: lưu **session mã hoá cục bộ**; lần sau mở app **tự đăng nhập offline** tới khi hết
   `expires_at` — không gọi server mỗi lần.
4. Khoá máy bằng `device_id` ổn định (SHA-256 từ Machine UUID + hostname + cpu + os).
5. Đổi **tên hiển thị** "LVC Affiliate" → "LVC Shop" ở tiêu đề cửa sổ + sidebar, **GIỮ NGUYÊN**
   identifier `vn.lvcmedia.seedance` (không mất DB/tài khoản đã có).

## Không làm (non-goals)

- KHÔNG dựng/ký license phía server (private key ở `tool.lvcmedia.vn`, ngoài phạm vi).
- KHÔNG verify license **hoàn toàn offline** — kích hoạt lần đầu BẮT BUỘC có mạng (đúng như Veo3).
  Chỉ auto-login offline từ session đã cache.
- KHÔNG đổi identifier / đường dẫn data dir / schema DB (tránh mất tài khoản + workflow người dùng).
- KHÔNG port lớp **obfuscate hằng số** (XOR+Base64 + self-fingerprint) của Veo3 ở v1 — nhúng pubkey/
  URL/tool_code trực tiếp. (Người dùng đã đưa pubkey dạng trần → chấp nhận. Có thể làm hardening sau,
  xem "Để lại cho builder".)

## Chống mục tiêu (anti-goals — hỏng kể cả khi "chạy")

- Khoá nhầm người đã trả tiền: máy sai giờ / đổi múi giờ / VPN / đổi card mạng KHÔNG được chặn
  (Veo3 cố ý nới clock skew 1 ngày, KHÔNG kiểm IP/MAC, KHÔNG chống lùi đồng hồ).
- Mất tài khoản AI / workflow do đổi data dir.
- **Lộ bí mật ra log/UI**: password, license key, JWT, session plaintext KHÔNG được log (luật #3
  CLAUDE.md). UI chỉ thấy trạng thái, không thấy giá trị.
- Để chữ ký thành "tuỳ chọn": thiếu `signature` = KHÔNG HỢP LỆ (đây đúng lỗ hổng Veo3 từng bị bẻ).

## Hằng số nhúng (lvc-shop)

- `API_URL = https://tool.lvcmedia.vn`
- `TOOL_CODE = lvc-shop`
- `PUBLIC_KEY_PEM` (ECDSA P-256 / SPKI — người dùng cung cấp):
  ```
  -----BEGIN PUBLIC KEY-----
  MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEiKkmCNoUo1pI/fILINFp/U8wF5MZ
  U6ZyQpmwf5qnDUe44e5lRiKRZIxc6PQVnCmpC+nkGXsHLYc0W++euqS//A==
  -----END PUBLIC KEY-----
  ```

## Giao thức (phải khớp server BYTE-FOR-BYTE)

Trích từ `src/core/license_client.py` + `src/core/license_response_guard.py`.

### a) Đăng nhập — `authenticate(username, password)`
`POST {api}/api/v1/auth/login`, body **form-urlencoded** `username`,`password`.
→ 200: `{access_token}` (JWT HS256). Client KHÔNG verify JWT, chỉ base64-decode payload lấy `sub`
(user_id). ≠200 → "Sai tài khoản".

### b) Kích hoạt — `verify_license(key)`
`POST {api}/api/v1/verify/secure-verify`, JSON:
```
{ "encrypted_payload": <b64>, "tool_code": "lvc-shop", "auth_token": <b64> }
```
- `encrypted_payload` = `_encrypt_data(verify_data)` với
  `verify_data = {license_key, device_id, device_name, device_info, tool_code, timestamp(ISO utc), nonce(16 byte hex)}`.
  - `device_info` = dict lồng (os, os_version, machine, processor, hostname, tool_version, cpu_id,
    machine_id) — sao chép đúng tên field best-effort. Nó NẰM TRONG payload mã hoá và KHÔNG xuất hiện
    ở bất kỳ lớp guard nào → không ảnh hưởng verify (server coi là thông tin).
- `auth_token` = `_encrypt_data({user, user_id, token, expires_at})`.

**`_encrypt_data(d)` (ECDH → HKDF → Fernet):**
1. Sinh cặp khoá ephemeral **P-256**.
2. ECDH với server pubkey → shared secret (toạ độ X, 32 byte).
3. `HKDF-SHA256(salt=None, info=b"license verification", length=32)` → khoá.
4. `Fernet(urlsafe_b64(khoá)).encrypt(json.dumps(d).encode())` → token Fernet (b64 urlsafe).
   `json.dumps(d)` ở đây là **THƯỜNG** (không sort, server tự `json.loads`) → KHÔNG dùng canonical.
5. `combined = ephemeral_pub_DER(SubjectPublicKeyInfo) ++ token`; trả `base64.b64encode(combined)`.

> **Fernet — KHÔNG dùng crate `fernet` (kéo OpenSSL).** Repo đi rustls-only. Client CHỈ ENCRYPT
> (không bao giờ decrypt → TTL không liên quan phía client). Tự viết Fernet-encrypt ~40 dòng bằng
> crate đã có: `aes` + `cbc` (AES-128-CBC, PKCS7) + `hmac` + `sha2` + `base64`. Token Fernet =
> `urlsafe_b64( 0x80 ++ timestamp(8B BE) ++ IV(16B) ++ ciphertext ++ HMAC_SHA256(signing_key, phần_trước) )`,
> trong đó key 32B tách: 16B đầu = signing key (HMAC), 16B sau = AES key.

### c) Verify phản hồi — `check_license_response` (4 lớp, ném lỗi chứ không trả bool)
Phản hồi: `{ data:{...}, signature:<b64 DER> }`.
1. **Chữ ký (BẮT BUỘC)**: `verify_signature(pubkey, data, signature)` —
   `payload = json.dumps(data, sort_keys=True).encode("utf-8")`;
   `pubkey.verify(b64decode(signature), payload, ECDSA(SHA256))`. Thiếu signature = loại.
2. `data.status=="success"` và `data.license_valid` truthy.
3. **nonce**: `data.nonce == nonce ta vừa gửi` (chống phát lại).
4. **device_id**: `data.license_info.device_id == device_id của ta` (chống chép máy).
5. **key**: `data.license_info.key == key đã nhập`.
6. **thời gian**: `|now - data.timestamp| <= 86400s`. KHÔNG chống lùi đồng hồ (cố ý).
→ Lưu `license_info` (`key, expires_at, tool_id, max_devices, device_id`).

> ⚠️ **ĐIỂM RỦI RO LỚN NHẤT — canonical JSON:** `json.dumps(data, sort_keys=True)` của Python mặc
> định `ensure_ascii=True` (escape ký tự non-ASCII thành `\uXXXX`) + `separators=(", ", ": ")` (CÓ
> dấu cách sau `,` và `:`). `serde_json` của Rust mặc định KHÔNG cách, KHÔNG escape non-ASCII →
> chữ ký sẽ **luôn fail**. Builder PHẢI viết bộ canonical riêng khớp Python và test bằng vector thật
> (một `data` + `signature` hợp lệ lấy từ server/Python).
>
> **Chi tiết canonical (bắt buộc đúng):** canonicalize trên **raw `serde_json::Value`** (KHÔNG parse
> vào struct có kiểu — để giữ nguyên kiểu số/bool/thứ tự server gửi); **sort key đệ quy**; escape
> non-ASCII thành `\uXXXX` **chữ thường** (và surrogate pair cho ký tự > U+FFFF); separators `", "`
> và `": "`. CHỈ dùng cho đường **chữ ký** (`check_license_response`), **TUYỆT ĐỐI không** cho
> `encrypt_data`. Keys toàn ASCII nên sort trùng Python. Rủi ro còn lại: format số float (payload
> hầu như toàn string/int/bool nên khả năng cao an toàn) — chỉ **vector thật (B2)** chốt được.

### d) device_id (`_generate_device_id`)
`sha256( machine_uuid | hostname | processor | os ).hexdigest()`.
Windows `machine_uuid`: `Win32_ComputerSystemProduct.UUID` (WMI/PowerShell), fallback registry
`HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`, fallback MAC. Cần **ổn định** qua reboot.

### e) Session cache + auto-login (`login_widget.py`)
Lưu 3 file **Fernet** (đây là nơi client VỪA encrypt VỪA decrypt — tự viết cả hai chiều) trong
**`app_config_dir` của Tauri** (file MỚI của lvc-shop, không migrate Veo3 → không đụng data dir cũ):
- credentials (user+pass, tuỳ chọn "ghi nhớ"), license_key, session (user+license+device_id).
Khoá Fernet = `urlsafe_b64( sha256(device_id[:32] + <salt>) )`, **3 salt riêng** cho 3 file (chuỗi
cố định của lvc-shop, local-only, server không quan tâm).
Mở app: nếu có session + `now < expires_at` → auto-login OFFLINE, vào thẳng; hết hạn → xoá, về màn
login.

## Kiến trúc Rust/Tauri (theo luật repo)

**Crate mới `crates/lvc-license/`** (giống `lvc-vault`: được dùng reqwest/tokio; KHÔNG phải node nên
không vướng ADR-R014). Chia file, mỗi file một trách nhiệm (luật chia module):
- `verify.rs` — **THUẦN**: `canonical_json` (khớp Python), `verify_signature` (p256 ECDSA),
  `check_license_response` (4 lớp, trả `Result`). Unit test bằng vector thật.
- `crypto.rs` — `encrypt_data` (ECDH p256 + HKDF + Fernet). Testable (ephemeral cố định trong test).
- `device.rs` — `device_id()` (registry MachineGuid + hostname + cpu + os).
- `client.rs` — reqwest: `authenticate`, `verify_license`, `report_tamper`.
- `session.rs` — lưu/đọc session mã hoá (Fernet, khoá từ device_id) + kiểm `expires_at`.
- `lib.rs` — ghép + hằng số (PUBLIC_KEY_PEM, API_URL=`https://tool.lvcmedia.vn`, TOOL_CODE=`lvc-shop`).

**Phụ thuộc:** `p256` (features `ecdh`,`ecdsa`,`pem`), `ecdsa`, `hkdf`, `sha2`, `hmac`, `aes`+`cbc`
(Fernet tự viết — đã có aes/cbc trong workspace), `base64`, `reqwest`(rustls, đã có workspace),
`serde`/`serde_json`, `winreg` (device id Windows), `chrono`/`time` (parse ISO + skew). **KHÔNG**
dùng crate `fernet` (kéo OpenSSL — xem ghi chú Fernet ở trên).

**Tauri (lvc-app):** lệnh mới — `license_login(username,password)`, `license_activate(key)`,
`license_status()` (đọc session/auto-login, trả trạng thái đã che), `license_logout()`. Bí mật chỉ
đi VÀO; trả ra chỉ trạng thái + thông điệp. Đăng ký trong `lib.rs` invoke_handler.

**Gate khởi động (frontend):** thêm `LicenseGate` bọc toàn app (`App.tsx`): khi mở gọi
`license_status()`; chưa hợp lệ → render màn **Đăng nhập → Kích hoạt** (2 bước, giống Veo3); hợp lệ →
render app hiện tại. Không có route nào vào được màn chính khi chưa qua.

**Đổi tên LVC Shop:** chỉ chuỗi hiển thị — `tauri.conf.json` `productName` + `app.windows[].title`,
`app/index.html` `<title>`, sidebar branding ("LVC Affiliate" / "Affiliate video studio"). GIỮ
`identifier`, đường dẫn data dir, tên DB.

## Trạng thái lỗi hiển thị (không mách nước)
Sai tài khoản / sai mã / sai máy / phản hồi giả → đều quy về thông điệp chung ("Không xác minh được
phản hồi máy chủ" cho nhóm chữ ký/máy; "Sai tài khoản"; "Hết hạn"; "Lỗi kết nối" khi mất mạng). Chi
tiết chỉ ghi log nội bộ, KHÔNG lộ lớp nào fail.

## Quyết định của tôi (call của tôi — không cần hỏi lại)
- **Fernet tự viết** bằng aes/cbc/hmac/sha2, KHÔNG dùng crate `fernet` (tránh OpenSSL).
- **canonical_json** trên raw `Value`, chỉ cho đường chữ ký (chi tiết ở mục Giao thức).
- **3 salt session** riêng của lvc-shop (local-only).
- **obfuscate hằng số**: để lại hardening sau (v1 nhúng trực tiếp — người dùng đã đưa pubkey trần).
- **device_id**: MachineGuid làm nguồn chính trên Windows; `processor` từ env `PROCESSOR_IDENTIFIER`,
  `os`="Windows" (chỉ cần ổn định nội bộ qua reboot, không cần byte-match Veo3).
- **3 file .enc** lưu trong `app_config_dir` Tauri (file mới, không đụng data dir cũ).
- **device_info** sao chép tên field best-effort (không nằm trong guard).
- **Dev oracle cho canonical**: so khớp output với `python -c "import json;print(json.dumps(...))"`.

## CẦN BẠN cung cấp (chặn verify end-to-end — xem câu hỏi khi bàn giao)
- **B2 — vector/tài khoản test**: 1 cặp `(data, signature)` thật từ server lvc-shop, HOẶC 1 tài khoản
  (user+pass) + 1 license key lvc-shop hợp lệ. Thiếu B2: vẫn **build + unit-test (canonical khớp
  Python) xanh** được, nhưng KHÔNG dám khẳng định kích hoạt THẬT chạy cho tới khi test live với server.
  (B1 — public key — đã có, nhúng ở mục "Hằng số nhúng".)

## Verify (khi làm xong)
- `cargo test -p lvc-license` (canonical JSON khớp Python; verify_signature với vector; 4 lớp guard
  mỗi lớp 1 ca loại; encrypt_data round-trip; session hết hạn → về login).
- `scripts/check.ps1` TẤT CẢ XANH.
- Thủ công: mở app → màn khoá; login sai → báo lỗi; login đúng → bước license; kích hoạt đúng → vào
  app; tắt/mở lại → auto-login không cần mạng tới khi hết hạn; tên hiển thị = LVC Shop; tài khoản/
  workflow cũ CÒN NGUYÊN.
