// Primitive dung chung cho toan bo UI moi — dung theo ban ban giao
// `references/lvc-ui-design` (xem HANDOFF.md). MOI man/dialog deu lap lai dung may hinh dang nay,
// nen tach ra mot cho: sua o day la sua toan app, khong phai di tung file.
//
// Quy uoc kich thuoc (HANDOFF "Design token"): bo goc nut/o nhap 9–10px (`rounded-md`), the 12–13px
// (`rounded-lg`), dialog 16px (`rounded-xl`); chieu cao toi thieu nut 40px, nut trong bang 34px.
import type { ReactNode, SVGProps } from "react";
import { createPortal } from "react-dom";

// ---------------------------------------------------------------------------------------------
// Dialog — vo chung: nen mo, the giua man, header/body/footer.
// ---------------------------------------------------------------------------------------------

interface DialogProps {
  title: string;
  /** Dong phu duoi tieu de (vd "Áp dụng cho 2 tài khoản: reed01, thorne_fb"). */
  subtitle?: string;
  /** Be rong the — ban thiet ke dung 560/580/620px tuy dialog. */
  width?: number;
  /** `true` = nut goc phai la "thu nho" (–) thay vi "dong" (✕) — xem CheckProgress. */
  minimize?: boolean;
  onClose: () => void;
  children: ReactNode;
  /** Hang nut duoi cung; dung `DialogFooter` de co dung khoang cach/nen. */
  footer?: ReactNode;
}

export function Dialog({
  title,
  subtitle,
  width = 560,
  minimize = false,
  onClose,
  children,
  footer,
}: DialogProps) {
  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/55 p-8"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        style={{ width, boxShadow: "var(--dialog-shadow)" }}
        className="max-w-full overflow-hidden rounded-xl border border-line2 bg-panel text-ink"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2.5 border-b border-line px-[18px] py-4">
          <div className="min-w-0 flex-1">
            <h2 className="text-[16px] font-bold leading-[1.35]">{title}</h2>
            {subtitle ? <p className="mt-[3px] text-[12.5px] text-ink3">{subtitle}</p> : null}
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={minimize ? "Ẩn xuống, vẫn chạy nền" : "Đóng"}
            className="grid h-9 w-9 place-items-center rounded-md text-[16px] text-ink3 hover:bg-fill hover:text-ink"
          >
            {minimize ? "–" : "✕"}
          </button>
        </div>
        <div className="flex flex-col gap-4 px-[18px] py-4">{children}</div>
        {footer}
      </div>
    </div>,
    document.body,
  );
}

/** Hang nut duoi dialog — nen `fill`, vien tren, can phai. `hint` hien ben trai. */
export function DialogFooter({ hint, children }: { hint?: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center gap-2 border-t border-line bg-fill px-[18px] py-3.5">
      {hint ? <span className="flex-[1_1_200px] text-[12.5px] text-ink3">{hint}</span> : null}
      {hint ? null : <span className="flex-1" />}
      {children}
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// Nut — 3 dang trong ban thiet ke: chinh (accent), phu (vien), nguy hiem (vien do).
// ---------------------------------------------------------------------------------------------

type BtnVariant = "primary" | "secondary" | "danger";

const BTN_BASE =
  "inline-flex items-center justify-center rounded-md text-[13px] disabled:cursor-not-allowed disabled:opacity-50";
const BTN_VARIANT: Record<BtnVariant, string> = {
  primary: "bg-accent font-bold text-accent-ink hover:brightness-105",
  secondary: "border border-line2 bg-fill text-ink2 hover:text-ink",
  danger: "border border-rose/40 text-rose hover:bg-rose/10",
};

export function Button({
  variant = "secondary",
  small = false,
  className = "",
  ...rest
}: { variant?: BtnVariant; small?: boolean } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const size = small ? "min-h-[34px] px-2.5 text-[12.5px]" : "min-h-10 px-4";
  return <button type="button" className={`${BTN_BASE} ${size} ${BTN_VARIANT[variant]} ${className}`} {...rest} />;
}

// ---------------------------------------------------------------------------------------------
// Switch — 36×20, num 14px (ban thiet ke dung o bang tai khoan + dialog).
// ---------------------------------------------------------------------------------------------

export function Switch({
  checked,
  onChange,
  label,
  disabled = false,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  /** Doc cho trinh doc man hinh (vd "Lịch chạy reed01"). */
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={
        "relative h-5 w-9 shrink-0 rounded-[10px] transition-colors disabled:opacity-50 " +
        (checked ? "bg-accent" : "bg-line2")
      }
    >
      <span
        className={
          "absolute top-[3px] h-3.5 w-3.5 rounded-[7px] transition-[left] " +
          (checked ? "left-[19px] bg-accent-ink" : "left-[3px] bg-ink3")
        }
      />
    </button>
  );
}

// ---------------------------------------------------------------------------------------------
// Segmented — nhom nut chon mot (Cách đăng nhập, Trình duyệt…).
// ---------------------------------------------------------------------------------------------

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: ReadonlyArray<readonly [T, string]>;
  onChange: (next: T) => void;
  label: string;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex flex-wrap gap-0.5 self-start rounded-md border border-line2 p-[3px]">
      {options.map(([v, text]) => {
        const on = v === value;
        return (
          <button
            key={v}
            type="button"
            role="radio"
            aria-checked={on}
            onClick={() => onChange(v)}
            className={
              "min-h-[34px] rounded-sm px-3 text-[12.5px] " +
              (on ? "bg-accent-soft font-semibold text-accent-text" : "text-ink2 hover:text-ink")
            }
          >
            {text}
          </button>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// Chip loc — vien tron, co so dem; `on` = nen accent.
// ---------------------------------------------------------------------------------------------

export function Chip({
  label,
  count,
  dotColor,
  squareSwatch = false,
  on = false,
  onClick,
}: {
  label: string;
  count?: number;
  /** Mau cham truoc nhan; bo trong = khong co cham. */
  dotColor?: string;
  /** `true` = o vuong (chip loai node) thay vi cham tron (chip trang thai). */
  squareSwatch?: boolean;
  on?: boolean;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={
        "inline-flex min-h-8 items-center gap-1.5 rounded-full px-3 text-[12.5px] " +
        (on ? "bg-accent font-bold text-accent-ink" : "border border-line2 text-ink2 hover:text-ink")
      }
    >
      {dotColor && !on ? (
        <span
          className={squareSwatch ? "h-2 w-2 rounded-[2px]" : "h-[7px] w-[7px] rounded-[4px]"}
          style={{ background: dotColor }}
        />
      ) : null}
      {label}
      {count === undefined ? null : <span className="opacity-70">{count}</span>}
    </button>
  );
}

// ---------------------------------------------------------------------------------------------
// Tag loai node — Video / Hình ảnh / Tạo kịch bản. Dung o bang tai khoan + API key.
// ---------------------------------------------------------------------------------------------

/** Khoa loai node — khop `nodeTypes` trong ban ban giao (backend can them truong nay). */
export type NodeTypeKey = "video" | "image" | "script";

export const NODE_TYPE_VI: Record<NodeTypeKey, string> = {
  video: "Video",
  image: "Hình ảnh",
  script: "Tạo kịch bản",
};

const NODE_TYPE_STYLE: Record<NodeTypeKey, { fg: string; bg: string; border: string }> = {
  video: { fg: "var(--tag-video)", bg: "rgb(244 63 94 / .09)", border: "rgb(244 63 94 / .35)" },
  image: { fg: "var(--tag-image)", bg: "rgb(20 184 166 / .09)", border: "rgb(20 184 166 / .35)" },
  script: { fg: "var(--tag-script)", bg: "rgb(99 102 241 / .10)", border: "rgb(99 102 241 / .38)" },
};

function NodeTypeIcon({ kind, ...rest }: { kind: NodeTypeKey } & SVGProps<SVGSVGElement>) {
  const common = {
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.8,
    strokeLinejoin: "round" as const,
    ...rest,
  };
  if (kind === "video") {
    return (
      <svg {...common}>
        <rect x="3" y="6" width="13" height="12" rx="2" />
        <path d="M16 10.5 21 8v8l-5-2.5" />
      </svg>
    );
  }
  if (kind === "image") {
    return (
      <svg {...common}>
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <circle cx="9" cy="10" r="1.8" />
        <path d="m21 16-5-5-9 9" />
      </svg>
    );
  }
  return (
    <svg {...common}>
      <path d="M6 3h9l4 4v14H5V3Z" />
      <path d="M8.5 12h7M8.5 16h5" />
    </svg>
  );
}

export function NodeTag({ kind }: { kind: NodeTypeKey }) {
  const s = NODE_TYPE_STYLE[kind];
  return (
    <span
      className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-sm border py-[3px] pl-1.5 pr-2 text-[11.5px] font-semibold"
      style={{ color: s.fg, background: s.bg, borderColor: s.border }}
    >
      <NodeTypeIcon kind={kind} className="h-3 w-3 shrink-0" />
      {NODE_TYPE_VI[kind]}
    </span>
  );
}

/** Chon nhieu loai node (dialog Thêm tài khoản / Thêm API key). */
export function NodeTagPicker({
  value,
  onChange,
}: {
  value: ReadonlySet<NodeTypeKey>;
  onChange: (next: ReadonlySet<NodeTypeKey>) => void;
}) {
  function toggle(k: NodeTypeKey): void {
    const next = new Set(value);
    if (next.has(k)) {
      next.delete(k);
    } else {
      next.add(k);
    }
    onChange(next);
  }
  return (
    <div className="flex flex-wrap gap-2">
      {(Object.keys(NODE_TYPE_VI) as NodeTypeKey[]).map((k) => {
        const on = value.has(k);
        const s = NODE_TYPE_STYLE[k];
        return (
          <label
            key={k}
            className="inline-flex min-h-10 cursor-pointer items-center gap-2 rounded-md border px-3 text-[13px]"
            style={
              on
                ? { borderColor: s.border, background: s.bg, color: s.fg, fontWeight: 600 }
                : { borderColor: "var(--line2)", color: "var(--ink2)" }
            }
          >
            <input
              type="checkbox"
              checked={on}
              onChange={() => toggle(k)}
              className="h-4 w-4 accent-[#F5A524]"
            />
            {NODE_TYPE_VI[k]}
          </label>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------------------------
// Field — nhan + o nhap, dung chung trong moi dialog.
// ---------------------------------------------------------------------------------------------

export function Field({
  label,
  required = false,
  hint,
  children,
}: {
  label: string;
  required?: boolean;
  /** Chu nho xam ben canh nhan (vd "host:port hoặc host:port:user:pass"). */
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-[12.5px] font-semibold text-ink2">
        {label}
        {required ? <span className="text-rose"> *</span> : null}
        {hint ? <span className="ml-1.5 font-normal text-ink3">{hint}</span> : null}
      </span>
      {children}
    </label>
  );
}

export const INPUT_CLASS =
  "min-h-10 w-full rounded-md border border-line2 bg-fill px-3 text-[13px] text-ink outline-none placeholder:text-ink3 focus:border-accent/60";

/** O tim kiem co icon kinh lup — dung o moi man danh sach. */
export function SearchInput({
  value,
  onChange,
  placeholder,
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder: string;
  label: string;
}) {
  return (
    // `h-10` (khong phai `min-h-10`): `flex-[1_1_280px]` dat flex-basis theo TRUC CHINH cua cha —
    // trong mot cha `flex-col` thi 280px la CHIEU CAO, o tim kiem se cao 280px. Khoa chieu cao lam
    // component nay dung duoc trong ca hang ngang lan cot doc.
    <label className="flex h-10 flex-[1_1_280px] items-center gap-2 rounded-md border border-line2 bg-fill px-3 text-ink3">
      <svg viewBox="0 0 24 24" className="h-3.5 w-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth={1.8}>
        <circle cx="10.5" cy="10.5" r="6.5" />
        <path d="M20 20l-4.3-4.3" strokeLinecap="round" />
      </svg>
      <span className="sr-only">{label}</span>
      <input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="min-w-0 flex-1 border-none bg-transparent text-[13px] text-ink outline-none placeholder:text-ink3"
      />
    </label>
  );
}

/** The chon duoc (Nền tảng, Kiểu kết nối) — vien accent + nen mo khi chon. */
export function SelectCard({
  selected,
  onClick,
  ring = false,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  /** AddAccount co them vong sang 3px quanh the dang chon. */
  ring?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onClick}
      className={
        "flex min-h-12 items-center gap-2.5 rounded-md border px-3 text-left text-ink " +
        (selected ? "border-accent/65 bg-accent-soft" : "border-line2 hover:border-line2 hover:bg-fill")
      }
      style={selected && ring ? { boxShadow: "0 0 0 3px rgb(245 165 36 / .14)" } : undefined}
    >
      {children}
    </button>
  );
}
