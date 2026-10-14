# Thiết kế — Cơ chế auto-update (Tauri updater + GitHub Releases)

> Ngày: 2026-10-13 · Liên quan: ADR-R014 (node độc lập — không áp dụng, đây là hạ tầng app-level,
> không phải node), `2026-10-11-remote-template-registry-design.md` (tiền lệ "repo GitHub phụ, tách
> khỏi source" — ADR-R019, tái dùng tinh thần đó ở đây).
> Trạng thái: **THIẾT KẾ — chưa code.** Quyết định kiến trúc then chốt (ADR-R023) đã chốt với người
> dùng 2026-10-13. Phần còn lại (CI cụ thể, UI) cần duyệt plan riêng trước khi code (theo quy ước
> CLAUDE.md gốc).

## 1. Bối cảnh / câu hỏi gốc

Người dùng muốn: tận dụng kho lưu trữ GitHub repo hiện có (`github.com/GrayPham/lvc_affilate`) để
tự động phát hiện + cài bản cập nhật mới cho app desktop (Tauri 2, Windows) đang chạy trên máy người
dùng cuối, thay vì phải gửi file cài đặt thủ công mỗi lần có bản mới.

## 2. Hiện trạng (đã xác minh bằng cách đọc code, không suy đoán)

- **Không có gì** cho auto-update: không `tauri-plugin-updater`/`tauri-plugin-process` (Rust lẫn
  JS), không `plugins.updater` trong `crates/lvc-app/tauri.conf.json`, không `.github/workflows/*`
  (thư mục `.github` chưa tồn tại), không code-signing (không `TAURI_SIGNING_PRIVATE_KEY`, không
  file `.key`/`.pub`/`.minisign` nào trong repo).
- **3 nơi giữ version độc lập, không đồng bộ bằng script nào**: `crates/lvc-app/Cargo.toml`
  (`version = "0.1.0"`), `app/package.json` (`"version": "0.1.0"`), `crates/lvc-app/tauri.conf.json`
  (`"version": "0.1.0"`). Hiện trùng nhau tình cờ, không có `[workspace.package].version` dùng
  chung.
- `tauri.conf.json`: `"identifier": "vn.lvcmedia.seedance"`, `bundle.targets: "all"`, schema
  `config/2` (xác nhận Tauri v2). Không có `windows.certificateThumbprint` (chưa ký Authenticode
  Windows — khác với chữ ký updater của Tauri, xem mục 5).
- `Cargo.toml` gốc: `license = "proprietary"` → repo source **private** trên GitHub (xác nhận qua
  `.git/config` remote `https://github.com/GrayPham/lvc_affilate.git`, không có `LICENSE` file).
- Không có spec/plan nào trước đây về release/update/distribution trong
  `docs/superpowers/{specs,plans}/`.

## 3. Vì sao repo private đổi hẳn kiến trúc

`tauri-plugin-updater` chỉ gọi MỘT URL tĩnh, kỳ vọng response là đúng JSON schema của Tauri:

```json
{
  "version": "1.2.0",
  "notes": "...",
  "pub_date": "2026-10-13T00:00:00Z",
  "platforms": {
    "windows-x86_64": { "signature": "...", "url": "https://.../lvc-affiliate_1.2.0_x64-setup.nsis.zip" }
  }
}
```

Với repo **public**, `github.com/<owner>/<repo>/releases/latest/download/latest.json` trả thẳng
file này, không cần auth — "tận dụng GitHub repo" đúng nghĩa, zero hạ tầng thêm. Với repo **private**,
URL đó 404 với client ẩn danh; client phải tự xác thực (token), nhưng nhúng token đọc-private vào
một binary phát tán cho nhiều máy là rủi ro (token có thể bị trích xuất ngược từ binary đã build).

## 4. Quyết định kiến trúc

### ADR-R023 — Repo release RIÊNG, public, tách khỏi repo source (đã chốt với người dùng)

Giữ `lvc_affilate` (source) **private** nguyên trạng. Tạo một repo GitHub **mới, public**, ví dụ
`lvc-affiliate-releases` — repo này **không chứa source code**, chỉ chứa:
- Các bản installer đã build + ký (`.exe` NSIS, có thể thêm `.msi`).
- File `latest.json` (schema Tauri ở mục 3).
- Release notes (mô tả ngắn mỗi bản, tự sinh từ tag message hoặc CHANGELOG).

CI chạy trong repo **source** (có quyền truy cập secrets ký + checkout code), build xong thì
**publish sang repo release** bằng một Personal Access Token phạm vi hẹp (chỉ `contents: write`
trên đúng repo release đó, không đụng tới repo source). Vì repo release là **public**, endpoint
`github.com/<owner>/lvc-affiliate-releases/releases/latest/download/latest.json` chạy được với
client ẩn danh — **không có secret nào nằm trong app đã phát hành cho người dùng cuối.**

Lý do chọn (so với 2 phương án còn lại đã trình bày):
- So với "mở public cả repo source": giữ được source code private tuyệt đối — đây là lý do người
  dùng chọn phương án này.
- So với "nhúng PAT private-repo vào app": không có secret nào bị lộ nguy cơ reverse-engineer, không
  cần kế hoạch xoay vòng token phía client.
- Chi phí đánh đổi duy nhất: thêm một repo GitHub rỗng phải quản lý (không cần maintain code, chỉ là
  nơi chứa release — tiền lệ đã có ở `ADR-R019` cho `lvc-template-registry`, cùng tinh thần "repo
  phụ public cho nội dung không nhạy cảm").

### ADR-R024 — Version: 1 nguồn sự thật + script đồng bộ, không tay sửa 3 chỗ

Thêm `[workspace.package] version = "x.y.z"` vào `Cargo.toml` gốc; `crates/lvc-app/Cargo.toml` đổi
`version = "0.1.0"` → `version.workspace = true`. Thêm `scripts/bump-version.ps1 -Version x.y.z`:
ghi version vào `Cargo.toml` (workspace.package), `app/package.json`, `crates/lvc-app/tauri.conf.json`
— cả 3 cùng lúc, rồi tự tạo git tag `vx.y.z` (không tự push — người dùng tự `git push --tags` khi
sẵn sàng phát hành, tránh CI chạy ngoài ý muốn).

### ADR-R025 — Ký bản cập nhật bằng khoá riêng của Tauri updater (minisign/Ed25519), KHÔNG phải Authenticode

Hai khái niệm "ký" khác nhau, không nhầm lẫn:
- **Chữ ký updater của Tauri** (bắt buộc cho cơ chế này): keypair Ed25519 sinh 1 lần bằng
  `cargo tauri signer generate`. Public key nhúng vào `tauri.conf.json` (`plugins.updater.pubkey`).
  Private key + mật khẩu lưu làm GitHub Actions secret (`TAURI_SIGNING_PRIVATE_KEY`,
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) **trên repo source** (nơi chạy CI build). Đây là cơ chế xác
  minh "bản update này đúng là do mình build", bắt buộc phải có để `tauri-plugin-updater` chấp nhận
  cài.
- **Windows Authenticode code-signing** (tuỳ chọn, KHÔNG bắt buộc cho auto-update chạy được): chứng
  chỉ ký `.exe`/`.msi` để Windows SmartScreen không cảnh báo "Unknown publisher". Hiện chưa có, chi
  phí mua chứng chỉ (vd. qua SignPath/DigiCert) — để ngoài phạm vi thiết kế này, ghi vào mục 8 làm
  việc tương lai, không chặn auto-update hoạt động.

### ADR-R026 — Check thủ công/khi mở app, KHÔNG tự âm thầm tải + cài

`check()` chạy ngầm lúc app khởi động (không chặn UI); nếu có bản mới, hiện **banner** (không phải
dialog chặn thao tác) "Có bản vX.Y.Z mới — Xem ghi chú / Cài đặt". Chỉ khi người dùng bấm "Cài đặt"
mới `downloadAndInstall()` rồi `relaunch()`. Thêm nút "Kiểm tra cập nhật" thủ công trong trang Cài
đặt (`SettingsPage.tsx`) cho người muốn chủ động kiểm tra.

Lý do: app đang chạy workflow có thể đang giữa một lần render/chạy tốn credit Dola (luật #2 CLAUDE.md
gốc) — ép cài + relaunch giữa chừng sẽ làm mất tiến trình đang chạy. Không bao giờ tự relaunch mà
không hỏi.

### ADR-R027 — Vị trí code: app-level, không phải node/provider

Đây là hạ tầng của chính `lvc-app` (Tauri shell), không phải một loại node trên canvas và không phải
client gọi dịch vụ AI — nên **không** tạo crate mới trong `crates/nodes/*` hay `crates/providers/*`.
Code Rust nằm thẳng trong `crates/lvc-app` (đăng ký plugin trong builder, ví dụ
`src/lib.rs`/`main.rs`). Code frontend: `app/src/store/updater.ts` (state check/download/progress) +
`app/src/components/UpdateBanner.tsx` (UI banner) + 1 section nhỏ trong `SettingsPage.tsx`.

**Cập nhật khi code (2026-10-13):** app chỉ chạy Windows → bỏ `tauri-plugin-process`/`relaunch()` —
`Update.install()`/`downloadAndInstall()` trên Windows tự thoát app rồi chạy installer NSIS, installer
tự khởi động lại app sau khi cài xong (hành vi mặc định của Tauri updater, không cần gọi gì thêm).

## 5. Luồng end-to-end

```
Dev: scripts/bump-version.ps1 -Version 0.2.0
     -> sửa Cargo.toml/package.json/tauri.conf.json, tạo tag v0.2.0 (local)
Dev: git push && git push --tags
     -> tag v0.2.0 lên repo SOURCE (private) kích hoạt .github/workflows/release.yml

CI (chạy trong repo SOURCE, có secrets):
  1. checkout + setup rust/node, npm ci (app/)
  2. tauri-action build Windows NSIS (+ MSI nếu cần) bundle
  3. ký bằng TAURI_SIGNING_PRIVATE_KEY(_PASSWORD) -> sinh .sig
  4. sinh latest.json (version, notes từ tag message, pub_date, platforms.windows-x86_64)
  5. publish: tạo release trong repo RELEASE (public) bằng PAT cross-repo,
     upload installer + .sig + latest.json làm release assets, tag v0.2.0

Client (app đã cài, đang chạy máy người dùng):
  1. Lúc mở app (hoặc bấm "Kiểm tra cập nhật"): check() gọi
     github.com/<owner>/lvc-affiliate-releases/releases/latest/download/latest.json (ẩn danh, public)
  2. version trong latest.json > version hiện tại -> hiện UpdateBanner
  3. Người dùng bấm "Cài đặt" -> downloadAndInstall() (xác minh chữ ký bằng pubkey nhúng sẵn)
     -> relaunch()
```

## 6. Thay đổi cụ thể cần làm khi implement (chưa làm — liệt kê để duyệt plan riêng)

- `Cargo.toml` (gốc): thêm `[workspace.package] version`.
- `crates/lvc-app/Cargo.toml`: `version.workspace = true`; thêm `tauri-plugin-updater = "2"`.
- `crates/lvc-app/tauri.conf.json`: thêm khối
  ```json
  "plugins": {
    "updater": {
      "pubkey": "<base64 pubkey sinh ở ADR-R025>",
      "endpoints": [
        "https://github.com/<owner>/lvc-affiliate-releases/releases/latest/download/latest.json"
      ]
    }
  }
  ```
- `crates/lvc-app/src/lib.rs` (hoặc nơi dựng `tauri::Builder`): `.plugin(tauri_plugin_updater::Builder::new().build())`.
- `app/package.json`: thêm `@tauri-apps/plugin-updater`.
- `app/src/store/updater.ts`: state machine `idle -> checking -> available(info) -> downloading(progress) -> readyToRelaunch`.
- `app/src/components/UpdateBanner.tsx`: hiển thị theo state trên, mount 1 lần ở `App.tsx` (giống
  `engine_bridge.ts` đăng ký 1 lần).
- `app/src/pages/SettingsPage.tsx`: thêm nút "Kiểm tra cập nhật" gọi lại cùng store.
- `scripts/bump-version.ps1`: script mới (ADR-R024).
- Repo mới `lvc-affiliate-releases` (ngoài workspace này, giống cách `lvc-template-registry` được
  tạo ở P7a) — chỉ cần `gh repo create <owner>/lvc-affiliate-releases --public`, không cần nội dung
  ban đầu (CI tự tạo release đầu tiên).
- `.github/workflows/release.yml` trong repo SOURCE này — trigger `push: tags: ["v*.*.*"]`, dùng
  `tauri-apps/tauri-action@v0` với input trỏ sang repo release (owner/repo override + token riêng).
- Secrets cần tạo tay (không tự động hoá bằng code, hành động ngoài git):
  `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `RELEASE_REPO_TOKEN` (PAT
  fine-grained, chỉ `contents:write` trên `lvc-affiliate-releases`) — set trong
  Settings → Secrets của repo `lvc_affilate` trên GitHub.

## 7. Luật bảo mật áp dụng (nhắc lại CLAUDE.md, không đổi)

1. Không secret nào (cookie, API key, proxy, token) xuất hiện trong repo release công khai hay trong
   binary phát hành — chỉ installer + chữ ký + metadata version/notes.
2. `lvc-core` vẫn THUẦN — plugin updater đăng ký ở `lvc-app`, không rò vào `lvc-core`.
3. Request `check()` từ client không gửi kèm bất kỳ secret nào (endpoint public, ẩn danh).
4. Private key ký updater + PAT cross-repo chỉ tồn tại dạng GitHub Actions secret của repo source,
   không bao giờ commit vào git, không log ra CI output.

## 8. Việc CHƯA làm / câu hỏi mở (cần chốt trước hoặc trong lúc implement)

1. Tên repo release chính xác: tạm đề xuất `lvc-affiliate-releases` — người dùng có thể đổi tên khi
   `gh repo create`, không ảnh hưởng thiết kế.
2. `releaseDraft: true` (CI tạo bản nháp, người dùng tự bấm "Publish" trên GitHub sau khi tải về tự
   test) hay publish thẳng ngay khi CI xanh? Khuyến nghị **draft trước** cho vài release đầu để có
   lưới an toàn, sau này ổn định có thể chuyển publish thẳng.
3. Windows Authenticode code-signing (SmartScreen) — làm ngay hay để sau (xem ADR-R025)? Không chặn
   auto-update hoạt động, chỉ ảnh hưởng trải nghiệm cài đặt lần đầu/cảnh báo Windows.
4. Có cần kênh "beta"/"stable" riêng (tag prefix khác, endpoint khác) hay chỉ 1 kênh ổn định cho bây
   giờ? Đề xuất: chỉ 1 kênh ở bản đầu, mở rộng sau nếu cần.
5. Release notes: tự lấy từ `git tag -l -n<N> vX.Y.Z` message, hay có `CHANGELOG.md` riêng người viết
   tay rồi CI đọc vào? Cần quyết định trước khi viết bước sinh `latest.json.notes`.

## 9. Quyết định đã chốt (2026-10-13)

- Kiến trúc phân phối: **repo release riêng, public, tách khỏi source** (ADR-R023) — người dùng đã
  chọn qua so sánh 3 phương án (mở public toàn repo / repo release riêng / nhúng PAT private).
- Version: 1 nguồn sự thật + script đồng bộ (ADR-R024).
- Ký: Ed25519 updater key (bắt buộc) tách biệt Authenticode (tuỳ chọn, để sau) — ADR-R025.
- UX: check ngầm + banner, không tự ý tải/cài/relaunch (ADR-R026).
- Vị trí code: app-level trong `lvc-app`, không phải node/provider (ADR-R027).
- Thứ tự tiếp theo: viết **plan** chi tiết (`docs/superpowers/plans/`) cho từng bước ở mục 6 trước
  khi code, theo đúng quy ước CLAUDE.md gốc — CHƯA code gì ở bước này.
