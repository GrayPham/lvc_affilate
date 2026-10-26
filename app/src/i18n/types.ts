// Kiểu dùng chung cho i18n. Thêm ngôn ngữ = thêm 1 file từ điển + 1 dòng ở `LANGS`/`LANG_LABEL`
// + 1 dòng ở `dicts.ts` — không sửa chỗ nào khác.
export const LANGS = ["vi", "en"] as const;

export type Lang = (typeof LANGS)[number];

/** Tên ngôn ngữ hiển thị trong chính ngôn ngữ đó (không dịch). */
export const LANG_LABEL: Record<Lang, string> = {
  vi: "Tiếng Việt",
  en: "English",
};

/** Giá trị chèn vào chuỗi dịch qua chỗ giữ `{ten}`. */
export type Vars = Record<string, string | number>;
