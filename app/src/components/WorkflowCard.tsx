// Thẻ trong kho workflow — dựng theo `references/lvc-ui-design/screens/Library.dc.html`
// (ảnh bìa 16/10, huy hiệu đếm ở góc phải, dải vạch dưới đáy, tiêu đề, mô tả, một nút hành động).
//
// Khác bản thiết kế một chỗ CÓ CHỦ ĐÍCH: bản vẽ ghi "4 cảnh" và "9:16 · 20s". App không biết số
// cảnh hay thời lượng của một workflow trước khi chạy nó — số cảnh do dữ liệu đầu vào quyết định
// lúc thêm vào hàng chờ. Thứ app BIẾT CHẮC là số node, nên thẻ ghi "N node". Thà nói đúng một con
// số nhỏ hơn là in ra một con số nghe hay mà sai.
import { useState } from "react";
import type { ReactNode } from "react";

/** Ảnh bìa có sẵn trong `app/public/covers/`. */
const COVERS = [
  "g_live",
  "g_food",
  "g_fashion",
  "g_tech",
  "review",
  "hook",
  "char",
  "testi",
  "beforeafter",
  "demo",
  "unbox",
  "story",
  "multi",
  "basic",
] as const;

/**
 * Chọn ảnh bìa ỔN ĐỊNH theo khoá (id/tên) khi mục đó không tự khai bìa.
 *
 * Phải ổn định: cùng một workflow luôn ra cùng một bìa qua mọi lần mở app, nếu không người dùng
 * sẽ không nhận ra thẻ của mình. THUẦN, export để test.
 */
export function coverFor(key: string, declared?: string): string {
  if (declared !== undefined && declared !== "") {
    return declared;
  }
  let hash = 0;
  for (let i = 0; i < key.length; i += 1) {
    hash = (hash * 31 + key.charCodeAt(i)) >>> 0;
  }
  return COVERS[hash % COVERS.length]!;
}

/**
 * Đường dẫn ảnh thật từ giá trị bìa.
 *
 * Ba dạng đều phải nhận được, vì ba nguồn khai bìa khác nhau:
 * - `g_food`        — tên bìa dựng sẵn trong `app/public/covers/`
 * - `https://…`     — mục kho CỘNG ĐỒNG trỏ ảnh trên GitHub
 * - `data:image/…`  — mẫu riêng người dùng tự chụp canvas
 *
 * THUẦN, export để test: nối nhầm `/covers/https://….svg` là cách ảnh bìa vỡ thành ô đen.
 */
export function coverSrc(cover: string): string {
  if (/^(data:|https?:|\/)/.test(cover)) {
    return cover;
  }
  return `/covers/${cover}.svg`;
}

/** Số vạch dưới đáy bìa — chặn trên để thẻ không thành một dải kẻ li ti. */
const MAX_TICKS = 8;

export default function WorkflowCard({
  title,
  description,
  cover,
  nodeCount,
  topLeftBadge,
  footer,
  action,
}: {
  title: string;
  description: string;
  /** Tên bìa (`g_food`) hoặc data URL. */
  cover: string;
  nodeCount: number;
  /** Nhãn góc trái bìa (vd nhóm) — bỏ trống thì không hiện. */
  topLeftBadge?: string;
  /** Hàng phụ dưới mô tả (vd tác giả · phiên bản). */
  footer?: ReactNode;
  action: ReactNode;
}) {
  // Bìa của mục cộng đồng nằm trên mạng — mất mạng là ảnh vỡ thành ô đen. Hỏng thì lùi về một
  // bìa dựng sẵn (chọn theo tiêu đề nên vẫn ổn định), thẻ luôn còn nhìn được.
  const [failed, setFailed] = useState(false);
  const src = failed ? coverSrc(coverFor(title)) : coverSrc(cover);
  const ticks = Math.min(Math.max(nodeCount, 1), MAX_TICKS);

  return (
    <article className="flex flex-col overflow-hidden rounded-xl border border-line bg-panel">
      <div className="relative aspect-[16/10] overflow-hidden bg-stage">
        <img
          src={src}
          alt={`Ảnh bìa: ${title}`}
          loading="lazy"
          onError={() => setFailed(true)}
          className="absolute inset-0 block h-full w-full object-cover"
        />
        <span className="absolute inset-0 bg-gradient-to-b from-transparent via-transparent to-black/55" />
        {topLeftBadge ? (
          <span className="absolute top-2 left-2 rounded-full bg-black/65 px-2.5 py-[3px] text-[11px] font-semibold text-[#F2F1EE]">
            {topLeftBadge}
          </span>
        ) : null}
        {nodeCount > 0 ? (
          <span className="absolute top-2 right-2 rounded-full bg-black/65 px-2.5 py-[3px] text-[11px] font-semibold text-[#F2F1EE]">
            {nodeCount} node
          </span>
        ) : null}
        <span className="absolute right-[9px] bottom-1.5 left-[9px] flex gap-[3px]">
          {Array.from({ length: ticks }, (_, i) => (
            <span key={i} className="h-[3px] flex-1 rounded-[2px] bg-white/75" />
          ))}
        </span>
      </div>

      <div className="flex flex-1 flex-col gap-1.5 p-3">
        <h3 className="text-[13.5px] leading-[1.35] font-semibold text-ink">{title}</h3>
        <p className="flex-1 text-[12px] leading-[1.5] text-ink3">{description}</p>
        {footer}
        {action}
      </div>
    </article>
  );
}
