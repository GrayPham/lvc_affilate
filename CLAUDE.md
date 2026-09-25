# LVC Seedance v2 — luật làm việc trong repo

Rust workspace + Tauri 2 + React Flow. Tạo ảnh/video AI (Dola trước, sau: Gemini, ChatGPT…) bằng
workflow node kéo thả. Thiết kế: `docs/superpowers/specs/` (đọc spec mới nhất trước khi sửa lớn),
kế hoạch từng giai đoạn: `docs/superpowers/plans/`.

## Luật cứng

1. **Không bao giờ sửa `A:\ToolLVC\lvcseedance`** (bản Python) — chỉ đọc làm tham chiếu.
2. **Chạy thật tốn credit Dola** (~2 credit / video 10s). Hỏi người dùng trước mọi lần chạy live;
   dev dùng `cargo tauri dev --features fake-driver`, test dùng `FakeDriver`/host giả/wiremock.
3. **Không log bí mật**: cookie, mật khẩu, TOTP, API key, proxy có mật khẩu, prompt đầy đủ.
   Bí mật không bao giờ đi ra UI qua IPC (UI chỉ thấy dạng che `••••1234`).
4. MSRV 1.85: **không let-chains**. `clippy::pedantic` + `-D warnings`.
5. `lvc-core` THUẦN — không `sqlx`/`tokio`/`reqwest`/`chromiumoxide`/`tauri`.
6. Cổng chất lượng trước commit: `powershell -ExecutionPolicy Bypass -File scripts/check.ps1`
   (fmt, clippy ×2, test, luật phụ thuộc, frontend typecheck/test/build).

## Node độc lập (ADR-R014) — luật quan trọng nhất khi thêm tính năng

Mỗi loại node = **một crate** `crates/nodes/node-<ten>/` (+ tuỳ chọn một thư mục UI
`app/src/nodes/<kind>/`). Node chỉ biết **đầu vào / đầu ra / tham số / loại tài khoản cần**.

- Node **chỉ được** phụ thuộc: `lvc-node-api`, `lvc-core`, `crates/providers/*`, thư viện ngoài.
  **Cấm**: node khác, `lvc-engine`, `lvc-store`, `lvc-browser`, `lvc-app`, `tauri`
  (`check.ps1` kiểm bằng `cargo tree -e normal`). Test được dùng node khác qua dev-dependency.
- Provider (`crates/providers/lvc-<ten>`) là client dịch vụ AI dùng chung giữa các node cùng nhà
  cung cấp; **không biết khái niệm node**. Tài nguyên dùng chung do host (engine) giữ thì provider
  khai *trait dịch vụ* (vd `DolaVideoHost`), engine impl, node lấy qua `ctx.services.get::<T>()`.
- Engine **không `match` theo kind**. Thêm node = crate mới + 1 dòng trong
  `crates/lvc-app/src/registry.rs` + 1 dòng dependency trong `crates/lvc-app/Cargo.toml`.
- Lỗi một node chỉ làm hỏng node đó (và node phụ thuộc nó); `NodeError::fatal` mới dừng cả run.
- Tạo khung node mới: `powershell -File scripts/new-node.ps1 -Kind gemini_video -Category video`.
  Khung có sẵn `CLAUDE.md` (luật node), `MEMORY.md` (ghi nhớ kỹ thuật), `README.md` (cho người
  dùng), `manifest.json` (hợp đồng, sinh bằng `LVC_WRITE_FIXTURES=1 cargo test -p node-<ten>`).
- Đổi hình dạng `params` → tăng `manifest.version` và tự migrate trong node.
- `kind` là khoá vĩnh viễn (lưu trong workflow/template của người dùng). Đổi tên = migrate trong
  `lvc_core::graph::migrate`.

## Ghi nhớ khi làm việc

- Học được quirk của một node/provider (mã lỗi lạ, giới hạn server, bẫy giao thức) → ghi vào
  `MEMORY.md` của CHÍNH node/provider đó, không rải vào nơi khác.
- Fixture IPC (`app/src/ipc/fixtures/*.json`) sinh từ Rust:
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`. Không sửa tay.
- Test `lvc-browser::dola::labels::chep_nguyen_van_khop_video_worker_ui_py` so với file Python
  ĐANG SỐNG — đỏ khi bản Python đổi; đó là tín hiệu cần đồng bộ, không phải lỗi refactor.
