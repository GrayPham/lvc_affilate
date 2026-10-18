# Firefox Portable — profile đăng nhập bằng cookie (dùng tay, KHÔNG automation)

Ngày: 2026-10-18. Yêu cầu chủ dự án: "các tk như bên chrome tôi nhập cookie vào, bạn thêm cookie
vào trong profile đó để setup chạy là được" + "tích hợp thêm firefox portable". KHÔNG làm đăng nhập
tự động.

## Bối cảnh / giới hạn cứng (phải nêu)

- Mọi luồng TỰ ĐỘNG của app (verify, chạy workflow, bootstrap token MuseAI) nói **CDP** qua
  `chromiumoxide` → chỉ Chromium. Firefox nói BiDi/Marionette — **không có driver automation** cho
  Firefox trong repo (chỉ một smoke test `#[ignore]`).
- Port driver Firefox chống-phát-hiện (Camoufox) **đã bị safety classifier chặn** — KHÔNG làm lại
  (`docs/superpowers/memory/camoufox-flow-port-blocked.md`).
- ⇒ Firefox Portable ở đây = **profile dùng TAY**: tiêm cookie để người dùng mở Firefox ra là đã
  đăng nhập sẵn. Workflow tự động vẫn chạy trên Chromium. Tài khoản gắn engine Firefox sẽ KHÔNG
  verify/chạy workflow tự động được (UI phải nói rõ, không để người dùng tưởng chạy được).

## Mô hình engine

`BrowserKind` (chromium/chrome/opera) là *họ Chromium* cho `ChromiumDriver`/CDP — KHÔNG nhét Firefox
vào đây (sẽ bị launch như Chromium → hỏng). Thêm khái niệm **engine** ở trên, trực giao:

```
BrowserEngine { Chromium, FirefoxPortable }   // lvc-core, thuần
```

- `Chromium` → đường cũ y hệt (ChromiumDriver + BrowserKind phụ).
- `FirefoxPortable` → đường MỚI, không CDP: tiêm cookies.sqlite + mở tiến trình firefox.exe.

Lưu engine theo tài khoản: credential `meta.engine = "firefox_portable"` (mặc định thiếu =
chromium). (Dola để sau — Dola có pipeline riêng; làm trước cho credential vì đó là nơi dán cookie
nền tảng khác.)

## Giai đoạn

### GĐ1 — Dò firefox.exe (thuần, test được, không có phần dễ vỡ)  ✅ làm trước
- `lvc-core`: `BrowserEngine` enum + parse/as_str + test.
- `lvc-browser/src/firefox/locate.rs`: `resolve_firefox_in(override_, roots) -> Result<PathBuf>`
  (giống `executable.rs`): env `LVC_FIREFOX` → `FirefoxPortable/App/Firefox64/firefox.exe` +
  `.../Firefox/firefox.exe` dưới các gốc → `%ProgramFiles%\Mozilla Firefox\firefox.exe`. Test bằng
  tempdir, không cần Firefox thật.

### GĐ2 — Tiêm cookies.sqlite (phần DỄ VỠ — cần Firefox thật để nghiệm thu)
- Thêm dep `rusqlite` (feature `bundled`) cho `lvc-browser` (sqlite C đã có sẵn trong graph qua
  sqlx của lvc-store). Lý do rusqlite (đồng bộ) thay vì sqlx (bất đồng bộ): ghi MỘT file ngoài,
  một lần, không cần async; lvc-browser không dùng sqlx.
- `lvc-browser/src/firefox/cookies_db.rs`:
  - `fn to_moz_rows(cookies: &[Cookie]) -> Vec<MozCookie>` — THUẦN, ánh xạ Cookie → hàng
    `moz_cookies` (đơn vị: `expiry` giây, `creationTime`/`lastAccessed` MICRO-giây; `schemeMap` =
    2 nếu secure else 1; `originAttributes=""`). Test độc lập.
  - `fn write_cookies(profile_dir, cookies) -> Result<usize>` — tạo `<profile>/cookies.sqlite` với
    ĐÚNG schema `moz_cookies` + `PRAGMA user_version=13` (bản Firefox hiện hành) nếu chưa có; UPSERT
    theo khoá `(name,host,path,originAttributes)`. Firefox PHẢI đóng khi ghi. Test round-trip bằng
    rusqlite (ghi → đọc lại), KHÔNG cần Firefox.
  - ⚠️ RỦI RO: schema sai bản → Firefox dựng lại cookies.sqlite lúc khởi động và DROP cookie ta
    tiêm. Không kiểm được trong CI (không có Firefox) — phải người dùng nghiệm thu trên máy họ (mở
    Firefox Portable thấy đã đăng nhập). Nêu rõ.

### GĐ3 — Engine trong lvc-engine (phân luồng, không match-kind trong driver)
- `provision_profile` (profiles.rs) nhận thêm `engine: BrowserEngine`:
  - `Chromium` → đường CDP cũ (không đổi).
  - `FirefoxPortable` → `firefox::write_cookies(profile_dir, cookies)` (không launch, không CDP).
- `open_profile_viewer` nhận `engine`:
  - `Chromium` → `open_headed` cũ.
  - `FirefoxPortable` → spawn `firefox.exe -profile <dir> -no-remote` (tiến trình thường, không CDP,
    không theo dõi watch_until_closed).

### GĐ4 — Lệnh + frontend
- Lệnh `login_cookie_text`/`credential_open_viewer` đọc `meta.engine` để chọn nhánh.
- AddAccountDialog: nút chọn engine Chrome / Firefox Portable (chỉ nền tảng credential, không Dola).
- EditAccountDialog ("Proxy & trình duyệt"): hiện engine + cảnh báo "Firefox: chỉ dùng tay".
- Lưu `meta.engine` khi thêm/sửa.

## Verify
- `cargo test -p lvc-core` (BrowserEngine), `-p lvc-browser` (locate + to_moz_rows + round-trip DB).
- `scripts/check.ps1` xanh (chú ý: KHÔNG log giá trị cookie — chỉ số lượng/tên).
- THỦ CÔNG (người dùng, không tốn credit): thêm tài khoản engine Firefox + dán cookie → bấm "Mở
  trình duyệt" → Firefox Portable mở ra đã đăng nhập. Đây là bước nghiệm thu phần cookies.sqlite.

## Không làm
- Driver automation Firefox (BiDi/Marionette). Verify/workflow tự động qua Firefox. Camoufox.
