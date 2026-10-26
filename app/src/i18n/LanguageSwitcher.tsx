// Nút đổi ngôn ngữ: một ô cờ cho mỗi ngôn ngữ trong một khay chung (giống nhóm tab nhỏ ở
// `DlgLicense`: khay nền chìm, ô đang chọn nổi lên nền panel). Cờ không-chọn làm mờ và xám hoá
// để không tranh chú ý với nội dung. Dùng được ở mọi nơi vì chỉ đọc/ghi `useLangStore`.
import { FLAG } from "./flags";
import { useLangStore } from "./store";
import { LANG_LABEL, LANGS } from "./types";

export default function LanguageSwitcher({ className = "" }: { className?: string }) {
  const lang = useLangStore((s) => s.lang);
  const setLang = useLangStore((s) => s.setLang);
  return (
    <div
      className={`inline-flex items-center gap-0.5 rounded-lg bg-panel-2 p-0.5 ${className}`}
      role="group"
    >
      {LANGS.map((code) => {
        const Flag = FLAG[code];
        const active = code === lang;
        return (
          <button
            key={code}
            type="button"
            title={LANG_LABEL[code]}
            aria-label={LANG_LABEL[code]}
            aria-pressed={active}
            onClick={() => setLang(code)}
            className={`inline-flex h-6 w-7 items-center justify-center rounded-[7px] transition-all ${
              active
                ? "bg-panel shadow-card"
                : "opacity-45 grayscale hover:opacity-80 hover:grayscale-0"
            }`}
          >
            <Flag className="h-3 w-[18px]" />
          </button>
        );
      })}
    </div>
  );
}
