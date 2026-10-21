# LVC Seedance v2 (Rust + Tauri)

Bản viết lại của `lvcseedance` (Python) sang Rust + Tauri: tạo video hàng loạt trên Dola
(seedance) bằng một workflow kéo thả (Prompt → Video → Lưu), quản lý nhiều tài khoản/proxy,
chạy nền, theo dõi tiến độ theo thời gian thực.

**Không bao giờ đụng tới thư mục bản Python (`A:\ToolLVC\lvcseedance`)** — chỉ đọc, không sửa
(xem "Nhập dữ liệu từ bản Python" bên dưới).

## Trạng thái

| Giai đoạn | Nội dung | Trạng thái |
|---|---|---|
| P0 | Lõi thuần (`lvc-core`) + tầng lưu trữ SQLite (`lvc-store`) | Xong |
| P1 | Điều khiển trình duyệt (`lvc-browser`, chromiumoxide) | Xong |
| P2 | Luồng tạo video Dola thật (`lvc-engine`) | Xong |
| P3 | Ứng dụng Tauri 2 + UI (Workflow/Tác vụ/Cài đặt/Tài khoản) + kịch bản (P3c) | Xong |
| P3b | Auto-login Google/Facebook + TOTP | Kế tiếp |
| P4a | Node thành module độc lập (`lvc-node-api`, `crates/nodes/*`, registry) | Xong |
| P4b | Dola ảnh + video qua API app Android (`crates/providers/lvc-dola`) | Xong |
| P4c | Kho tài khoản AI (`lvc-vault`: Dola, Google, MuseAI, API key + hạn mức/xoay vòng) | Xong |
| P5 | Node Veo3 ảnh/video (`lvc-veo3`) | Xong |
| P6 | Node API chính thức (Google/Gemini, Seedance, Grok) + `script_gen`/`script_agent`, TTS | Xong |
| P7 | Kho template từ xa (`lvc-template-registry`) + Kho mẫu 9:16 | Xong |
| P8 | Tự cập nhật (`tauri-plugin-updater`, `release.yml`) | Xong |
| — | Proxy 3 chế độ + hồ sơ cookie Firefox portable | Xong |
| — | MuseAI ảnh-trước (khoá nhân vật) + mẫu reel món ăn 30s | Xong, đã nghiệm thu live |
| — | Chạy bền: lưu trạng thái node, khôi phục sau restart, chạy lại node lỗi, chạy song song N node | Xong |
| — | Đăng nhập + license LVC Shop (`lvc-license`, `LicenseGate`) | Code xong (Task 1–9); **chưa thử với server thật** |
| — | Ký bộ cài chính thức, e2e UI tự động | Kế tiếp |

Kế hoạch chi tiết từng giai đoạn: `docs/superpowers/plans/`, thiết kế: `docs/superpowers/specs/`.

Xem "Giới hạn đã biết" bên dưới.

## Cấu trúc

```
crates/
  lvc-core     lõi thuần: chế độ (Mode), cảnh (Scene), TXT, tiến độ, FailureCode, Graph. Không I/O.
  lvc-store    SQLite qua sqlx, migration, nhập dữ liệu Python cũ (legacy import).
  lvc-browser  điều khiển Chrome/Chromium qua chromiumoxide (CDP) + toàn bộ thao tác Dola.
  lvc-engine   lịch tài khoản (AccountPool), pipeline tạo video, DAG workflow, resume.
  lvc-app      ứng dụng Tauri 2 (Rust backend + lệnh IPC), nhúng app/dist làm giao diện.
               src/registry.rs = nơi DUY NHẤT đăng ký node (1 dòng/node).
  lvc-node-api hợp đồng node độc lập: NodeExecutor, Value, Inputs, Registry::prepare.
  providers/   client dịch vụ AI dùng chung (lvc-dola, …) — không biết khái niệm node.
  nodes/       MỖI loại node một crate (prompt, image_upload, dola_video, save, …).
app/           frontend Vite + React 19 + React Flow + zustand + zod (giao diện 4 màn).
```

`lvc-core` **không được** phụ thuộc `sqlx`/`tokio`/`reqwest`/`chromiumoxide`/`tauri` — cưỡng
chế bằng `scripts/check.ps1` (kiểm `cargo tree -p lvc-core`).

## Chuẩn bị máy

- Rust 1.95 qua `rust-toolchain.toml` (MSRV workspace: 1.85 — không dùng let-chains).
- Node 24 + pnpm 10 (`corepack enable` hoặc cài pnpm 10 trực tiếp).
- `cargo tauri` CLI 2 (`cargo install tauri-cli --version ^2`).
- WebView2 Runtime (đã có sẵn trên Windows 10/11 cập nhật; nếu thiếu, Tauri báo lỗi rõ lúc chạy).
- Trình duyệt Chromium: khuyến nghị dùng đúng bản Playwright Chromium (rev ≥ 1148) đã cài cho
  bản Python — xem "Trình duyệt" bên dưới, `lvc-browser` **không tự tải** Chromium.

## Chạy dev

```powershell
cd crates/lvc-app
cargo tauri dev
```

Lệnh này tự chạy `pnpm -C ../app dev` (Vite trên `:5173`) rồi mở cửa sổ Tauri trỏ vào đó
(`tauri.conf.json`: `beforeDevCommand`/`devUrl`, chạy từ `crates/`, gốc là cha của `lvc-app`).
Lần đầu sẽ biên dịch toàn bộ cây phụ thuộc Tauri 2 (~700+ crate: wry, tao, webview2-com,
chromiumoxide, sqlx-macros…), có thể mất vài phút; các lần sau chỉ biên dịch lại phần đổi.

### Chế độ giả lập, không tốn credit (`fake-driver`)

```powershell
cargo tauri dev --features fake-driver
```

Thay `ChromiumDriver` thật bằng `FakeDriver::endless_ok` — mọi phiên/nộp/poll đều thành công,
poll xong sau 3 giây, ghi ra một file mp4 giả (không phải video thật, không gọi Dola, không tốn
credit). Log ghi rõ dòng `WARN … CHE DO GIA LAP — FakeDriver, khong goi Dola` ngay sau khi khởi
động. Cần ít nhất một tài khoản có mặt trong DB + có thư mục `accounts/<tên>/` (import cookie
bất kỳ qua tab Tài khoản là đủ, vì `probe_session` trong chế độ này luôn trả "đã đăng nhập" bất
kể nội dung cookie). Chi tiết: `docs/superpowers/notes/2026-09-18-p3-run.md`.

## Build (đóng gói)

```powershell
cd crates/lvc-app
cargo tauri build
```

Biên dịch bản release rồi đóng gói cài đặt Windows. Vì đây là một Cargo workspace (không có
`target-dir` riêng cho `lvc-app`), thư mục `target/` dùng chung nằm ở **gốc repo**, không phải
dưới `crates/lvc-app/` — bộ cài nằm ở `target/release/bundle/nsis/*.exe` (NSIS;
`tauri.conf.json` khai `bundle.targets: "all"` nên có thể sinh thêm `.msi` trong `bundle/msi/`).
Lần build đầu tiên (toàn bộ cây phụ thuộc ở profile release, kể cả các crate đã build ở profile
`dev` cho `cargo tauri dev` phải build lại từ đầu vì khác profile) có thể mất 10–20 phút.

Bộ cài **chưa được ký** (không có chứng chỉ code-signing) — Windows SmartScreen sẽ cảnh báo
"Windows protected your PC" khi chạy lần đầu trên máy khác; chọn "More info" → "Run anyway" để
tiếp tục. Đây là hạn chế đã biết của P3, việc ký bộ cài nằm ở P5.

## Trình duyệt (Chromium/Chrome/Opera)

`lvc-browser` (`crates/lvc-browser/src/executable.rs::resolve_executable`) tìm file `.exe` theo
đúng thứ tự sau, dừng ở ứng viên đầu tiên tồn tại:

1. **Override tường minh**: biến môi trường `LVC_CHROME` (nếu đặt, PHẢI trỏ đúng tới một file có
   thật — nếu không sẽ báo lỗi ngay, không dò tiếp).
2. Theo loại trình duyệt gán cho từng tài khoản (`accounts_meta.browser`, mặc định `chromium`):
   - **Chromium**: `PLAYWRIGHT_BROWSERS_PATH` (nếu đặt) hoặc `%LOCALAPPDATA%\ms-playwright` →
     tìm mọi thư mục `chromium-<rev>\chrome-win64\chrome.exe`, lấy **revision cao nhất**. Bỏ qua
     `chromium_headless_shell-*` (không phải Chrome đầy đủ) và các rev cũ dùng bố cục
     `chrome-win\chrome.exe` (rev < 1148) — không bao giờ rơi về bản cũ.
   - **Chrome**: `%ProgramFiles%`, `%ProgramFiles(x86)%`, `%LOCALAPPDATA%` (theo thứ tự đó) +
     `Google\Chrome\Application\chrome.exe`.
   - **Opera**: `%LOCALAPPDATA%\Programs\Opera\opera.exe` rồi `%ProgramFiles%\Opera\opera.exe`.

**Bắt buộc chạy có giao diện (headed), không headless** — user agent của Chrome headless là
`HeadlessChrome`, Dola/Cloudflare chặn/nghi ngờ ngay. `--user-data-dir` luôn là đường dẫn
**tuyệt đối** (bẫy đã biết: đường dẫn tương đối khiến Chrome tạo profile sai chỗ hoặc dùng lẫn
profile mặc định của Windows).

## Nơi lưu dữ liệu

Mặc định dưới `%APPDATA%\vn.lvcmedia.seedance\`:

| Đường dẫn | Nội dung |
|---|---|
| `lvc.db` | SQLite: tài khoản, workflow đã lưu, lịch sử chạy (run), tác vụ (task), cài đặt |
| `logs\app.log.<ngày>` | Log xoay theo ngày (`tracing-appender`), **không bao giờ ghi cookie/mật khẩu/prompt/proxy vào log** |
| `accounts\<tên>\` | Thư mục profile Chrome/Chromium riêng cho từng tài khoản (mặc định; đổi được trong Cài đặt) |
| `downloads\` | Video tải về (mặc định; đổi được trong Cài đặt) |

Đổi `accountsDir`/`outputDir` trong tab **Cài đặt** cần **khởi động lại** app mới có hiệu lực
(`EngineConfig` dựng một lần lúc khởi động); riêng `threads` áp dụng ngay không cần khởi động lại.

## Nhập dữ liệu từ bản Python

Tab **Cài đặt** → "Nhập dữ liệu từ bản Python" → chọn thư mục cài bản Python cũ
(`A:\ToolLVC\lvcseedance` hoặc bản sao khác) → nhập `tasks.db`/`pool_usage.db` + copy thư mục
profile từng tài khoản (`accounts/<tên>/`) sang thư mục dữ liệu mới. **Chỉ đọc bản Python, không
bao giờ sửa/xoá** — chạy lại nhiều lần vẫn an toàn (idempotent), tài khoản/profile đã có sẽ được
bỏ qua chứ không ghi đè.

Dùng trực tiếp từ Rust (không qua UI):

```rust
let db = Db::open(Path::new("lvc.db")).await?;
let report = import_legacy(&db, Path::new(r"A:\ToolLVC\lvcseedance")).await?;
```

## Gate (chạy trước mọi commit lớn)

```powershell
powershell -ExecutionPolicy Bypass -File scripts/check.ps1
```

Chạy tuần tự: `cargo fmt --check` → `cargo clippy --workspace -D warnings` → clippy riêng cho
feature `fake-driver` → `cargo test --workspace` → kiểm `lvc-core` thuần (không dính
sqlx/tokio/reqwest/chromiumoxide/tauri) → (nếu có `app/package.json`) `pnpm typecheck` →
`pnpm test` → `pnpm build`. Thêm `-SkipFrontend` để bỏ 3 bước frontend (máy không có Node/pnpm,
hoặc chỉ muốn kiểm phần Rust).

## Ràng buộc quan trọng

- **Không bao giờ sửa `A:\ToolLVC\lvcseedance`** (bản Python) — chỉ đọc để nhập dữ liệu.
- Chạy thật (không phải `fake-driver`) sẽ **tốn credit Dola** (~2 credit / video 10 giây) —
  hỏi người dùng trước khi chạy live.
- Không log cookie, mật khẩu, proxy (giá trị thật), hay prompt đầy đủ vào bất kỳ đâu.

## Tài liệu

- Spec thiết kế: `docs/superpowers/specs/2026-09-16-lvc-seedance-rust-design.md`
- Kế hoạch từng giai đoạn: `docs/superpowers/plans/2026-09-16-p0-core-store.md`,
  `2026-09-16-p0.5-align-python-head.md`, `2026-09-16-p1-browser-driver.md`,
  `2026-09-17-p2-dola-video.md`, `2026-09-18-p3-tauri-node-graph.md`
- Ghi chú luồng video bản Python (tham chiếu khi port): `docs/superpowers/notes/2026-09-17-python-video-pipeline.md`
- Hướng dẫn thao tác UI P3 (4 màn) + bảng tra mã lỗi:
  `docs/superpowers/notes/2026-09-18-p3-run.md`
