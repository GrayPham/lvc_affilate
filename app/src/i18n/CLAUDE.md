# i18n — luật module

Đa ngôn ngữ cho toàn app. Mặc định đoán theo ngôn ngữ hệ thống, người dùng đổi bằng nút cờ; lựa
chọn nhớ trong `localStorage` (`lvc.ui.lang`).

## Chia file (một file = một trách nhiệm)

| File | Trách nhiệm |
|---|---|
| `types.ts` | mã ngôn ngữ (`LANGS`), tên hiển thị, kiểu `Vars` |
| `vi.ts` | **từ điển gốc** — nguồn chân lý của tập khoá (`DictKey`) |
| `en.ts` | bản dịch English, kiểu `Record<DictKey, string>` |
| `dicts.ts` | nơi DUY NHẤT ghép mã ngôn ngữ → từ điển |
| `translate.ts` | hàm dịch THUẦN (không React) — test độc lập |
| `store.ts` | zustand: ngôn ngữ hiện tại + ghi nhớ |
| `useT.ts` | hook `useT()` cho component |
| `flags.tsx` | cờ SVG nội tuyến |
| `LanguageSwitcher.tsx` | nút đổi ngôn ngữ |
| `index.ts` | cửa ngõ, chỉ re-export |

## Luật

1. **Thêm khoá**: viết vào `vi.ts` TRƯỚC. `en.ts` kiểu `Record<DictKey, string>` nên thiếu khoá là
   lỗi biên dịch — đừng tắt bằng `as` hay `Partial`.
2. **Thêm ngôn ngữ** = 1 file từ điển + 1 mã ở `LANGS` + 1 dòng ở `dicts.ts` + 1 component cờ ở
   `flags.tsx`. Không sửa chỗ nào khác.
3. **KHÔNG emoji cờ** (🇻🇳): Windows không có glyph regional-indicator, chỉ hiện "VN". Luôn vẽ SVG.
4. Khoá đặt theo miền, chấm phân cấp: `gate.login.submit`. Lỗi IPC đặt `gate.err.<MÃ_LỖI>` để tra
   thẳng từ `IpcError.code`.
5. Chuỗi có biến dùng chỗ giữ `{ten}`, truyền qua `t("key", { ten: x })` — KHÔNG nối chuỗi thủ công
   (trật tự từ mỗi ngôn ngữ một khác).
6. Không nhét chuỗi tiếng Việt cứng vào component mới; đi qua `useT()`.
