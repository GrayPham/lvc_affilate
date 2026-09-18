# Thiết kế — LVC Seedance v2: Rust + Tauri, workflow kéo thả

> Ngày: 2026-09-16 · Nguồn: `A:\ToolLVC\lvcseedance` (Python, v0.60) · Đích: `A:\ToolLVC\lvc_seedancev2`
> **Baseline Python:** commit `8ec1cfac` (2026-09-16 21:34:28 +0700). Mọi "khớp Python" trong tài liệu này tính theo commit đó.
> Trạng thái: **chờ duyệt** — chưa viết code.

## 1. Bối cảnh

`lvcseedance` là công cụ desktop Windows tự động hoá dịch vụ AI `dola.com` để tạo video.
Kiến trúc hiện tại: FastAPI + patchright (Playwright vá tàng hình) + SQLite + một file
`web/index.html` 99KB vanilla JS, chạy trong cửa sổ pywebview. Hợp đồng API đã tiến hoá
tới **v1.19** qua **24 ADR** — đây là tài sản thiết kế quý nhất và là chuẩn đối chiếu
cho bản viết lại.

**Bản chất kỹ thuật cần nhớ:** đây *không* phải REST client. Công cụ điều khiển trình
duyệt thật để thao tác UI của Dola, rồi lấy kết quả bằng `page.evaluate(fetch(...))` với
`credentials:"include"` — tức chạy fetch *bên trong* trang để mượn cookie và origin.

### 1.1 Vì sao bắt buộc phải có trình duyệt

| Rào cản | Chi tiết đo được trong code |
|---|---|
| CAPTCHA trượt | iframe `bdcaptcha.html`; OpenCV `Canny(100,200)` + `matchTemplate(TM_CCOEFF_NORMED)`; kéo `.captcha-slider-btn` theo quỹ đạo easing bậc 5 |
| Chống fingerprint | `patchright`, `--disable-blink-features=AutomationControlled`, `locale=ja-JP`, `timezone_id=Asia/Tokyo` |
| Token chống bot | `msToken`, `s_v_web_id` đọc từ cookie trình duyệt — **không tính toán được** |
| Extension bắt buộc | `extensions/dola30` (mở khoá 30s + watermark) → ép chạy headed |
| Google OAuth + TOTP | máy trạng thái 12 bước, có nhận diện trang xác minh danh tính |
| Opera VPN | ghi `freedom.proxy_switcher` vào `Default/Preferences` để mở vùng Asia |

`src/api/dola_client.py` (55KB, aiohttp thuần) là **code chết** — chỉ `CREDIT_FAIL_PATTERN`
và `CreditError` được import thật. ADR-005 ghi rõ đường HTTP thuần đã bị bác. Tuy nhiên file
này chứa **đặc tả giao thức đã giải mã** và là tài liệu tham chiếu cho node tạo ảnh.

## 2. Phạm vi

### Trong phạm vi
1. Workflow kéo thả dạng node graph (mới)
2. Tab quản lý tài khoản: cookie, auto-login, **xoay proxy** (mới)
3. Đa luồng, nhận dữ liệu video đổ về
4. Bảo mật license key 2 bước

### Ngoài phạm vi (giai đoạn này)
- Máy chủ license `tool.lvcmedia.vn` (giữ nguyên giao thức, không đụng)
- Chế độ web/LAN (`desktop.py` mode "web")

### Bị loại bỏ hẳn (hệ quả của ADR-R010)
- HTTP server và toàn bộ ~45 route
- **Tab API Keys** và bảng `api_keys`
- `KeyConcurrencyLimiter`, `api_key_hash`, hạn mức theo key, `allowed_durations` theo key
- Middleware `license_gate` và cái hack tự tiêm `x-admin-key` cho loopback

## 3. Quyết định kiến trúc

### ADR-R001 — Workspace nhiều crate, lõi thuần tách khỏi I/O

```
crates/
  lvc-core/      modes, scenes, node graph, validation — THUẦN, không I/O
  lvc-store/     sqlx + SQLite, migration, repository
  lvc-browser/   trait BrowserDriver -> impl chromiumoxide
  lvc-media/     ffmpeg orchestration, validate ảnh, ghép cảnh
  lvc-license/   license client, HWID, lưu trữ an toàn
  lvc-engine/    scheduler: limiter động, account pool, DAG executor
  lvc-app/       Tauri commands + events
ui/              trình soạn node graph
```

**Vì sao:** giữ tinh thần ADR-001 gốc — `modes.py`/`scenes.py` thuần nên test được không
cần trình duyệt. Nguyên tắc này đã chứng minh hiệu quả qua 24 đợt phát triển.

**Bác:** giữ bố cục phẳng như bản Python — Rust cần ranh giới crate để cưỡng chế
"lõi không import browser" lúc biên dịch thay vì bằng script kiểm tra tên file.

### ADR-R002 — `trait BrowserDriver` bọc chromiumoxide

Người dùng chọn CDP thuần Rust (`chromiumoxide`) để có một binary duy nhất.

**Rủi ro đã nêu rõ:** `chromiumoxide` gọi `Runtime.enable` — đúng thứ patchright vá để
tránh bị phát hiện. Dola dùng risk-control của ByteDance (mã `710022004` = captcha,
`710022002` = rate limit).

**Giảm thiểu:** mọi thao tác browser đi qua `trait BrowserDriver`. Nếu P1 đo thật thấy
bị chặn, thay driver mà không đụng phần còn lại. Chi phí trait ≈ 0.

```rust
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    async fn launch(&self, profile: &ProfilePath, opts: LaunchOpts) -> Result<Session>;
    async fn add_cookies(&self, s: &Session, cookies: &[Cookie]) -> Result<()>;
    async fn submit(&self, s: &Session, req: &SubmitRequest) -> Result<ConversationId>;
    async fn poll_and_download(&self, s: &Session, c: &ConversationId, out: &Path)
        -> Result<VideoArtifact>;
}
```

### ADR-R003 — DAG executor thay fan-out bắn-rồi-quên

Bản Python tạo N cảnh = N `asyncio.create_task` độc lập, **không có barrier**. Node
"Ghép phân cảnh" bắt buộc phải chờ toàn bộ cảnh xong, nên đây là thay đổi kiến trúc thật.

**Bác:** giữ fire-and-forget rồi thăm dò DB để biết khi nào xong — đua điều kiện và
không biểu diễn được phụ thuộc trong đồ thị.

### ADR-R004 — Account pool là dịch vụ tài nguyên, không phải node

Node "Tạo video" **xin** lease tài khoản từ scheduler. Nếu tài khoản là cổng vào của node,
logic xoay tài khoản / hạn mức ngày / cooldown sẽ rò rỉ vào từng node.

### ADR-R005 — Event thay cho polling 10 giây

Dashboard hiện poll `GET /api/admin/tasks` mỗi 10000ms. Tauri `emit` đẩy sự kiện tức thì.

### ADR-R006 — Gộp 2 SQLite thành 1, thêm index

`tasks.db` + `pool_usage.db` → một DB. Bản gốc **không có index nào** và dùng một
connection + `threading.Lock` toàn cục gọi đồng bộ từ event loop (chặn executor).

## 4. Mô hình Node Graph

### 4.1 Kiểu cổng

`Text` · `Image` · `Video`

### 4.2 Danh sách node

| Node | Vào | Ra | Nguồn |
|---|---|---|---|
| Prompt | – | `Text[]` | port `scenes::split_bulk` |
| Tải ảnh lên | – | `Image` | port `media::save_uploaded_image` |
| Lấy ảnh từ URL | `Text` | `Image` | port `download_one_image` (giữ chống SSRF) |
| **Tạo ảnh** | `Text` + `Image?` | `Image` | **mới** — hai backend, xem ADR-R009 |
| Tạo video | `Text` + `Image[]?` | `Video` | port `generate_video`, tách đôi |
| Xoá logo | `Video` | `Video` | port `postprocess::remove_logo` |
| Chuẩn H.264 | `Video` | `Video` | port `ensure_web_codec` |
| **Ghép phân cảnh** | `Video[]` | `Video` | **mới** — node rào chắn |
| Lưu | `Video`/`Image` | – | port chính sách `_task_out_dir` |

### 4.3 Map ngầm và node rào chắn

**Map ngầm:** node `Prompt` nhả 20 dòng → `Tạo video` chạy 20 lượt, giữ thứ tự qua
`scene_index` (1-based). Người dùng không phải kéo 20 node.

**Barrier:** `Ghép phân cảnh` có cổng vào kiểu *collect* — chờ toàn bộ nhánh thượng nguồn
hoàn tất mới chạy.

### 4.4 Tách `generate_video` làm đôi

Hàm gốc gộp 6 việc. Tách theo đường nối mà `resume_video` đã chứng minh tồn tại:

```rust
async fn submit(&self, req: &SubmitRequest) -> Result<ConversationId>;
async fn poll_and_download(&self, conv: &ConversationId, out: &Path)
    -> Result<VideoArtifact>;
```

Hệ quả tốt: đóng app giữa chừng vẫn resume được vì `conversation_id` đã trong DB.

## 5. Scheduler và đồng thời

### 5.1 DynamicLimiter

`tokio::sync::Semaphore` **không co permit lại được**, nên phải tự viết.
`Drop` không async được → dùng `AtomicUsize` + CAS, không dùng `Mutex`.

```rust
pub struct DynamicLimiter {
    active: AtomicUsize,
    threads: watch::Receiver<usize>,   // cài đặt người dùng 1..100
    accounts: watch::Receiver<usize>,  // số tài khoản đang bật lịch
    notify: Notify,
}

impl DynamicLimiter {
    fn effective(&self) -> usize {
        (*self.threads.borrow()).min(*self.accounts.borrow()).max(1)
    }

    pub async fn acquire(self: &Arc<Self>) -> Permit {
        let mut threads = self.threads.clone();
        let mut accounts = self.accounts.clone();
        loop {
            let cur = self.active.load(Ordering::Acquire);
            if cur < self.effective() {
                if self.active.compare_exchange_weak(
                    cur, cur + 1, Ordering::AcqRel, Ordering::Acquire).is_ok()
                {
                    return Permit { inner: Arc::clone(self) };
                }
                continue;
            }
            tokio::select! {
                _ = self.notify.notified() => {}
                _ = threads.changed()      => {}
                _ = accounts.changed()     => {}
            }
        }
    }
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.inner.active.fetch_sub(1, Ordering::Release);
        self.inner.notify.notify_waiters();
    }
}
```

Giữ đúng ngữ nghĩa ADR-016: tăng luồng → đánh thức ngay; giảm luồng → không cắt job
đang chạy, lượt xin tiếp theo mới chờ.

> **Sửa sau review P2 Task 15 (2026-09-18):** đoạn mã trên có **race mất wakeup** —
> `self.notify.notified()` được tạo *sau* khi kiểm `active < effective()`, còn
> `notify_waiters()` chỉ đánh thức waiter *đã đăng ký*; nếu `Permit` drop đúng khe đó,
> waiter treo vĩnh viễn dù có chỗ. Bản hiện thực trong `lvc-engine::limiter` tạo
> `Notified` + `enable()` **trước** khi đọc `active` ở mỗi vòng lặp (`tokio::pin!`). Hai
> nhánh `watch::changed()` không bị (edge-triggered theo version). Giảm `effective()`
> giữa `load` và CAS có thể nhận thừa vài permit trong đúng khoảnh khắc đó — chấp nhận
> theo ADR-016.

### 5.2 Khoá theo tài khoản

```rust
locks: DashMap<AccountId, Arc<tokio::Mutex<()>>>
```

- **`tokio::Mutex`**, không phải `std::Mutex` — khoá cố ý giữ qua `await` suốt phiên browser.
  Đây là *lease tài nguyên*, không phải data guard.
- **`try_lock()`** để bỏ qua tài khoản bận và xoay sang tài khoản khác.
- Trừ khi `prefer_account` → **chờ**, không xoay (ADR-011).

### 5.3 Hai tầng giới hạn

1. `DynamicLimiter` toàn cục (§5.1)
2. `tokio::Mutex` theo từng tài khoản (§5.2)

Bản gốc có ba tầng; tầng `KeyConcurrencyLimiter` theo API key biến mất cùng HTTP server
(ADR-R010).

## 6. Mô hình dữ liệu

### 6.1 Trạng thái task — đúng 4 giá trị

`queued` · `processing` · `completed` · `failed`

Không có trạng thái `cancelled`; huỷ ghi `status='failed', failure_code='cancelled'`.

### 6.2 `failure_code` — 15 giá trị

`cancelled` · `LOGGED_OUT` · `429` · `ACCOUNT_UNAVAILABLE` · `PROMPT_VIOLATION` ·
`REF_UPLOAD` · `PROFILE_BUSY` · `PROXY_DOWN` · `DURATION_UNAVAILABLE` · `AGENT_QUESTION` ·
`MODE_UNAVAILABLE` · `NO_CONVERSATION` · `RESTART` · `RATIO_UNAVAILABLE` · `AGENT_NO_VIDEO`
(+`RATIO_UNAVAILABLE`, `AGENT_NO_VIDEO` theo Python HEAD 3d07c19 `KNOWN_FAILURE_CODES` `server.py:404-406`, P2 Task 9;
`DOLA_REVOKED_STREAK`/`REAL_PERSON` để P3/P4)

`DEAD_CHAT_CODES = (PROMPT_VIOLATION, AGENT_QUESTION, AGENT_NO_VIDEO)` chặn resume — chat cũ vô dụng.

> Bản gốc nhận diện mã lỗi bằng **dò tiền tố chuỗi** trên text exception. Bản Rust dùng
> `enum` + `thiserror` — mã lỗi là kiểu, không phải chuỗi.

### 6.3 Bảng proxy (mới)

```sql
CREATE TABLE proxies (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL UNIQUE,
  label TEXT DEFAULT '',
  enabled INTEGER DEFAULT 1,
  health TEXT DEFAULT 'unknown',   -- unknown | ok | fail
  last_checked REAL DEFAULT 0,
  fail_count INTEGER DEFAULT 0,
  last_used_at REAL DEFAULT 0,
  exit_ip TEXT DEFAULT ''
);
```

### 6.4 Index bắt buộc

`(status)` · `(batch_id, scene_index)` · `(account, day, mode)` · `(status, retry_at)`

### 6.5 Di trú dữ liệu cũ

Người dùng đang có dữ liệu thật. Phải nhập được:
- `accounts/<name>/` — profile Chromium giữ nguyên tại chỗ
- `tasks.db` → bảng `tasks`
- `pool_usage.db` → `accounts_meta`, `usage`, `settings`
- `accounts_creds.json` → lưu trữ mã hoá mới

## 7. Tab tài khoản

### 7.1 Cookie (port nguyên trạng)

Netscape 7 cột tách bằng TAB; nhận `#HttpOnly_`; fallback `re.split(r"\s{2,}|\t")` khi
mất tab; trần 200KB. **Điền alias `*_ss`**: cookie tên `sessionid_ss`, `uid_tt_ss` được
nhân bản sang tên rút gọn với `httpOnly=true, secure=true`. Bắt buộc có `sessionid`
thuộc `*.dola.com`. **Không bao giờ ghi cookie ra file** — nạp thẳng vào profile.

### 7.2 Auto-login (port nguyên trạng)

Máy trạng thái 12 bước + TOTP (RFC-6238). Ranh giới quan trọng phải giữ:

- Tự xử lý được: `challenge/totp`, `challenge/az`, `challenge/pwd`
- **Dừng, báo làm tay**: `challenge/pa|dp|ipp|iap|idvre|idv|kpe|sk|skotp`,
  `signin/rejected`, `disabled/explanation`, `familylink`; chữ `本人確認`,
  `通常と異なる`, `recaptcha`, `choose a parent`

ADR-020: cố vượt xác minh = khoá tài khoản. **Không được cố.**

### 7.3 Xoay proxy (mới)

Chiến lược: `sticky` (như hiện tại) · `rotate` (ít dùng gần nhất trước, theo ADR-012) ·
`random`.

**Ràng buộc vật lý:** proxy gắn lúc `launch_persistent_context`, **không đổi được giữa
phiên**. "Xoay" xảy ra ở mỗi lần mở browser cho một job, không phải giữa chừng. Đây là
giới hạn của Chromium.

Proxy chết → `health='fail'`, `fail_count+1`, lấy proxy kế, thử lại **1 lần**;
vượt ngưỡng → `enabled=0` + báo UI.

Health check: giữ `api.ipify.org?format=json`, TTL 600s. socks5 không test được bằng
cách này (giới hạn sẵn có).

## 8. Giấy phép

### 8.1 Giao thức — giữ nguyên, không đụng server

1. `POST /api/v1/auth/login` (form-urlencoded) → JWT
2. `GET /api/v1/tools/list` → khớp `tool_code == "lvc_seedance"` → `tool_id`
3. `GET /api/v1/licenses/check/{tool_id}`
4. `POST /api/v1/verify/secure-verify` — ECDH P-256 → HKDF-SHA256
   (`info=b"license verification"`) → Fernet; đáp có chữ ký ECDSA P-256

Crate: `p256`, `ecdsa`, `hkdf`, `fernet`. Base URL ghi đè bằng `DOLA_LICENSE_API`.

### 8.2 Năm lỗ hổng phải vá

| # | Hiện tại | Vá |
|---|---|---|
| 1 | `Fernet(SHA256(device_id[:32] + salt hardcode))` — ai biết MAC+hostname là giải mã được | **DPAPI** (`CryptProtectData`) buộc vào tài khoản Windows + Argon2id, salt ngẫu nhiên mỗi máy |
| 2 | Mật khẩu plaintext trong `.credentials.enc` | **Không lưu mật khẩu**; lưu refresh token. Nếu server chưa hỗ trợ → mật khẩu dưới DPAPI |
| 3 | `if chk.success: return chk` — vào được dù `secure-verify` hỏng | Xoá nhánh; `secure-verify` là thẩm quyền duy nhất |
| 4 | `_verify_signature` trả `True` khi thiếu crypto | Biến mất do kiến trúc — crypto biên dịch tĩnh vào binary |
| 5 | HWID = `SHA256(uuid.getnode() + platform.node())` | MachineGuid + ProcessorId + Volume Serial, khớp **2/3** → đổi NIC không bị khoá oan |

### 8.3 Offline grace (mới)

Server ký blob `{license, expires_at, grace_days}` bằng P-256; client verify cục bộ bằng
public key nhúng sẵn → mất mạng vẫn dùng N ngày.

Hiện tại `restore_session()` gọi mạng mỗi lần khởi động, **không có grace** — mất mạng là
không mở được app.

## 9. ffmpeg

Bản gốc có đúng 4 lệnh, **không hề có ghép cảnh**. Binary tra bằng PATH
(`DOLA_FFMPEG_BIN`, `DOLA_FFPROBE_BIN`), timeout 600s, chạy qua
`create_subprocess_exec` (không bao giờ qua shell).

```
ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 <path>
ffprobe -v error -select_streams v:0 -show_entries stream=codec_name -of csv=p=0 <src>

ffmpeg -hide_banner -loglevel error -y -i <src> -vf <vf> \
  -c:v libx264 -preset veryfast -crf 18 -pix_fmt yuv420p \
  -c:a copy -movflags +faststart <dst>

ffmpeg -hide_banner -loglevel error -y -i <src> \
  -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p \
  -c:a aac -b:a 128k -movflags +faststart <dst>
```

Filter: crop → `crop=iw:ih-{n}:0:0` với `n = even(round(height * 0.08))`;
delogo → `delogo=x={x}:y={y}:w={w}:h={h}` từ `LOGO_BOX_REL = (0.74, 0.935, 0.20, 0.05)`
(dọc) hoặc `(0.82, 0.885, 0.15, 0.085)` (ngang), kẹp ≥1px mỗi cạnh, kích thước chẵn.

Lỗi ffmpeg **không fatal** — task vẫn completed với `error="LOGO_SKIPPED: ..."`.

### 9.1 Ghép cảnh (mới) — ADR-R008: chỉ nối thẳng

```
ffmpeg -hide_banner -loglevel error -y -f concat -safe 0 -i <list.txt> -c copy <dst>
```

Không mã hoá lại → nhanh, không giảm chất lượng. **Không** xfade, **không** nhạc nền,
**không** tự chuẩn hoá độ phân giải.

**Hệ quả phải xử lý tử tế:** concat demuxer đòi mọi file **cùng codec, cùng độ phân giải,
cùng timebase**. Các cảnh sinh từ cùng model và cùng tỉ lệ thì khớp; nhưng nếu người dùng
trộn 9:16 với 16:9 trong một đồ thị, ghép sẽ hỏng.

Xử lý: trước khi ghép, `ffprobe` mọi file đầu vào; nếu lệch → **fail sớm với thông báo rõ**
("cảnh 3 là 1280x720, các cảnh khác là 720x1280 — hãy dùng cùng tỉ lệ"), **không** tự ý
scale. Người dùng sửa tỉ lệ rồi chạy lại.

Chuẩn hoá/xfade/nhạc nền để lại cho sau, khi có nhu cầu thật.

## 10. Chế độ và model

`MODES = (fast, pro, pro_max)`

| Mode | Nút Dola | Thời lượng | Hạn mức ngày | Timeout |
|---|---|---|---|---|
| fast | 高速 → 動画を作成 | 10 | 30 | 300s |
| pro | プロ → スキル → 動画の生成 | 10, 15 | 10 | 1800s (gia hạn 3600s) |
| pro_max | プロ, không スキル, lệnh `/Create Video` | 10, 15 | 10 | 3600s |
| agent | プロ (chat tự nhiên, không skill) | 10, 15 | 30 | 1800s |

30s đã bị bỏ (ADR-033/034 Python): scene vẫn chấp nhận `dur=30` trong TXT,
`fit_duration_to_mode` kẹp về mức mode cho phép.

Model: `seedance_v1.0`, `seedance_v2.0`, `seedance_v2.5` (alias `seedance-1.0`/`seedance-2.0`/
`seedance-2.5` ở API công khai). Nhãn UI: `Dreamina Seedance 2.5`, `Dreamina Seedance 2.0高速`,
`Dreamina Seedance 2.0`, `Seedance2.0Fast`.

Logo: `off`, `crop`, `delogo`, `auto`.

Ở `pro`/`pro_max`, thời lượng và tỉ lệ **viết vào prompt** (tiếng Nhật) chứ không có menu:
`"15秒の縦動画（9:16）を1本作ってください。内容: ...。BGMなし、環境音のみ"`.

Reset hạn mức: 0h `Asia/Tokyo`.

## 11. Lộ trình

| GĐ | Nội dung | Nghiệm thu |
|---|---|---|
| P0 | Workspace + `lvc-core` + `lvc-store` + di trú DB cũ | Test lõi xanh **không cần browser**; dữ liệu cũ nhập được |
| P1 | `BrowserDriver` + chromiumoxide + cookie + login | **Đo thật trên Dola**: đăng nhập được, không bị chặn |
| P2 | Scheduler + node Tạo video | Tạo 1 video thật end-to-end |
| P3 | Node graph UI + DAG engine + barrier | Kéo thả 3 cảnh → 3 video |
| P4 | Tạo ảnh + Ghép phân cảnh | Kịch bản hoàn chỉnh ra 1 file |
| P5 | Siết giấy phép + đóng gói | Bundle Tauri, DPAPI, offline grace |

**P1 là cửa tử.** Nếu chromiumoxide bị chặn, biết ngay ở GĐ1 chứ không phải sau khi đã
viết xong node graph.

## 12. Rủi ro

| Rủi ro | Mức | Giảm thiểu |
|---|---|---|
| chromiumoxide bị Dola phát hiện | **Cao** | `trait BrowserDriver`; P1 đo thật sớm |
| CAPTCHA port sang Rust sai | Trung bình | `imageproc` thuần Rust; giữ bộ ảnh mẫu để đối chiếu |
| Dola đổi DOM/nhãn tiếng Nhật | Trung bình | Gom mọi nhãn vào một chỗ (như `modes.UI_LABELS`) |
| Trick paste main-world gãy | Trung bình | Đếm `prepare_upload` + `/upload/v1/`; cảnh báo khi thừa |
| Di trú dữ liệu hỏng | Thấp | Chỉ đọc DB cũ, ghi sang DB mới; không sửa dữ liệu gốc |

## 13. Quyết định đã chốt (2026-09-16)

### ADR-R007 — React Flow cho trình soạn node graph

Chọn React Flow vì trưởng thành nhất, tài liệu và ví dụ nhiều, dễ tìm người sửa về sau.
Đánh đổi chấp nhận: bundle nặng hơn Svelte Flow → khởi động chậm hơn chút.

Hệ quả: `ui/` là React + TypeScript. Node tuỳ biến implement `NodeProps`; trạng thái đồ thị
đồng bộ hai chiều với backend qua Tauri events.

### ADR-R008 — Ghép cảnh chỉ nối thẳng, lệch thông số thì báo lỗi

Xem §9.1. Không xfade, không nhạc nền, không tự chuẩn hoá.

**Vì sao:** `-c copy` nhanh và không giảm chất lượng; tự ý scale khi lệch tỉ lệ là đoán ý
người dùng. Báo lỗi rõ để họ tự quyết.

**Bác:** tự chuẩn hoá ngầm — người dùng không biết video đã bị scale, phát hiện muộn.

### ADR-R009 — Node tạo ảnh có hai backend, mặc định browser

```rust
pub enum ImageBackend { Browser, Http }
```

- **`Browser`** (mặc định): tái dùng toàn bộ hạ tầng đã có — cookie, captcha, chống
  fingerprint, xoay tài khoản, hạn mức ngày. Chắc chắn chạy.
- **`Http`** (tuỳ chọn): `POST /chat/completion` với `chat_ability.ability_type = 16`,
  prompt tiếng Trung `"帮我生成图片：{prompt}\n风格：{style}\n比例：{ratio}"`; poll
  `/im/chain/single` `cmd:3100` tìm `block_type 2074` → `creations[].image.image_raw.url`.
  Cần `msToken` + `s_v_web_id` lấy từ cookie của tài khoản.

**Vì sao làm cả hai:** đường HTTP nhanh hơn nhiều và đa luồng thật, nhưng là code **chưa
từng chạy thật** trong sản phẩm. Có công tắc thì đo được thực tế mà không đánh cược.

Nếu `Http` dính risk-control (`710022004`/`710022002`) → tự rơi về `Browser` và ghi log.

### ADR-R010 — Bỏ hẳn HTTP server, chỉ dùng Tauri commands

**Vì sao:** không có client ngoài nào đang gọi. Bỏ được ~45 route, tầng xác thực
admin-key, và cái middleware tự tiêm `x-admin-key` vào `request.scope` cho loopback
(một hack khó bảo trì).

**Hệ quả dây chuyền — đáng kể:**
- Tab **API Keys** và bảng `api_keys` biến mất (khoá lưu plaintext, nay không còn vấn đề)
- `KeyConcurrencyLimiter` biến mất → còn **hai** tầng giới hạn thay vì ba (§5.3)
- `api_key_hash`, `daily_limit`/`allowed_durations` theo key, `client_concurrency_limit`
  bỏ khỏi bảng `tasks`
- Route tĩnh `/videos`, `/files/{id}.mp4`, `/uploads` biến mất → frontend đọc file qua
  Tauri asset protocol (`convertFileSrc`), vốn an toàn hơn vì bản cũ **không xác thực**
  hai route này

**Rủi ro:** nếu sau này cần client ngoài, phải dựng lại axum. Chấp nhận — YAGNI.

### ADR-R011 — Adopt Python HEAD `8ec1cfac` làm baseline (2026-09-16)

**Bối cảnh:** Python được phát triển song song, spec dựa trên snapshot 07/09; review cuối
P0 phát hiện lệch (mode agent, bỏ 30s…).

**Lựa chọn:** ghim baseline = HEAD lúc kết thúc P0.

**Bác:** giữ snapshot cũ (port thứ người dùng không còn dùng).

**Hệ quả:** P0.5 này; mọi phase sau đối chiếu với `8ec1cfac`; nếu Python tiếp tục đổi, mở
ADR mới chứ không trượt ngầm.

### ADR-R012 — Cửa tử P1 MỞ: chromiumoxide giữ được phiên Dola (2026-09-17)

**Đo thật:** 2026-09-17 15:24 UTC, máy Windows Server 2022, `crates/lvc-browser/examples/probe_profile.rs`,
bản copy profile Python `qertqegfdsacd` (14 profile gốc không đụng), binary
`ms-playwright/chromium-1234/chrome-win64/chrome.exe` (đúng bản patchright 1.62.3 dùng, UA Chrome/151),
headed, **không gửi prompt**. Kết quả: `Storage.getCookies` trước goto = 79/79 cookie
(27 dola, `sessionid` có); `goto /chat` + 6 s → `state: logged_in`, `has_sessionid: true`,
`composer: true`, 0 login marker, không `region-restricted`; screenshot thấy avatar tài
khoản + lịch sử chat + nút 高速 (ảnh **đã xoá** sau khi xem — chứa tên tài khoản; không lưu
vào repo). `navigator.webdriver=false`, `navigator.language=ja-JP`.

**Quyết định:** giữ chromiumoxide làm driver (ADR-R002 phương án A); **không** mở sidecar
Python. Tiến P2.

**Bẫy đã dẫm phải (ghi để không lặp) — đã sửa sau review toàn P1:**

1. **`--user-data-dir` TƯƠNG ĐỐI → mất cookie bất định.** Cùng binary, cùng DB (79 dòng,
   journal rỗng, integrity ok, mọi giá trị giải mã được bằng DPAPI + AES-GCM, hash host
   khớp), nhưng Chromium chỉ nạp 7–8/79 cookie, tập nạp đổi giữa các lần chạy, và
   `sessionid` luôn thiếu — kể cả khi đã dùng nguyên bộ cờ patchright. Đường dẫn tuyệt đối
   nạp 79/79. Cơ chế bên trong Chromium không xác định được từ ngoài; kết luận thực nghiệm
   đủ chắc (3 lần tương đối = thiếu, 3 lần tuyệt đối = đủ, trên **cả hai** binary). Driver ép
   `std::path::absolute` trong `ChromiumDriver::config`, ghim bằng test
   `config_ep_user_data_dir_tuyet_doi`.
2. **Kết luận "sai binary" ban đầu là SAI.** Đo 1 (Google Chrome 153) chạy với đường dẫn
   tương đối nên dính bẫy 1; đo lại (2026-09-17, headless, tuyệt đối) → Chrome 153 cũng nạp
   79/79 và `logged_in`; sau đó DB vẫn toàn cookie `v10` (81 dòng) — Chrome đọc được cookie
   DPAPI của Chromium. Quy tắc "mở profile bằng đúng binary đã tạo nó (cột `browser` trong
   `pool_usage.db`)" **vẫn giữ** vì App-Bound Encryption (`v20`) của Google Chrome ≥127 là
   một chiều theo tài liệu Chromium (Chromium không giải mã được `v20`) — chưa đo, không
   dựa vào việc lần này Chrome chưa ghi `v20`.

**Chưa đo / lệch so với Python, ghi nhận:** (a) chỉ đo headed; headless cho UA
`HeadlessChrome/153.0.0.0` — dấu hiệu bot rõ, nên P2 chạy **headed** (Python ở chế độ
`profile` cũng ép headed). (b) chromiumoxide thêm `--window-size=1280,900`,
`--remote-debugging-port=0`, `--disable-extensions` và ta thêm `--lang=ja-JP`; patchright
dùng `--remote-debugging-pipe`, không có 3 cờ kia (xem doc comment `PATCHRIGHT_DEFAULT_ARGS`).
(c) `Runtime.enable` là khác biệt duy nhất phát hiện được từ JS trang; nếu Task 18 (P2) bị
chặn ở bước gửi, phương án là fork chromiumoxide để evaluate qua isolated world.

**Đổi kèm:** `PATCHRIGHT_DEFAULT_ARGS` thay bộ cờ Puppeteer chép tay từ chromiumoxide bằng
bộ cờ **thật** patchright 1.62.3 truyền (bắt qua `DEBUG=pw:browser`), trừ cờ đã có trong
`LAUNCH_ARGS` và cờ chromiumoxide tự lo. Không phải nguyên nhân lỗi, nhưng là parity với
công cụ đã chạy ổn với Dola; phải rà lại mỗi khi nâng patchright/chromiumoxide.

**Chưa chứng minh (để P2 đo tiếp):** gửi prompt + nhận video qua CDP có bị Dola chặn ở
tầng `a_bogus`/fetch hook hay không — cửa tử này chỉ chứng minh *phiên đăng nhập* sống.

**Bác:** sidecar Python (đã bác lần 2 bởi người dùng với đầy đủ thông tin, 2026-09-17).

## 14. Câu hỏi còn mở

1. **Di trú**: có chạy song song bản Python và bản Rust một thời gian không, hay cắt hẳn?
2. **Bộ chọn tỉ lệ ở node**: giữ đủ 6 tỉ lệ (`9:16, 16:9, 1:1, 4:3, 3:4, 21:9`) hay rút gọn?
