// Hàm dịch THUẦN (không React) để test độc lập: tra khoá trong từ điển của `lang`, thay chỗ giữ
// `{ten}` bằng `vars.ten`. Thiếu khoá trong từ điển phụ thì rơi về tiếng Việt, không bao giờ trả rỗng.
import { DICTS } from "./dicts";
import type { Lang, Vars } from "./types";
import type { DictKey } from "./vi";

export function translate(lang: Lang, key: DictKey, vars?: Vars): string {
  const raw = DICTS[lang][key] || DICTS.vi[key];
  if (!vars) {
    return raw;
  }
  return raw.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const v = vars[name];
    return v === undefined ? whole : String(v);
  });
}
