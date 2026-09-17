# P2 — Tạo video Dola end-to-end + Scheduler: Kế hoạch triển khai

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Từ một tài khoản đã đăng nhập (profile `accounts/<tên>/`), tạo **một video thật** trên dola.com — gửi prompt, lấy `conversation_id`, poll, tải `.mp4` về — và có bộ lập lịch (limiter động + khoá tài khoản + hạn mức ngày) để P3 gắn node graph lên.

**Architecture:** Ba tầng, tách theo I/O:
1. `lvc-core` (thuần): phân loại chữ Dola (limit/refusal/credit/hỏi lại), parse JSON `/im/chain/single`, chọn URL video không logo (base64/qAAB), parse proxy, timeout/duration theo mode.
2. `lvc-browser` (I/O): thao tác trang bằng CDP thô (tìm phần tử bằng JS → `Input.dispatchMouseEvent`; gõ bằng `Input.insertText`), luồng `submit` (ngôn ngữ → mode → composer → model/tỉ lệ/thời lượng → ảnh tham chiếu → gửi → captcha → conv id) và `poll_and_download` (HTTP reqwest với cookie lấy từ CDP, tải mp4 streaming).
3. `lvc-engine` (mới): `DynamicLimiter` (spec §5.1), `AccountPool` (chọn/xoay/cooldown/hạn mức, khoá `tokio::Mutex` theo tài khoản), `generate_video` điều phối + ánh xạ lỗi → `FailureCode`.

**Tech Stack:** chromiumoxide 0.9 · reqwest 0.12 (rustls, stream, socks, cookies **tắt** — tự đặt header Cookie) · regex (+ fancy-regex nếu Python dùng lookaround) · aes + cbc + sha2 + base64 (qAAB) · dashmap · tokio (sync, time) · uuid

**Spec:** `docs/superpowers/specs/2026-09-16-lvc-seedance-rust-design.md` §4.4, §5, §6.2, ADR-R002/R003/R004, ADR-R012. **Bản đồ Python HEAD `3d07c19`:** `docs/superpowers/notes/2026-09-17-python-video-pipeline.md` (gọi tắt **NOTES**, mọi `NOTES §x.y` bên dưới trỏ vào đó; mọi `file:line` Python là của `A:\ToolLVC\lvcseedance` @ `3d07c19`).

## Global Constraints

- `lvc-core` **không** phụ thuộc tokio/reqwest/sqlx/chromiumoxide/tauri, **không I/O** (gate `scripts/check.ps1` kiểm bằng `cargo tree`). `regex`, `serde_json`, `aes`/`cbc`/`sha2`/`base64` là thuần → cho phép.
- Clippy `all` + `pedantic` = warn, gate chạy `-D warnings`. PF-3: mọi `#[allow(clippy::…)]` phải hẹp và có comment lý do tiếng Việt.
- **Không log cookie/giá trị header Cookie/URL có chữ ký** ở mức trên `trace`; log tên cookie được, giá trị không.
- **Chế độ chạy = `profile`**: mỗi tài khoản một Chromium, `--user-data-dir` **tuyệt đối** (ADR-R012), **không extension**. Không port hub/wtabs/lvc_relay ở P2.
- Nhãn UI Dola là **tiếng Nhật** (`高速`, `プロ`, `動画を作成`, `比率`, `モデル`, `スキル`, `動画の生成`); regex chấp nhận thêm Việt/Anh y như Python (`NOTES §3.6-3.9`). Mọi nhãn/regex/JS gom vào **một** module `lvc-browser::dola::labels` (spec §12).
- JS chạy bằng `Runtime.evaluate` của chromiumoxide = **MAIN world** (khác Playwright isolated). Hệ quả: hook 30s và paste ảnh KHÔNG cần trick `<script>` tag — evaluate thẳng. Ghi rõ trong doc comment nơi dùng.
- Poll `/im/chain/single` bằng **HTTP reqwest** (đã chứng minh chỉ cần cookie, ADR-029 Python), **không** bằng fetch trong trang. UA = `navigator.userAgent` thật của browser vừa mở (không hardcode).
- Tải mp4: GET thuần, **không cookie**, timeout 300 s, đi qua proxy của tài khoản nếu có; `downloads/<account>_<YYYYmmdd_HHMMSS>.mp4`.
- Thời lượng: fast `10` không cần hook; fast `15`/`30` cần hook `DURATION_HOOK_JS` (30 chỉ với model 2.5); pro-like `10|15` ghi trong prompt (đã có `pro_prompt`/`agent_prompt`).
- `failure_code` đúng 13 giá trị của `lvc_core::FailureCode` + `None` (spec §6.2). Lỗi Python "không mã" (timeout poll, captcha, hết credit) → `None` + message có tiền tố như Python.
- **Test live tốn credit** (Task 18) — CHỈ chạy khi người dùng đồng ý trong phiên; mọi task khác phải xanh **không cần browser** (smoke `#[ignore]` chạy trên fixture HTML cục bộ, không gọi dola.com).
- Không đụng `A:\ToolLVC\lvcseedance` (DB, `accounts/`, code). Profile thử nghiệm = **bản copy**.

---

## Bố cục file

```
crates/lvc-core/src/
  dola_text.rs      # T1  regex + classify_dola_text / needs_confirmation / agent_question
  chain.rs          # T2  parse_chain, ChainPoll, video_ids, link_key, pick_new_conv_id
  video_url.rs      # T3  find_fallback_apis, decode_main_url (base64|qAAB), pick_best, replace_query
  proxy.rs          # T4a parse_proxy, ProxySpec
  mode.rs           # T4b allowed_durations(mode, model), route_30s, video_timeout
crates/lvc-browser/src/
  ops.rs            # T5  PageOps: eval_json, click_js, press, insert_text, set_files, wait_until, url, cookies, response counter
  dola/mod.rs
  dola/labels.rs    # T6  mọi JS + regex nhãn
  dola/session.rs   # T6  ensure_japanese, wait_settled, assert_logged_in, recent_conv_ids, raise_if_logged_out
  dola/compose.rs   # T7  select_mode, open_video_composer, clear_attachments, select_model/ratio/duration, type_and_send, duration hook
  dola/refs.rs      # T8  attach_reference_images (input | paste)
  dola/submit.rs    # T9  SubmitRequest, submit(), after_submit (captcha + conv id)
  dola/error.rs     # T9  DolaError → Option<FailureCode>
  dola/http.rs      # T10 DolaHttp: pull_chain, fetch_raw_video, download
  dola/poll.rs      # T11 poll_and_download, handle_poll
  dola/skill.rs     # T12 pro / pro_max / agent composer + confirmation reply
  driver.rs         # T13 proxy auth (Fetch.*) ; T14 trait mở rộng submit/poll_and_download + FakeDriver
  tests/fixtures/composer.html   # trang giả lập composer Dola cho smoke test
crates/lvc-engine/src/
  lib.rs, limiter.rs (T15), pool.rs (T16), generate.rs (T17), session_cache.rs (T17)
crates/lvc-engine/examples/gen_one.rs   # T18 CLI E2E thật
```

## Quyết định P2 (ruling — đã chốt trước khi chạy, ghi vào spec ở Task 18 dưới ADR-R013)

| # | Quyết định | Vì sao | Nếu sai tốn gì |
|---|---|---|---|
| R1 | Click = JS tìm phần tử → `scrollIntoView` → lấy rect → `Input.dispatchMouseEvent` (moved/pressed/released) tại tâm | Chuột thật như Playwright `locator.click`; không có locator engine trong chromiumoxide | Chỉ phải đổi 1 hàm `click_js` |
| R2 | Poll bằng reqwest, không evaluate fetch | Python `HttpSession.poll` chứng minh chỉ cần cookie; tách khỏi tab → resume/warm dễ | Nếu WAF soi TLS fingerprint: chuyển `pull_chain` sang evaluate `POLL_JS` (đã có sẵn trong labels) — 1 hàm |
| R3 | Không chờ cố định 20 s sau Enter; kiểm captcha mỗi 1 s **song song** với chờ conv id (30 s) | Python chờ 20 s vô điều kiện — phí; captcha xuất hiện thì bắt được ngay | 0 |
| R4 | Ngôn ngữ: xoá cookie `user_language_code`/`i18next` lúc mở + `DOLA_SET_JA_JS` + reload; **không** port đường UI Settings→言語 | Python chỉ dùng UI path khi JS path hỏng; hiếm | Nếu vẫn không ja → `MODE_UNAVAILABLE` sớm, người dùng đổi tay 1 lần trong profile |
| R5 | Không reuse chat (BATCH_REUSE_CHAT=0 mặc định Python); `submit` luôn chat mới; `poll_and_download` nhận conv id → resume được | Đơn giản, đúng mặc định | Không có |
| R6 | Captcha: chỉ **phát hiện** iframe `bdcaptcha.html` → lỗi `RiskControl` (cooldown 1800 s); không giải slider ở P2 | Giải slider cần OpenCV port (spec §12) — P2.5 | Tài khoản bị captcha nghỉ 30 phút |
| R7 | Bỏ balance pre-flight, logo removal/H264, auto_retry, real-person, Facebook/Google login ở P2 | NOTES §10.2; nghiệm thu P2 là 1 video | Không có |
| R8 | Session cache (giữ browser ≤ 900 s idle) có, đơn giản: `HashMap<account, (Session, last_used)>` + tick 30 s | Batch nhiều cảnh cùng tài khoản đỡ 5-8 s/lần mở | Nếu lỗi → tắt cache = mở/đóng mỗi lần |

---

### Task 1: `lvc-core::dola_text` — phân loại chữ Dola

**Files:**
- Create: `crates/lvc-core/src/dola_text.rs`
- Modify: `crates/lvc-core/src/lib.rs` (thêm `pub mod dola_text;` + re-export), `crates/lvc-core/Cargo.toml` (thêm `regex = "1"`; nếu Python dùng lookbehind/lookahead → `fancy-regex = "0.14"` và ghi rõ trong report pattern nào cần)
- Test: unit trong file

**Interfaces:**
- Consumes: không.
- Produces:
```rust
pub enum DolaTextClass { Limit, Refusal, Credit }
pub fn classify_dola_text(text: &str) -> Option<DolaTextClass>;   // video_worker_ui.py:84-95
pub fn looks_like_json(text: &str) -> bool;                        // modes.py:28-31
pub fn policy_rejection(latest: &[String]) -> Option<String>;      // modes.py:333-340 (trả chuỗi khớp)
pub fn is_portrait_hint(text: &str) -> bool;                       // PORTRAIT_RE video_worker_ui.py:76
pub fn is_generating(text: &str) -> bool;                          // DOLA_TEXT["generating"] modes.py:128-132
pub fn needs_confirmation(latest: &[String]) -> Option<ConfirmKind>; // modes.py:288-305
pub enum ConfirmKind { Confirm, Choice, Question, Duration }
pub fn confirmation_reply(kind: ConfirmKind, duration: u16) -> String; // modes.py:308-322, 161-170
pub fn agent_question(latest: &[String]) -> Option<String>;        // modes.py:350-358
pub const REFUSAL_NOTE: &str;                                       // video_worker_ui.py (tiền tố PromptViolationError)
```

- [ ] **Step 1: Đọc nguồn** — `A:\ToolLVC\lvcseedance\src\utils\modes.py` dòng 28-31, 57-139, 161-170, 288-358; `src\video\video_worker_ui.py` dòng 62-95. Chép **từng pattern** vào hằng `LazyLock<Regex>` với tên giống Python (`DAILY_LIMIT_PATTERN`, `GENERIC_REFUSAL_RE`, `EXPLICIT_CREDIT_RE`, `CREDIT_FAIL_RE`, `PORTRAIT_RE`, `POLICY_REJECT`, `GENERATING`, `CONFIRM_REQUEST`, `CHOICE_REQUEST`, `QUESTION`, `DURATION_REQUEST`, `AGENT_QUESTION`). Mỗi hằng có doc comment `// modes.py:NN`.
- [ ] **Step 2: Viết test RED** — tối thiểu 14 test, mỗi test một mẫu chữ thật lấy từ chính regex/nhận xét trong Python:
```rust
#[test] fn limit_nhat() { assert_eq!(classify_dola_text("動画生成の1日あたりの上限に達しました"), Some(DolaTextClass::Limit)); }
#[test] fn refusal_khong_phai_credit() { assert_eq!(classify_dola_text("このリクエストはポリシーに違反しています"), Some(DolaTextClass::Refusal)); }
#[test] fn credit_thang_refusal_khi_co_explicit_credit() { /* chuỗi khớp cả GENERIC_REFUSAL_RE lẫn EXPLICIT_CREDIT_RE → Credit */ }
#[test] fn credit_thuan() { assert_eq!(classify_dola_text("残高不足のため生成できません"), Some(DolaTextClass::Credit)); }
#[test] fn chu_thuong_khong_khop() { assert_eq!(classify_dola_text("猫が歩く動画を作ります"), None); }
#[test] fn json_bi_bo_qua() { assert!(looks_like_json("{\"a\":1}")); assert!(!looks_like_json("hello")); }
#[test] fn confirm_duration_tra_loi_dung_dinh_dang() { assert_eq!(confirmation_reply(ConfirmKind::Duration, 15), "15秒 (15 seconds). 15秒の動画を生成してください。"); }
#[test] fn agent_question_dau_hoi_cuoi() { assert!(agent_question(&["どの比率にしますか？".into()]).is_some()); }
#[test] fn agent_question_abc() { /* chuỗi có "A." … "B." */ }
#[test] fn generating_nhat() { /* mẫu từ DOLA_TEXT["generating"] */ }
// + portrait hint, policy_rejection trả chuỗi khớp, needs_confirmation 4 loại, thứ tự ưu tiên limit > refusal > credit
```
- [ ] **Step 3: Chạy RED** — `cargo test -p lvc-core dola_text` → FAIL biên dịch (module chưa có). Dán output không cắt vào report.
- [ ] **Step 4: Implement** theo đúng thứ tự kiểm tra của `classify_dola_text` (`video_worker_ui.py:84-95`): limit → (refusal|policy) và **không** explicit-credit → credit.
- [ ] **Step 5: GREEN + gate** — `cargo test -p lvc-core`, `powershell -File scripts/check.ps1`.
- [ ] **Step 6: Commit** — `feat(core): dola_text — phan loai chu Dola (limit/refusal/credit/hoi lai)`.

---

### Task 2: `lvc-core::chain` — parse `/im/chain/single`

**Files:**
- Create: `crates/lvc-core/src/chain.rs`; Modify: `lib.rs`, `Cargo.toml` (thêm `serde_json.workspace = true`)

**Interfaces — Produces:**
```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChainPoll {
    pub ok: bool, pub status: u16,
    pub texts: Vec<String>,         // mọi text_block (cắt 1000 ký tự)
    pub videos: Vec<String>,        // download_url bắt đầu "http"
    pub video_models: Vec<String>,  // song song với videos
    pub links: Vec<String>,         // link mp4 trong text (pro)
    pub latest: Vec<String>,        // khối chữ của TIN BOT MỚI NHẤT (user_type==2), cắt 4000
}
pub fn parse_chain(body: &serde_json::Value) -> ChainPoll;          // hub_browser.py:755-793 + POLL_JS video_worker.py:19-115
pub const LOGIN_INVALID_CODES: &[i64] = &[712012001];                // hub_browser.py:746
pub fn chain_http_status(body: &serde_json::Value) -> Option<u16>;   // status_code ∉ (0,None) → 401 nếu trong LOGIN_INVALID_CODES, else 500
pub fn video_ids(s: &str) -> Vec<String>;                            // VIDEO_ID_RE r"\b(v[0-9a-z]{10,})\b" video_worker_ui.py:1432-1437
pub fn link_key(url: &str) -> String;                                // host + path từ "/video/" video_worker_ui.py:1440-1456
pub fn pick_new_conv_id(url_tail: &str, known: &HashSet<String>) -> ConvPick; // video_worker_ui.py:678-689
pub enum ConvPick { New(String), Known(String), None }
```
Luật `parse_chain` (chép đúng POLL_JS): `messages = downlink_body.pull_singe_chain_downlink_body.messages`; `hasType = any(user_type != null)`; `latestIdx = hasType ? findIndex(user_type==2) : 0`; `content` là chuỗi JSON → parse, không phải mảng → bỏ; `text_block.text` → texts (+latest nếu mi==latestIdx) + quét link `/https?:\/\/[^\s)\]"'<>]+/` giữ link khớp `/mime_type=video_mp4|\.mp4(\?|$)|\/video\//i` không trùng; `block_type == 2074` → `creation_block.creations[type==2].video.download_url` bắt đầu `http` → videos + `video.video_model`.

- [ ] **Step 1: RED** — test với fixture JSON tự dựng (không có dữ liệu thật): (a) 2 tin, tin[0] user_type 1 (prompt của tool), tin[1] user_type 2 có 2074 → `latest` chỉ lấy tin bot, `videos.len()==1`; (b) không có `user_type` → latest = tin[0]; (c) content là chuỗi JSON; (d) content không parse được → bỏ qua không panic; (e) link markdown `[動画を再生する](http://v16-dola.dola.com/x/video/abc.mp4?x=1)` → links; (f) `download_url` không bắt đầu http → bỏ; (g) `chain_http_status`: `status_code: 712012001` → 401, `status_code: 5` → 500, `0`/thiếu → None; (h) `video_ids("…/v0d1234567890abc…")`; (i) `pick_new_conv_id("123456", known∌) → New`, `∈ → Known`, `"chat" → None`.
- [ ] **Step 2: Implement** (serde_json::Value, không struct hoá toàn bộ — chỉ đọc đường dẫn cần).
- [ ] **Step 3: GREEN + gate + commit** — `feat(core): chain — parse /im/chain/single (port POLL_JS/parse_chain)`.

---

### Task 3: `lvc-core::video_url` — chọn URL video không logo

**Files:** Create `crates/lvc-core/src/video_url.rs`; Modify `lib.rs`, `Cargo.toml` (`base64 = "0.22"`, `sha2 = "0.10"`, `aes = "0.8"`, `cbc = { version = "0.1", features = ["alloc"] }`).

**Interfaces — Produces:**
```rust
pub const RAW_PARAMS: [(&str, &str); 3] = [("channel","no"),("codec_type","8"),("logo_type","unwatermarked")]; // unwatermark.py:26
pub fn find_fallback_apis(video_model: &str) -> Vec<String>;   // unwatermark.py:51-83 (khoá "fallback_api", host vod-urls-mya.byteintlapi.com)
pub fn replace_query(url: &str, params: &[(&str, &str)]) -> String; // giữ query cũ, ghi đè khoá trùng
pub fn decode_main_url(main_url: &str, key_seed: Option<&str>) -> Option<String>; // base64 → "http…" | qAAB (unwatermark.py:153-171)
pub struct RawCandidate { pub url: String, pub bitrate: u64, pub w: u64, pub h: u64 }
pub fn pick_raw_video(body: &serde_json::Value) -> Option<String>; // unwatermark.py:202-222: max(bitrate + w*h), decode main_url
pub fn extract_unwatermarked_url(video_model: &str, download_url: &str) -> String; // video_worker.py:118-141
pub fn is_watermarked_url(url: &str) -> bool;                      // postprocess.py:58-63: chứa "watermark" và không "unwatermarked"
```
qAAB (`unwatermark.py:153-171`, `QAAB_SALT_HEX :24-25`): payload sau 4 byte `a8 00 01 00`; `k = SHA512(SHA512(key_seed[..32]) + SALT)`; key = k[..16], iv = k[16..32]; AES-128-CBC, PKCS7. Chép SALT hex **nguyên văn** từ Python.

- [ ] **Step 1: RED** — (a) `find_fallback_apis` trên chuỗi JSON `video_model` giả có 2 `fallback_api` (1 host đúng, 1 host khác) → chỉ host đúng, giữ thứ tự; (b) `replace_query("https://h/p?a=1&channel=x", RAW_PARAMS)` → có `channel=no`, `a=1` giữ; (c) `decode_main_url(base64("https://x/y.mp4"), None)` → Some; (d) qAAB **round-trip**: test tự mã hoá bằng cùng thuật toán (key_seed cố định, plaintext `https://ok/`) → `decode_main_url` trả lại — và một vector **âm** (salt sai → None); (e) `pick_raw_video` chọn theo `bitrate + w*h`; (f) `extract_unwatermarked_url` chọn `main_url` bitrate cao nhất, fallback `download_url`; (g) `is_watermarked_url`.
- [ ] **Step 2: Implement** — không unwrap; mọi lỗi decode → None.
- [ ] **Step 3: GREEN + gate + commit** — `feat(core): video_url — fallback_api, qAAB, chon URL khong logo`.

---

### Task 4: `lvc-core` — proxy + thời lượng/timeout theo mode

**Files:** Create `crates/lvc-core/src/proxy.rs`; Modify `crates/lvc-core/src/mode.rs`, `limits.rs`, `lib.rs`.

**Interfaces — Produces:**
```rust
// proxy.rs — src/utils/proxies.py:31-80
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxySpec { pub scheme: String /* http|https|socks5 */, pub host: String, pub port: u16, pub username: Option<String>, pub password: Option<String> }
impl ProxySpec { pub fn server(&self) -> String /* "scheme://host:port" cho --proxy-server */ ; pub fn has_auth(&self) -> bool }
pub fn parse_proxy(s: &str) -> Result<Option<ProxySpec>, CoreError>; // "" | "direct" → None; "ip:port" | "ip:port:user:pass" | "scheme://user:pass@host:port"
pub fn effective_proxy(account_proxy: &str, global: &str) -> Result<Option<ProxySpec>, CoreError>; // own > global; "direct" chặn global

// mode.rs — modes.py:206-231, 275-285
impl Mode {
    pub fn allowed_durations(self, model: Model) -> &'static [u16]; // fast: [10,15] (+30 khi Seedance25) ; pro-like: [10,15]
}
pub fn route_30s(mode: Mode, model: Model, duration: u16) -> (Mode, Model); // 30 → (Fast, Seedance25) bất kể input
pub fn video_timeout(mode: Mode, duration: u16, has_refs: bool, limits: &Limits) -> u64; // base 300; pro-like max(1800); dur≥30 max(1500); refs max(900)
// limits.rs thêm: video_timeout_s: 300, pro_video_timeout_s: 1800, long_video_timeout_s: 1500, reference_video_timeout_s: 900, pro_max_timeout_s: 3600, pro_activity_window_s: 600, poll_interval_s: 5, reference_upload_timeout_s: 90, agent_question_grace_s: 60, agent_idle_fail_s: 120, confirm_max_replies: 3, link_raw_grace_s: 45, no_conversation_timeout_s: 30
```
- [ ] **Step 1: RED** — proxy: 4 định dạng + `direct` + lỗi (`"abc"` → Err, port không phải số → Err); `effective_proxy("", "1.2.3.4:8080")` → global, `("direct","…")` → None. Mode: `Mode::Fast.allowed_durations(Model::Seedance25) == [10,15,30]`, `Seedance20 → [10,15]`, `ProMax → [10,15]`; `route_30s(Agent, Seedance20, 30) == (Fast, Seedance25)`, `(Fast, Seedance20, 10)` giữ nguyên; `video_timeout(Fast,10,false) == 300`, `(Pro,10,false) == 1800`, `(Fast,30,false) == 1500`, `(Fast,10,true) == 900`, `(Pro,10,true) == 1800`.
- [ ] **Step 2: Cập nhật test cũ** của `allowed_durations` (P0.5 ký `allowed_durations(self)` không có model) — đổi chữ ký, sửa các chỗ gọi (`scene.rs` normalize dùng nó? kiểm `grep allowed_durations`).
- [ ] **Step 3: GREEN + gate + commit** — `feat(core): proxy spec, allowed_durations theo model, route_30s, video_timeout`.

---

### Task 5: `lvc-browser::ops` — thao tác trang bằng CDP thô

**Files:** Create `crates/lvc-browser/src/ops.rs`, `crates/lvc-browser/tests/fixtures/composer.html`, `crates/lvc-browser/tests/ops_smoke.rs`; Modify `lib.rs`, `chromium.rs` (expose `ChromiumSession::page()`), `Cargo.toml` (`serde_json`, `tokio` features `time`).

**Interfaces — Produces** (mọi hàm nhận `&Page` của chromiumoxide; lỗi → `BrowserError`):
```rust
pub async fn eval_json<T: DeserializeOwned>(page: &Page, js: &str) -> Result<T, BrowserError>;        // Runtime.evaluate, awaitPromise, returnByValue
pub async fn eval_with_arg<T, A: Serialize>(page: &Page, js_fn: &str, arg: &A) -> Result<T, BrowserError>; // gọi "(js_fn)(arg)" với arg JSON-serialized
pub async fn click_js(page: &Page, finder_js: &str) -> Result<bool, BrowserError>;
//  finder_js: biểu thức JS trả về Element|null. Thuật toán: eval `(()=>{const e=(FINDER); if(!e) return null; e.scrollIntoView({block:'center'}); const r=e.getBoundingClientRect(); return {x:r.x+r.width/2,y:r.y+r.height/2,w:r.width,h:r.height};})()`
//  → null → Ok(false); w==0||h==0 → Ok(false); else Input.dispatchMouseEvent mouseMoved → mousePressed(left,clickCount 1) → mouseReleased; Ok(true)
pub async fn click_js_at_offset(page: &Page, finder_js: &str, dx_from_right: f64, dy: f64) -> Result<bool, BrowserError>; // cho nút ✕ thẻ đính kèm (video_worker_ui.py:794-833)
pub async fn press(page: &Page, key: &str) -> Result<(), BrowserError>;      // "Enter" | "Delete" | "End" | "Escape" | "Control+A" (modifiers=2 cho Ctrl) qua Input.dispatchKeyEvent keyDown+keyUp (dùng chromiumoxide keys.rs cho code/windowsVirtualKeyCode)
pub async fn insert_text(page: &Page, text: &str) -> Result<(), BrowserError>; // Input.insertText — một phát (TYPE_PASTE_THRESHOLD=0)
pub async fn set_file_input(page: &Page, css: &str, paths: &[PathBuf]) -> Result<(), BrowserError>; // DOM.setFileInputFiles với objectId của page.find_element(css); path tuyệt đối
pub async fn delete_cookies(page: &Page, domain: &str, names: &[&str]) -> Result<(), BrowserError>; // Network.deleteCookies (domain ".dola.com" và "www.dola.com")
pub async fn cookie_header(page: &Page, url: &str) -> Result<String, BrowserError>; // Network.getCookies(urls=[url]) → "a=b; c=d"
pub async fn user_agent(page: &Page) -> Result<String, BrowserError>;
pub async fn url(page: &Page) -> Result<String, BrowserError>;
pub async fn wait_until(page: &Page, pred_js: &str, timeout: Duration, every: Duration) -> Result<bool, BrowserError>; // eval bool mỗi `every`; "Execution context was destroyed"/"Cannot find context" → coi false và tiếp (video_worker_ui.py:1036-1052)
pub async fn reload(page: &Page, settle: Duration) -> Result<(), BrowserError>;    // Page.reload + chờ Page.loadEventFired hoặc timeout 60 s, rồi sleep settle
pub struct ResponseCounter { /* Arc<Mutex<Vec<(String url, u16 status)>>> */ }
pub async fn count_responses(page: &Page) -> Result<ResponseCounter, BrowserError>; // Network.enable + event_listener::<EventResponseReceived>; counter.matches("/upload/v1/", 2xx) -> usize
pub async fn frame_urls(page: &Page) -> Result<Vec<String>, BrowserError>;  // Page.getFrameTree đệ quy — tìm "bdcaptcha.html"
```
- [ ] **Step 1: Fixture** `tests/fixtures/composer.html`: một `<textarea>`, nút `<button>高速</button>` mở `<div role="menu"><div role="menuitem">プロ</div></div>`, nút `動画を作成` khi bấm thêm `<button>10s</button><button>比率 9:16</button>`, `<input type="file" multiple>` ghi số file vào `#files`, `<iframe src="about:blank?bdcaptcha.html">` ẩn hiện theo nút, `<div id="log">` ghi mọi sự kiện (mousedown x,y; keydown key; input text) để test đọc lại.
- [ ] **Step 2: RED smoke** `tests/ops_smoke.rs` (tất cả `#[ignore]`, mở `file://<abs>/composer.html` bằng `ChromiumDriver` headless, `LVC_CHROME` tuỳ chọn): `click_js_bam_dung_tam_nut` (log có mousedown trong rect), `press_ctrl_a_delete_xoa_textarea`, `insert_text_mot_phat` (giá trị textarea == chuỗi có `\n`, không gửi Enter), `set_file_input_2_file` (`#files == "2"`), `wait_until_het_han_tra_false`, `count_responses_dem_2xx` (fixture fetch 1 URL file:// → không có status HTTP; dùng data: URL hoặc bỏ test này nếu file:// không phát `responseReceived` — implementer đo và ghi report), `frame_urls_thay_bdcaptcha`.
- [ ] **Step 3: Implement** — `click_js` KHÔNG dùng `element.click()` JS (không phải chuột thật); `press("Control+A")` = keyDown Control (modifiers 0) → keyDown 'a' (modifiers 2, text rỗng) → keyUp cả hai. Doc comment: `Runtime.evaluate` = MAIN world.
- [ ] **Step 4: GREEN** — `cargo test -p lvc-browser --test ops_smoke -- --ignored` (controller chạy trên máy có Chrome); gate không cần browser vẫn xanh.
- [ ] **Step 5: Commit** — `feat(browser): ops — click/keyboard/insertText/file/cookie/wait tren CDP tho`.

---

### Task 6: `dola::labels` + `dola::session` — ngôn ngữ, chờ ổn định, xác nhận đăng nhập

**Files:** Create `crates/lvc-browser/src/dola/mod.rs`, `dola/labels.rs`, `dola/session.rs`; Modify `lib.rs`.

**Interfaces — Produces:**
```rust
// labels.rs — CHÉP NGUYÊN VĂN (JS trong r#""#, regex qua regex crate)
pub const DOLA_CHAT_URL: &str = "https://www.dola.com/chat";
pub const LANGUAGE_COOKIES: [&str; 2] = ["user_language_code", "i18next"];
pub const DOLA_LANG_JS: &str;        // video_worker_ui.py:1019-1025 (NOTES §3.3)
pub const DOLA_SET_JA_JS: &str;      // :1026-1033
pub const MODE_BTN_JS: &str;         // :1055
pub const SESSION_JS: &str;          // = lvc_core::SESSION_PROBE_JS (re-export, không chép lại)
pub const RECENT_IDS_JS: &str;       // :635-664 (NOTES §3.4) — hàm async nhận {msToken, fp}
pub const COUNT_IMG_JS: &str;        // :710
pub const CLEAR_ABILITY_TAG_JS: &str;// :2330-2346 (NOTES §3.9)
pub const PASTE_MAIN_WORLD_SCRIPT: &str; // :730-753 (T8)
pub const DURATION_HOOK_JS: &str;    // :2215-2242 với "__DUR__" (T7)
pub const CAPTCHA_FRAME_KEY: &str = "bdcaptcha.html";
pub static MODE_BTN_RE: LazyLock<Regex>;   // (高速|Nhanh|Fast|プロ|Pro) i
pub static VIDEO_BTN_RE, DUR_BTN_RE, RATIO_BTN_RE, MODEL_BTN_RE, SKILL_BTN_RE, VIDEO_SKILL_OPT_RE, ABILITY_TAG_RE; // NOTES §3.6-3.9
pub const MODEL_OPTIONS: [(Model, &[&str]); 3]; // modes.py:45-50
pub fn finder_button_text(re_js: &str) -> String; // sinh JS: [...document.querySelectorAll('button')].find(b => /RE/.test((b.innerText||'').trim()))
pub fn finder_menu_item(re_js: &str) -> String;   // '[role="menuitem"], [role="option"], div[class*="menu-item"], div[class*="option"], button'
pub fn finder_text_exact(text: &str) -> String;   // phần tử lá có textContent.trim() == text
pub fn finder_composer() -> &'static str;         // 'textarea, [contenteditable="true"]' .first
pub const OPEN_MENU_JS: &str = "!!document.querySelector('[role=\"menu\"]')";
pub const COOKIE_OK_FINDER: &str;                  // button có text "OK" | "Đồng ý" và visible

// session.rs
pub async fn ensure_japanese(page: &Page) -> Result<(), BrowserError>;  // DOLA_LANG_JS; ls/ck/ul không bắt đầu "ja" và !jp → DOLA_SET_JA_JS + reload(4 s) ; R4
pub async fn wait_settled(page: &Page, timeout: Duration) -> Result<bool, BrowserError>; // :1058-1077 — MODE_BTN_JS true và url không đổi 2 tick liên tiếp
pub async fn assert_logged_in(page: &Page, account: &str) -> Result<(), DolaError>;      // :1170-1209 — ensure_japanese → wait_settled(25 s) → cookie sessionid → SESSION_JS → classify_session; logged_out có sessionid → reload 1 lần; vẫn → DolaError::LoggedOut{revoked:false}
pub async fn recent_conv_ids(page: &Page) -> Result<Vec<String>, BrowserError>;        // eval_with_arg(RECENT_IDS_JS, {msToken, fp}) timeout 10 s; lỗi → vec![]
pub async fn raise_if_logged_out(page: &Page, account: &str, ctx: &str) -> Result<(), DolaError>; // :1212-1228 — url chứa "from_logout=1" → revoked; SESSION_JS logged_out → revoked=true
```
- [ ] **Step 1: RED** — unit test (không browser): regex `MODE_BTN_RE` khớp `"高速\nNew"`, `"プロ"`, không khớp `"Model"`; `DUR_BTN_RE` khớp `"10s"`, `" 15 秒 (β)"`, không khớp `"10sec"`; `RATIO_BTN_RE` khớp `"比率\n16:9"`, `"比率"`; `finder_button_text` sinh JS hợp lệ (chứa regex đã escape `/`); `DOLA_LANG_JS`/`RECENT_IDS_JS` **so byte** với chuỗi lấy từ file Python (test đọc `A:\ToolLVC\lvcseedance\src\video\video_worker_ui.py` nếu tồn tại, cắt theo marker; nếu không tồn tại → `#[ignore]`-style skip có in lý do) — như P0.5 đã làm với SESSION_PROBE_JS.
- [ ] **Step 2: Smoke `#[ignore]`** trên fixture: `wait_settled_tra_true_khi_co_nut_mode`, `assert_logged_in_fixture_logged_in` (fixture có textarea, không marker, cookie giả `sessionid` đặt bằng `Network.setCookie` cho `file://`? — không được; đổi: test `classify` qua `SESSION_JS` trên fixture và test đường `has_sessionid=false → LoggedOut` với `expect_err`).
- [ ] **Step 3: Implement.** `DolaError` tạm định nghĩa ở `dola/error.rs` (T9 hoàn thiện): `LoggedOut{account, why, revoked}`, `ModeUnavailable(String)`, `Browser(#[from] BrowserError)`.
- [ ] **Step 4: GREEN + gate + commit** — `feat(browser): dola labels + session (ngon ngu, wait_settled, assert_logged_in)`.

---

### Task 7: `dola::compose` — chọn mode, mở composer, model/tỉ lệ/thời lượng, gõ & gửi

**Files:** Create `crates/lvc-browser/src/dola/compose.rs`; fixture mở rộng; smoke `tests/compose_smoke.rs`.

**Interfaces — Produces:**
```rust
pub async fn close_open_menus(page: &Page) -> Result<(), BrowserError>;                   // :1845-1851 — Escape tối đa 3 lần khi OPEN_MENU_JS
pub async fn dismiss_cookie_banner(page: &Page) -> Result<(), BrowserError>;             // :1987-1993
pub async fn select_mode(page: &Page, target: Mode) -> Result<(), DolaError>;            // :1970-2050 — target = Pro nếu ProMax|Agent; đọc text nút; không khớp → click nút → 700 ms → click menuitem (fast: (高速|Nhanh|Fast) / pro: (プロ|Pro)) → 1500 ms → close menus → đọc lại; 2 vòng; hết → DolaError::ModeSwitch{want, shows} (không mã, như Python); không tìm thấy nút → clear_ability_tag + retry 5 s → reload 60 s +5 s → retry 15 s → DolaError::ModeUnavailable(msg có url + buttons seen)
pub async fn open_video_composer_fast(page: &Page) -> Result<(), DolaError>;             // :2053-2089 — đã mở nếu DUR_BTN_RE|RATIO_BTN_RE|ABILITY_TAG_RE; 3 lần click VIDEO_BTN_RE (lần 3 evaluate e.click()); 1500 ms; hết → ModeUnavailable("không mở được composer Nhanh…")
pub async fn clear_composer_attachments(page: &Page) -> Result<usize, BrowserError>;     // :794-833 — ATTACH_CARD_SEL, click_js_at_offset (4,4),(6,6),(2,2),(8,8)
pub async fn select_model(page: &Page, model: Model) -> Result<(), BrowserError>;        // :2136-2169 — nuốt lỗi (log warn)
pub async fn select_ratio(page: &Page, ratio: Ratio) -> Result<(), DolaError>;           // :2176-2207 — 3 lần → RatioUnavailable
pub async fn select_fast_duration(page: &Page, duration: u16) -> Result<(), DolaError>;  // :1879-1922 — 10×500 ms chờ nhãn khớp; click + menuitem `^\s*{d}s(\b|\s|$)`; KHÔNG có nút và d==10 → Ok (NOTES §3.7 caveat); mismatch → DurationUnavailable
pub async fn install_duration_hook(page: &Page, duration: u16) -> Result<(), BrowserError>; // evaluate DURATION_HOOK_JS.replace("__DUR__", d) (MAIN world — không cần <script>); rồi kiểm `data-lvc-hook` == "installed:N"|"patched:N"
pub async fn verify_hook_patched(page: &Page, duration: u16) -> Result<bool, BrowserError>; // 20×250 ms chờ "patched:N"
pub async fn type_and_send(page: &Page, text: &str, append: bool) -> Result<(), BrowserError>; // :2117-2133 — close menus → click composer → (End | Control+A, Delete) → insert_text → 600 ms → Enter
pub async fn clear_ability_tag(page: &Page) -> Result<String, BrowserError>;             // :2349-2367 — eval CLEAR_ABILITY_TAG_JS, trả "cleared"|"cleared-svg"|"none"|"no-close-button"
```
- [ ] **Step 1: Fixture** thêm: menu mode (bấm nút `高速` mở menu với `プロ`; bấm `プロ` đổi text nút), nút `モデル 2.0高速` mở menu `Dreamina Seedance 2.5`, nút `比率 9:16` mở `[role=menuitem]` `16:9`, nút `10s` mở `15s`, thẻ đính kèm `[class*="image-container-x"] [class*="container-y"]` có nút ✕ ở góc, `data-lvc-hook` do fixture set khi `window.fetch('/chat/completion', {body: '{"chat_ability":{"ability_param":"{\\"duration\\":10}"}}'})` được gọi (fixture stub `fetch` gốc trả Response rỗng).
- [ ] **Step 2: RED smoke** `#[ignore]`: `select_mode_fast_khi_dang_pro`, `select_mode_khong_co_nut_bao_mode_unavailable` (fixture `?nomode=1`), `open_composer_fast_hien_nut_10s`, `select_ratio_16_9`, `select_fast_duration_15`, `select_fast_duration_10_khong_co_nut_van_ok`, `hook_patch_duration_15` (gọi fetch giả → attribute `patched:15`), `type_and_send_ghi_log_enter`.
- [ ] **Step 3: Implement** — mọi sleep dùng hằng có tên (`MENU_SETTLE = 700 ms`…), không magic number rải rác.
- [ ] **Step 4: GREEN + gate + commit** — `feat(browser): dola compose — mode/composer/model/ratio/duration/hook/type_and_send`.

---

### Task 8: `dola::refs` — ảnh tham chiếu

**Files:** Create `crates/lvc-browser/src/dola/refs.rs`; smoke mở rộng.

**Interfaces — Produces:**
```rust
pub enum AttachWay { Input, Paste }
pub async fn attach_reference_images(page: &Page, paths: &[PathBuf], timeout: Duration) -> Result<AttachWay, DolaError>;
// :836-892 — counter = count_responses; thumbs0 = eval COUNT_IMG_JS; có input[type=file] → set_file_input (tất cả một lần, way=Input)
// else mỗi file: paste_image_once (way=Paste) + 700 ms.
// Chờ mỗi 250 ms đến khi prepare(/alice/resource/prepare_upload 2xx) ≥ 1 && tos(/upload/v1/ 2xx) ≥ n && thumbs ≥ thumbs0 + n; +800 ms.
// Hết timeout (max(10 s, 90 s)) → raise_if_logged_out → DolaError::RefUpload{attached, expected, way, prepare, tos, thumbs}
pub async fn paste_image_once(page: &Page, path: &Path) -> Result<(), DolaError>;
// evaluate PASTE_MAIN_WORLD_SCRIPT một lần (MAIN world; idempotent qua data-dolaPasteReady) rồi window.postMessage({type:'dola-paste', b64, name, mime}); đọc data-dolaPasteResult 20×100 ms: "ok" | "nobox" | "error:…" → RefUpload
pub fn mime_by_suffix(path: &Path) -> &'static str; // :709
```
- [ ] **Step 1: RED** — unit `mime_by_suffix` (jpg/jpeg/png/webp/gif, khác → application/octet-stream); smoke `#[ignore]`: fixture có `input[type=file]` + stub `fetch` tự gọi `/alice/resource/prepare_upload` và `/upload/v1/x` (qua `data:` hoặc `blob:`? — nếu `Network.responseReceived` không bắn cho file://, implementer chạy fixture qua một server HTTP cục bộ tối giản trong test (`tokio::net::TcpListener` trả 200) và ghi report); `attach_2_anh_qua_input`; `paste_khi_khong_co_input` (fixture `?noinput=1` có contenteditable và listener paste đếm `clipboardData.files.length` — kiểm read-once: đọc 2 lần → lần 2 = 0).
- [ ] **Step 2: Implement; GREEN; commit** — `feat(browser): dola refs — anh tham chieu qua input/paste, dem upload`.

---

### Task 9: `dola::submit` + `dola::error` — gửi prompt fast, lấy conversation id

**Files:** Create `crates/lvc-browser/src/dola/submit.rs`, hoàn thiện `dola/error.rs`.

**Interfaces — Produces:**
```rust
#[derive(Debug, Clone)]
pub struct SubmitRequest { pub account: String, pub prompt: String, pub mode: Mode, pub ratio: Option<Ratio>, pub duration: u16, pub model: Model, pub reference_images: Vec<PathBuf> }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct ConversationId(pub String);

#[derive(Debug, thiserror::Error)]
pub enum DolaError {
    #[error("LOGGED_OUT: {account}: {why}")] LoggedOut { account: String, why: String, revoked: bool },
    #[error("MODE_UNAVAILABLE: {0}")] ModeUnavailable(String),
    #[error("Cannot switch Dola mode to {want}; button shows {shows:?}")] ModeSwitch { want: String, shows: String },
    #[error("RATIO_UNAVAILABLE: {0}")] RatioUnavailable(String),
    #[error("DURATION_UNAVAILABLE: {0}")] DurationUnavailable(String),
    #[error("REF_UPLOAD: {0}")] RefUpload(String),
    #[error("NO_CONVERSATION: {0}")] NoConversation(String),
    #[error("RISK_CONTROL: {0}")] RiskControl(String),           // captcha — không mã, cooldown 1800 s
    #[error("PROMPT_VIOLATION: {0}")] PromptViolation(String),
    #[error("AGENT_QUESTION: {0}")] AgentQuestion(String),
    #[error("AGENT_NO_VIDEO: {0}")] AgentNoVideo(String),
    #[error("LIMIT: {0}")] AccountLimited(String),               // không mã → pool đánh dấu hạn mức ngày
    #[error("CREDIT: {0}")] Credit(String),                      // không mã → pool xoay
    #[error("TIMEOUT: {0}")] Timeout(String),                    // không mã
    #[error("BROWSER_CLOSED: {0}")] BrowserClosed(String),
    #[error(transparent)] Browser(#[from] BrowserError),
    #[error("HTTP: {0}")] Http(String),
}
impl DolaError {
    pub fn failure_code(&self) -> Option<FailureCode>;  // LoggedOut→LoggedOut, ModeUnavailable→ModeUnavailable, DurationUnavailable→DurationUnavailable, RefUpload→RefUpload, NoConversation→NoConversation, PromptViolation→PromptViolation, AgentQuestion→AgentQuestion; còn lại None
    pub fn is_dead_chat(&self) -> bool;                 // PromptViolation | AgentQuestion | AgentNoVideo
    pub fn rotates_account(&self) -> bool;              // LoggedOut | RiskControl | AccountLimited | Credit | Browser(ProfileBusy)
}

pub struct SubmitCtx<'a> { pub page: &'a Page, pub limits: &'a Limits }
pub async fn submit(ctx: &SubmitCtx<'_>, req: &SubmitRequest) -> Result<ConversationId, DolaError>;
```
Thuật toán `submit` (fast; pro-like ở T12) — `video_worker_ui.py:2384-2530, 2533-2616`:
1. Kiểm tra thuần: `req.duration ∈ allowed_durations(mode, model)` else `DurationUnavailable`.
2. `goto DOLA_CHAT_URL` (nếu url hiện tại đã là `/chat` → chờ 1 s; else goto + 5 s).
3. `assert_logged_in`.
4. `known = recent_conv_ids()` (HashSet).
5. `dismiss_cookie_banner`; `select_mode(target)`.
6. Fast: `open_video_composer_fast` → `clear_composer_attachments` → nếu refs: `attach_reference_images` → `select_model` → `close_open_menus` → nếu `ratio`: `select_ratio` → `select_fast_duration(d)`; lỗi DurationUnavailable và d ∈ {15,30} → `select_fast_duration(10)` + `install_duration_hook(d)` → `type_and_send(prompt, false)` → nếu hook: `verify_hook_patched` false → `DurationUnavailable("hook không patch")`.
7. `after_submit`: vòng 30 lần × 1 s (R3): mỗi tick (a) `frame_urls` chứa `bdcaptcha.html` → `RiskControl`; (b) `pick_new_conv_id(url_tail, known)`: `New(id)` → trả; `Known(id)` và tick % 3 == 0 → `recent_conv_ids` lấy id đầu ∉ known → `goto /chat/<id>` + 2 s → trả. Hết 30 s → screenshot `logs/no_conv_<account>_<ts>.png` → `raise_if_logged_out` → `NoConversation(url=…, ảnh=…)`.

- [ ] **Step 1: RED** — unit: `failure_code()` bảng 17 dòng; `is_dead_chat`; `rotates_account`; `Display` có tiền tố đúng. Smoke `#[ignore]` trên fixture "trang chat giả" (`chat.html`: composer + nút mode + `動画を作成` + sau Enter đổi `location.hash`? — không: `pick_new_conv_id` đọc **path**; fixture dùng `history.pushState({}, '', '/chat/123456789')` sau 1 s Enter): `submit_fast_tra_conv_id`, `submit_fast_captcha_bao_risk_control` (fixture `?captcha=1` chèn iframe sau Enter), `submit_fast_khong_conv_bao_no_conversation` (`?noconv=1`, 30 s — dùng `Limits.no_conversation_timeout_s = 3` cho test).
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(browser): dola submit — luong fast den conversation_id, DolaError`.

---

### Task 10: `dola::http` — poll HTTP và tải file bằng reqwest

**Files:** Create `crates/lvc-browser/src/dola/http.rs`; `Cargo.toml` thêm `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json", "stream", "socks"] }`, `uuid = { version = "1", features = ["v4"] }`, `tokio` feature `io-util`; workspace deps.

**Interfaces — Produces:**
```rust
pub struct DolaHttp { client: reqwest::Client, ua: String, proxy: Option<ProxySpec> }
impl DolaHttp {
    pub fn new(user_agent: String, proxy: Option<ProxySpec>) -> Result<Self, DolaError>; // reqwest::Proxy::all(spec.server()) + basic_auth; cookie_store(false); timeout 40 s
    pub async fn pull_chain(&self, cookie_header: &str, conv: &ConversationId) -> Result<ChainPoll, DolaError>;
    // hub_browser.py:853-873: POST https://www.dola.com/im/chain/single?<POLL_PARAMS + web_tab_id=uuid4>
    // headers: User-Agent=ua, Accept */*, Accept-Language ja-JP,ja;q=0.9, Origin https://www.dola.com, agw-js-conv str,
    //          Content-Type application/json; encoding=utf-8, Referer https://www.dola.com/chat/<id>, Cookie=<cookie_header>
    // body = đúng JSON POLL_JS (cmd 3100, anchor_index 9007199254740991, conversation_type 3, direction 1, limit 20, sequence_id uuid4, channel 2, version "1")
    // HTTP≠200 → ChainPoll{ok:false,status}; chain_http_status(body) Some(s) → ok:false,status:s; else parse_chain
    pub async fn fetch_raw_video(&self, fallback_api: &str, timeout: Duration) -> Result<Option<String>, DolaError>; // unwatermark.py:225-240: GET replace_query(api, RAW_PARAMS), header accept application/json,text/plain,*/*, KHÔNG cookie → pick_raw_video
    pub async fn download(&self, url: &str, out: &Path) -> Result<u64, DolaError>; // video_worker.py:170-187: GET thuần, timeout 300 s, stream 64 KiB → file; trả số byte; ghi .part rồi rename
}
pub const POLL_PARAMS: &[(&str, &str)]; // hub_browser.py:748-750 (version_code 20800, language ja, aid 495671, pc_version 3.32.61, region JP, …) — chép nguyên văn
pub fn output_path(out_dir: &Path, account: &str, now: chrono-free) -> PathBuf; // "<account>_<YYYYmmdd_HHMMSS>.mp4" — dùng `time` crate hoặc tự format từ SystemTime (không thêm chrono)
```
- [ ] **Step 1: RED** — test bằng server HTTP cục bộ trong test (`tokio::net::TcpListener` + parser tối giản, hoặc `wiremock = "0.6"` dev-dep — chọn wiremock): `pull_chain_gui_dung_header_va_body` (mock kiểm `Cookie`, `agw-js-conv`, body chứa `"cmd":3100` và conv id; trả JSON fixture T2 → videos 1); `pull_chain_status_712012001_tra_401`; `pull_chain_http_500_ok_false`; `download_stream_ghi_dung_byte` (mock trả 200 KiB); `download_404_loi`; `fetch_raw_video_chon_bitrate_cao`; `output_path_dinh_dang`.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(browser): dola http — pull_chain/fetch_raw_video/download bang reqwest`.

---

### Task 11: `dola::poll` — vòng poll và quyết định tải

**Files:** Create `crates/lvc-browser/src/dola/poll.rs`.

**Interfaces — Produces:**
```rust
pub struct VideoArtifact { pub video_url: String, pub local_path: PathBuf, pub conversation_id: ConversationId, pub account: String, pub watermarked: bool, pub bytes: u64, pub note: &'static str /* " (raw, no logo)" | " (unwatermarked priority)" | " (link in text)" */ }
pub struct PollOpts { pub timeout: Duration, pub mode: Mode, pub duration: u16, pub ignore_videos: HashSet<String>, pub ignore_links: HashSet<String>, pub accept_links: bool }
pub trait PollHooks: Send + Sync { fn on_poll(&self, elapsed: Duration) {} fn on_stage(&self, stage: &str) {} fn on_agent_reply(&self, text: &str) {} }
pub async fn poll_and_download(page: &Page, http: &DolaHttp, account: &str, conv: &ConversationId, out_dir: &Path, opts: &PollOpts, limits: &Limits, hooks: &dyn PollHooks) -> Result<VideoArtifact, DolaError>;
pub(crate) fn handle_poll(poll: &ChainPoll, state: &mut PollState, opts: &PollOpts, limits: &Limits, now: Instant) -> Result<PollDecision, DolaError>; // THUẦN — test không I/O
pub(crate) enum PollDecision { Continue, Download { url: String, note: &'static str }, ConfirmReply(String), DomScanNeeded }
pub(crate) struct PollState { pub confirmed: bool, pub last_latest: Vec<String>, pub last_activity: Instant, pub agent_question_since: Option<Instant>, pub confirm_replies: HashMap<String, u32>, pub link_first_seen: Option<Instant> }
```
Vòng (`video_worker_ui.py:1475-1525`): cookie_header lấy lại mỗi vòng từ CDP (`Network.getCookies` — cookie có thể xoay); sleep `poll_interval_s`; `pull_chain`; `!ok && status ∈ {401,403}` → `LoggedOut`; lỗi HTTP khác → tiếp; `hooks.on_poll` ≤ 1/30 s; `handle_poll` theo thứ tự `NOTES §4.4` (1 activity → 2 limit/refusal/credit trên `latest` → 4 DOM scan `<main>` (eval `((document.querySelector('main')||document.body).innerText||'').slice(-3000)`) → 5 lọc ignore → 6 policy_rejection → 7 needs_confirmation (chỉ pro-like, tối đa `confirm_max_replies`) → 8 agent_question ≥ grace → 9 idle ≥ agent_idle_fail và !is_generating → 10 videos → 11 links (grace 45 s) → Continue). Timeout: dừng khi `elapsed ≥ timeout` **và không** `should_extend` (`modes.py:361-363`: elapsed < pro_max_timeout && silence < pro_activity_window). Tải: videos → `fetch_raw_video` cho ≤3 `find_fallback_apis(video_models[0])` → else `extract_unwatermarked_url` → `download`; links → `_raw_for_link` (khớp `video_ids` trong link với video_models) → else sau grace tải `links[0]`.

- [ ] **Step 1: RED** — `handle_poll` thuần: 12 test bảng quyết định (mỗi nhánh một test, dùng `Instant` giả bằng cách truyền `now`); smoke `#[ignore]` với wiremock: `poll_tra_video_sau_2_vong` (mock lần 1 rỗng, lần 2 có 2074 + fallback_api → raw), `poll_401_bao_logged_out`, `poll_timeout_bao_timeout` (timeout 1 s, interval 0).
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(browser): dola poll — vong poll, quyet dinh tai, tai raw/unwatermarked`.

---

### Task 12: `dola::skill` — pro / pro_max / agent

**Files:** Create `crates/lvc-browser/src/dola/skill.rs`; Modify `submit.rs` (rẽ nhánh theo mode), `poll.rs` (gửi trả lời xác nhận).

**Interfaces — Produces:**
```rust
pub async fn open_video_composer_pro(page: &Page) -> Result<(), DolaError>;   // :2091-2103 — SKILL_BTN_RE (スキル|Kỹ năng|Skills?) click 8 s → 1200 ms → [role="option"] VIDEO_SKILL_OPT_RE (動画の生成|Tạo video|Create Video) → 1200 ms → close menus; fallback VIDEO_BTN_RE
pub async fn submit_pro(ctx, req) -> Result<(), DolaError>;      // :2304-2311 — open pro → clear attachments → refs (paste) → text = lvc_core::pro_prompt(prompt, ratio, Some(duration), limits.pro_no_bgm) → type_and_send(" " + text, append=true)
pub async fn submit_pro_max(ctx, req) -> Result<(), DolaError>;  // :2314-2323 — không skill; text = pro_max_prompt(...) ; append=false
pub async fn submit_agent(ctx, req) -> Result<(), DolaError>;    // :2370-2381 — clear_ability_tag → clear attachments → refs (paste) → text = agent_prompt(prompt, ratio, Some(duration)) → type_and_send(text, false); lỗi select_mode được nuốt cho agent (:2480-2485)
pub async fn send_chat_message(page: &Page, text: &str) -> Result<(), BrowserError>; // :1242-1262 — type_and_send rồi nếu composer còn chữ → click nút gửi `button[aria-label*="Gửi" i], …, button:has(svg)` cuối
```
- [ ] **Step 1: RED** — smoke fixture: `open_composer_pro_qua_skill_menu`, `submit_agent_xoa_ability_tag_truoc`; unit: `submit` rẽ nhánh gọi đúng hàm (dùng fixture, kiểm log).
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(browser): dola skill — pro/pro_max/agent + tra loi xac nhan`.

---

### Task 13: Proxy có xác thực qua `Fetch.enable(handleAuthRequests)`

**Files:** Modify `crates/lvc-browser/src/driver.rs` (`LaunchOpts.proxy: Option<ProxySpec>` thay `proxy_server: Option<String>`), `chromium.rs`.

- `--proxy-server=<spec.server()>`; nếu `has_auth`: sau `new_page` gọi `Fetch.enable{handleAuthRequests:true, patterns:[{urlPattern:"*", requestStage:Request}]}` và spawn task nghe `EventAuthRequired` → `Fetch.continueWithAuth{response: ProvideCredentials, username, password}`; `EventRequestPaused` → `Fetch.continueRequest` ngay (không sửa). Chỉ bật Fetch **khi có auth** (không bật vô cớ — dấu vết).
- [ ] **Step 1: RED** — unit: `LaunchOpts` với `ProxySpec{auth}` → `config()` có arg `proxy-server=http://h:1`; smoke `#[ignore]`: proxy giả trong test (`tokio` TCP listener trả `407 Proxy-Authentication-Required` rồi `200` khi có header đúng) — implementer đo; nếu quá phức tạp, thay bằng smoke gọi `Fetch.enable` và kiểm không lỗi + ghi report.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(browser): proxy co user:pass qua Fetch.continueWithAuth`.

---

### Task 14: Mở rộng `trait BrowserDriver` + `FakeDriver`

**Files:** Modify `crates/lvc-browser/src/driver.rs`, `chromium.rs`; Create `crates/lvc-browser/src/fake.rs` (feature `fake` hoặc luôn có, `#[cfg(any(test, feature = "fake"))]`).

```rust
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    type Session: Send + Sync;
    async fn launch(&self, opts: &LaunchOpts) -> Result<Self::Session, BrowserError>;
    async fn add_cookies(&self, s: &Self::Session, cookies: &[Cookie]) -> Result<(), BrowserError>;
    async fn goto(&self, s: &Self::Session, url: &str) -> Result<(), BrowserError>;
    async fn probe_session(&self, s: &Self::Session) -> Result<SessionProbe, BrowserError>;
    async fn screenshot(&self, s: &Self::Session, path: &Path) -> Result<(), BrowserError>;
    async fn submit(&self, s: &Self::Session, req: &SubmitRequest, limits: &Limits) -> Result<ConversationId, DolaError>;
    async fn poll_and_download(&self, s: &Self::Session, account: &str, conv: &ConversationId, out_dir: &Path, opts: &PollOpts, limits: &Limits, hooks: &dyn PollHooks) -> Result<VideoArtifact, DolaError>;
    async fn user_agent(&self, s: &Self::Session) -> Result<String, BrowserError>;
    async fn close(&self, s: Self::Session) -> Result<(), BrowserError>;
}
pub struct FakeDriver { pub script: Mutex<VecDeque<FakeStep>> }   // FakeStep::{Submit(Result<ConversationId, DolaError>), Poll(Result<VideoArtifact, DolaError>), LaunchErr(BrowserError)}
```
`ChromiumDriver::submit` = `dola::submit`; `poll_and_download` tạo `DolaHttp::new(user_agent, opts.proxy)` mỗi lần (rẻ) và gọi `dola::poll`.
- [ ] **Step 1: RED** — `FakeDriver` chạy kịch bản: submit Ok → poll Ok; `LaunchErr(ProfileBusy)`; kịch bản hết → panic có thông điệp rõ. Test trait object: `Arc<dyn BrowserDriver<Session = FakeSession>>` compile.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(browser): trait submit/poll_and_download + FakeDriver`.

---

### Task 15: `lvc-engine::limiter` — `DynamicLimiter`

**Files:** Create `crates/lvc-engine/Cargo.toml` (deps: lvc-core, lvc-store, lvc-browser, tokio {sync,time,rt-multi-thread,macros}, dashmap = "6", tracing, thiserror, serde), `src/lib.rs`, `src/limiter.rs`; Modify workspace `Cargo.toml` (member + dep), `scripts/check.ps1` (nếu liệt kê crate).

Chép đúng spec §5.1 (`AtomicUsize` + CAS + `Notify` + hai `watch::Receiver`). Thêm `pub fn set_threads(&self, n: usize)` / `set_accounts(&self, n)` qua `watch::Sender` giữ trong `LimiterHandle`.
- [ ] **Step 1: RED** — `#[tokio::test]`: `effective_min_threads_accounts` (threads 5, accounts 2 → 2 permit, permit thứ 3 chờ); `tang_threads_danh_thuc_ngay` (chờ với timeout 200 ms rồi set_threads → có permit trong <100 ms); `giam_threads_khong_cat_job` (2 permit đang giữ, set 1 → không panic, permit mới chờ đến khi cả 2 thả); `drop_permit_notify` ; `effective_toi_thieu_1` (accounts 0 → 1).
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(engine): DynamicLimiter (spec §5.1)`.

---

### Task 16: `lvc-engine::pool` — `AccountPool`

**Files:** Create `crates/lvc-engine/src/pool.rs`; Modify `crates/lvc-store/src/accounts.rs` nếu thiếu hàm (thêm `list_schedulable`, `set_login_ok`, `set_cooldown_until`, `set_rate_limited_until(reason)`, `set_quota_blocked_until(reason)`, `touch_last_used`, `clear_expired_limits(now)`) với test store tương ứng.

```rust
pub struct AccountPool { db: Db, locks: DashMap<String, Arc<tokio::sync::Mutex<()>>>, recent_picks: Mutex<HashMap<String, Instant>>, limits: Limits, global_proxy: String }
pub struct Lease { pub name: String, pub proxy: Option<ProxySpec>, _guard: OwnedMutexGuard<()> }
impl AccountPool {
    pub async fn candidates_for(&self, mode: Mode, prefer: Option<&str>) -> Result<Vec<AccountRow>, EngineError>; // browser_pool.py:909-951: scheduling=1, dir tồn tại, login_ok≠0, cooldown/rate_limited/quota_blocked ≤ now, used_today(mode) < daily_limit(mode); prefer → chỉ nó (thiếu/bị chặn cứng → PreferredUnavailable); sắp theo spread_key = (used_today(fast), last_used_at, name); loại tên đã pick trong PICK_RESERVE=20 s (trừ khi chỉ còn nó)
    pub async fn try_lease(&self, name: &str, proxy: &str) -> Option<Lease>;           // try_lock; None nếu bận
    pub async fn lease_wait(&self, name: &str, proxy: &str, max: Duration) -> Result<Lease, EngineError>; // prefer: chờ đến BUSY_WAIT_MAX=600 s
    pub async fn claim(&self, name: &str, mode: Mode) -> Result<(), EngineError>;        // bump_usage + touch_last_used
    pub async fn mark_logged_out(&self, name: &str, reason: &str, revoked: bool);
    pub async fn mark_daily_limit(&self, name: &str, mode: Mode, reason: &str);          // used = limit, rate_limited_until = next Tokyo midnight
    pub async fn mark_cooldown(&self, name: &str, secs: u64);                            // RiskControl 1800 / ProfileBusy 300
    pub async fn mark_quota_blocked(&self, name: &str, reason: &str);
    pub fn all_limited_for(&self, candidates_empty_reason: …) …                          // dùng bởi generate
    pub fn next_limit_reset(now) -> i64;                                                 // Tokyo 00:00 kế tiếp (thuần — đặt ở lvc-core::scheduling nếu chưa có)
}
```
- [ ] **Step 1: RED** — dùng `Db::open_memory` + `AccountRepo`: `candidates_loai_dang_cooldown`, `candidates_loai_het_han_muc_mode`, `candidates_sap_theo_spread_key`, `pick_reserve_20s`, `prefer_thieu_bao_preferred_unavailable`, `try_lease_ban_tra_none`, `lease_wait_cho_den_khi_tha`, `mark_daily_limit_dat_used_va_reset_tokyo`, `mark_logged_out_login_ok_0`.
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(engine): AccountPool — chon/xoay/khoa/han muc tai khoan`.

---

### Task 17: `lvc-engine::generate` + `session_cache` — điều phối một video

**Files:** Create `crates/lvc-engine/src/generate.rs`, `src/session_cache.rs`.

```rust
pub struct EngineConfig { pub accounts_dir: PathBuf, pub downloads_dir: PathBuf, pub chrome_executable: Option<PathBuf>, pub headless: bool, pub global_proxy: String, pub warm_idle: Duration /* 900 s */, pub busy_wait_max: Duration /* 600 s */ }
pub struct Engine<D: BrowserDriver> { driver: Arc<D>, pool: Arc<AccountPool>, limiter: Arc<DynamicLimiter>, cache: SessionCache<D::Session>, db: Db, limits: Limits, cfg: EngineConfig }
pub struct GenerateOutcome { pub artifact: VideoArtifact, pub account: String, pub mode_used: Mode }
impl<D: BrowserDriver> Engine<D> {
    pub async fn generate_video(&self, task_id: &str, scene: &Scene, prefer: Option<&str>, hooks: &dyn PollHooks) -> Result<GenerateOutcome, EngineError>;
}
```
Luồng (`browser_pool.py:1090-1287` + `server._run_task` 596-719):
1. `let _slot = limiter.acquire().await` (FIFO đủ dùng: `Notify` + thứ tự đến; ghi chú không tuyệt đối FIFO — ruling).
2. Vòng theo `mode_fallback_chain(scene.mode, &limits)` (chỉ khi `mode_fallback` bật; mặc định bật như Python `MODE_FALLBACK=1`); `duration = fit_duration_to_mode(...)`.
3. Vòng tài khoản: `candidates_for(mode, prefer)`; rỗng → `AllAccountsLimited` (→ `FailureCode::Rate429` ở cuối chuỗi mode). Với mỗi ứng viên: `try_lease` (prefer → `lease_wait`); bận → next.
4. `session = cache.take_or_launch(name, LaunchOpts{profile_dir: accounts_dir/name (tuyệt đối), proxy, headless, chrome_executable})`; `ProfileBusy` → `mark_cooldown(300)` + next; lỗi launch khác → next.
5. `store.set_status(task, Processing, account)`; `submit` → `Err(e)`: `e.rotates_account()` → đánh dấu (LoggedOut→mark_logged_out; RiskControl→cooldown 1800; AccountLimited→mark_daily_limit; Credit→mark_quota_blocked) + đóng session + next; khác → trả lỗi (không xoay, giống Python generic).
6. `store.set_conversation(task, conv)`; `poll_and_download(...)`; `Ok` → `claim(name, mode)` + `cache.put(session)` + trả; `Err(Timeout)` → `claim` + trả lỗi; `Err(rotates)` → như bước 5; khác → trả lỗi.
7. Ánh xạ cuối: `EngineError::Dola(e)` → `e.failure_code()`; `AllAccountsLimited` → `Some(Rate429)`; `PreferredUnavailable` → `Some(AccountUnavailable)`; `ProfileBusy` hết ứng viên → `Some(ProfileBusy)`. `store.set_status(Failed, code, message)`.
`SessionCache`: `HashMap<String, (Session, Instant)>`; `take_or_launch`; `put`; `spawn_reaper(driver)` đóng session idle > `warm_idle` mỗi 30 s; `close_all`.
- [ ] **Step 1: RED** — với `FakeDriver` + `Db::open_memory` + 2 tài khoản: `thanh_cong_claim_usage`, `logged_out_xoay_sang_tai_khoan_2`, `ca_2_het_han_muc_bao_429`, `prompt_violation_khong_xoay`, `profile_busy_cooldown_300`, `timeout_van_claim`, `prefer_account_ban_cho_roi_chay`, `fallback_mode_agent_sang_pro_khi_het` , `cache_giu_session_va_reaper_dong_sau_idle` (warm_idle 100 ms).
- [ ] **Step 2: Implement; GREEN; gate; commit** — `feat(engine): generate_video — dieu phoi, xoay tai khoan, session cache`.

---

### Task 18: THÍ NGHIỆM LIVE — 1 video thật (CẦN NGƯỜI DÙNG ĐỒNG Ý, TỐN CREDIT)

**Files:** Create `crates/lvc-engine/examples/gen_one.rs`; Modify spec (ADR-R013).

`gen_one --account <tên> --prompt "…" [--mode fast] [--duration 10] [--ratio 9:16] [--model seedance-2.0] [--ref a.png]… [--headed] [--db path]`; `LVC_CHROME` chọn binary. Dùng `Engine` thật với `Db::open("lvc.db")` (import từ `pool_usage.db` **bản copy** nếu chưa có), `accounts_dir = accounts/` (bản copy profile), in JSON: conv id, thời gian từng giai đoạn, đường dẫn mp4, kích thước, `watermarked`.

- [ ] **Step 1:** Controller hỏi người dùng: tài khoản nào, tốn ~2 credit (2.0 10 s), headed. **Không chạy nếu chưa đồng ý.**
- [ ] **Step 2:** Chạy; kết quả `state` theo bảng:

| Kết quả | Kết luận |
|---|---|
| mp4 tải về, phát được (ffprobe/size > 100 KiB) | **P2 nghiệm thu.** ADR-R013 ghi số đo (thời gian submit/poll/download, credit trừ) |
| `NO_CONVERSATION` | Enter không được nhận — kiểm `insert_text`/`Enter` bằng screenshot; so DOM với fixture |
| `RISK_CONTROL` | captcha — chromiumoxide bị soi ở bước gửi; ghi ADR, cân nhắc giải slider (P2.5) hoặc đo lại với headed + chậm |
| poll 401/403 | reqwest bị từ chối dù cookie đúng → R2 fallback: `pull_chain` qua evaluate `POLL_JS` |
| video có nhưng tải lỗi | kiểm URL ký/hết hạn, proxy |

- [ ] **Step 3:** ADR-R013 vào spec; xoá screenshot PII; commit `docs: ADR-R013 ket qua P2 live`.

---

## Cố ý KHÔNG làm ở P2
- Hub/wtabs/lvc_relay; giải captcha slider; balance pre-flight; logo removal + H.264 (P4 `lvc-media`); auto_retry (P3 DAG executor); Facebook/Google login + tab tài khoản (P3); real-person/ảnh (P4); chuỗi credit-quote resend; `REVOKED_STREAK` circuit breaker (P3, khi có nhiều tài khoản chạy song song thật).

## Nghiệm thu P2
- [ ] Gate xanh không cần browser; smoke `#[ignore]` xanh trên fixture cục bộ (controller chạy).
- [ ] Task 18: 1 video thật tải về từ tài khoản thật, ADR-R013.
- [ ] `lvc-core` vẫn thuần (dep-check).
- [ ] Mọi lỗi Python có mã → cùng mã; lỗi không mã → `None` + tiền tố message giống Python.
