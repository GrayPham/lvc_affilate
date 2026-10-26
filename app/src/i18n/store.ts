// Store ngôn ngữ (zustand, cùng kiểu với `store/uiStore.ts`): nhớ lựa chọn qua `localStorage`,
// lần đầu đoán theo ngôn ngữ hệ thống. `localStorage` có thể ném (webview khoá) nên luôn bọc try.
import { create } from "zustand";

import { LANGS, type Lang } from "./types";

const LANG_KEY = "lvc.ui.lang";

function isLang(v: unknown): v is Lang {
  return LANGS.includes(v as Lang);
}

/** Ngôn ngữ đã lưu; chưa có thì đoán theo hệ thống (không phải tiếng Việt → English). */
function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(LANG_KEY);
    if (isLang(saved)) {
      return saved;
    }
  } catch {
    // Đọc không được thì rơi xuống đoán theo hệ thống.
  }
  const sys = typeof navigator === "undefined" ? "" : navigator.language.toLowerCase();
  return sys.startsWith("vi") ? "vi" : "en";
}

type LangStore = {
  lang: Lang;
  setLang: (lang: Lang) => void;
};

export const useLangStore = create<LangStore>((set) => ({
  lang: initialLang(),
  setLang: (lang) => {
    try {
      localStorage.setItem(LANG_KEY, lang);
    } catch {
      // Không lưu được thì chỉ mất ghi nhớ giữa các lần mở, không phải lỗi cần báo.
    }
    set({ lang });
  },
}));
