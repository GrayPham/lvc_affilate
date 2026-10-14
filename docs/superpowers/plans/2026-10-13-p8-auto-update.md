# P8 — Auto-update (Tauri updater + GitHub Releases)

Trạng thái: **đang làm (2026-10-13)**. Spec: `docs/superpowers/specs/2026-10-13-auto-update-design.md`
(ADR-R023..R027 — đã chốt với người dùng: repo release riêng/public, tách khỏi source private).

## Mục tiêu

App tự kiểm tra bản mới qua GitHub Releases của một repo release công khai riêng
(`lvc-affiliate-releases`), hiện banner cho người dùng chủ động bấm cài — không tự ý tải/cài/relaunch.

## Ranh giới lượt này (P8a — code trong repo này)

Chỉ sửa code trong `lvc_affilate` (repo đang làm việc). **Không** tạo repo GitHub mới, **không**
chạy `gh secret set`, **không** push tag — những việc đó cần tài khoản GitHub của người dùng
(`gh auth status` xác nhận máy này chưa đăng nhập `gh`) và có tác động ra ngoài (tạo repo public,
lưu secret) nên để người dùng tự làm theo checklist cuối file.

## P8a — Việc làm trong repo

1. **Version**: `Cargo.toml` gốc thêm `[workspace.package].version = "1.0.0"` (khớp
   `tauri.conf.json`/`package.json` hiện tại); `crates/lvc-app/Cargo.toml` đổi sang
   `version.workspace = true` (đang lệch ở `0.1.0`, gộp về 1.0.0 cho cả 3 nơi).
2. **Keypair ký updater**: sinh bằng `cargo tauri signer generate` (Ed25519), KHÔNG commit private
   key vào git — lưu ở scratchpad, in public key để nhúng `tauri.conf.json`, hướng dẫn người dùng tự
   cất private key + mật khẩu an toàn (password manager) rồi set làm GitHub secret.
3. **Rust**: thêm `tauri-plugin-updater` vào `crates/lvc-app/Cargo.toml`; đăng ký `.plugin(...)`
   trong `crates/lvc-app/src/lib.rs` (không cần `tauri-plugin-process` — app chỉ chạy Windows,
   `Update.install()` tự thoát + chạy installer NSIS, installer tự relaunch app).
4. **`tauri.conf.json`**: thêm `bundle.createUpdaterArtifacts: true` + khối `plugins.updater`
   (`pubkey`, `endpoints` trỏ
   `github.com/GrayPham/lvc-affiliate-releases/releases/latest/download/latest.json`).
5. **Frontend**: `@tauri-apps/plugin-updater` vào `app/package.json` (cài bằng **pnpm**, không phải
   npm — repo dùng `pnpm-lock.yaml`);
   `app/src/store/updater.ts` (state machine thuần, test được); `app/src/components/UpdateBanner.tsx`;
   mount 1 lần trong `App.tsx`; nút "Kiểm tra cập nhật" trong `SettingsPage.tsx`.
6. **`scripts/bump-version.ps1`**: ghi version vào 3 nơi (Cargo.toml workspace.package,
   package.json, tauri.conf.json) + tạo git tag `vX.Y.Z` cục bộ (không tự push).
7. **`.github/workflows/release.yml`**: trigger tag `v*.*.*`, build Windows NSIS bằng
   `tauri-apps/tauri-action@v0`, ký bằng secret, publish sang repo release qua PAT cross-repo.
8. Chạy `scripts/check.ps1` xác nhận không vỡ luật hiện có (frontend build dùng plugin mới vẫn phải
   qua `tsc`/`vitest`/`vite build`).

## P8b — Việc người dùng tự làm (ngoài phạm vi code, cần tài khoản GitHub thật)

Xem checklist cuối báo cáo lượt này trong hội thoại — tóm tắt: tạo repo
`GrayPham/lvc-affiliate-releases` (public, rỗng), tạo PAT fine-grained (`contents:write` chỉ repo
đó), set 3 secret trên repo `lvc_affilate` (`TAURI_SIGNING_PRIVATE_KEY`,
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `RELEASE_REPO_TOKEN`), rồi `git push --tags` để kích hoạt CI
lần đầu.

## Câu hỏi mở còn lại (không chặn P8a, quyết định khi P8b)

Giữ nguyên từ spec mục 8: tên repo release (đã chọn mặc định `lvc-affiliate-releases`, đổi được),
`releaseDraft` hay publish thẳng, Authenticode (để sau), 1 kênh hay nhiều kênh, nguồn release notes.
