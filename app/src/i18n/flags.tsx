// Cờ quốc gia vẽ bằng SVG nội tuyến — KHÔNG dùng emoji cờ (🇻🇳) vì Windows không có glyph cho
// regional indicator, chỉ hiện hai chữ "VN". Mỗi cờ là một component riêng, thêm ngôn ngữ = thêm
// một component + một dòng ở `FLAG`.
import type { Lang } from "./types";

type FlagProps = { className?: string };

const BOX = "shrink-0 rounded-[3px] shadow-[0_0_0_1px_rgb(0_0_0/0.12)]";

/** Việt Nam: nền đỏ, sao vàng 5 cánh ở giữa. */
export function FlagVi({ className = "" }: FlagProps) {
  return (
    <svg viewBox="0 0 60 30" className={`${BOX} ${className}`} aria-hidden="true">
      <rect width="60" height="30" fill="#DA251D" />
      <polygon
        fill="#FF0"
        points="30,6 32.02,12.22 38.56,12.22 33.27,16.06 35.29,22.28 30,18.44 24.71,22.28 26.73,16.06 21.44,12.22 27.98,12.22"
      />
    </svg>
  );
}

/** Anh (Union Jack) — bản rút gọn, đủ nhận ra ở cỡ 16px. */
export function FlagEn({ className = "" }: FlagProps) {
  return (
    <svg viewBox="0 0 60 30" className={`${BOX} ${className}`} aria-hidden="true">
      <rect width="60" height="30" fill="#012169" />
      <path d="M0,0 L60,30 M60,0 L0,30" stroke="#FFF" strokeWidth="6" />
      <path d="M0,0 L60,30 M60,0 L0,30" stroke="#C8102E" strokeWidth="3" />
      <path d="M30,0 V30 M0,15 H60" stroke="#FFF" strokeWidth="10" />
      <path d="M30,0 V30 M0,15 H60" stroke="#C8102E" strokeWidth="6" />
    </svg>
  );
}

export const FLAG: Record<Lang, (p: FlagProps) => React.ReactElement> = {
  vi: FlagVi,
  en: FlagEn,
};
