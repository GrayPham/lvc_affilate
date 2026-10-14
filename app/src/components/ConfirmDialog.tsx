// Hop xac nhan dung chung — dung theo `references/lvc-ui-design/screens/Confirm.dc.html`.
// MOT component cho ca 5 tinh huong (xoa tai khoan, xoa key, dung hang cho, don thu muc tam, bo
// khoi hang cho): khac nhau o NOI DUNG + muc do nguy hiem, khong phai o hinh dang.
import type { ReactNode } from "react";
import { createPortal } from "react-dom";

import { Button } from "../ui/primitives";

/** `danger` = hanh dong mat du lieu (nut do); `caution` = hanh dong dung/hoan tac duoc (nut vang). */
export type ConfirmSeverity = "danger" | "caution";

export interface ConfirmListItem {
  /** Cot trai, dam (vd ten tai khoan). */
  a: string;
  /** Cot phai, mo (vd "Dola · Video, Hình ảnh"). */
  b: string;
}

export interface ConfirmDialogProps {
  severity?: ConfirmSeverity;
  title: string;
  body: string;
  /** Danh sach muc bi anh huong — bo trong = khong hien. */
  list?: ConfirmListItem[];
  /** Dong nho cuoi, giai thich he qua — bo trong = khong hien. */
  note?: string;
  /** Lua chon kieu radio (vd "Dừng sau node hiện tại") — bo trong = khong hien. */
  options?: ReadonlyArray<{ label: string; value: string }>;
  optionValue?: string;
  onOptionChange?: (v: string) => void;
  cancelLabel?: string;
  okLabel: string;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  /** Noi dung them (hiem khi can) — chen duoi `note`. */
  children?: ReactNode;
}

export default function ConfirmDialog({
  severity = "danger",
  title,
  body,
  list = [],
  note,
  options = [],
  optionValue,
  onOptionChange,
  cancelLabel = "Huỷ",
  okLabel,
  busy = false,
  onCancel,
  onConfirm,
  children,
}: ConfirmDialogProps) {
  const danger = severity === "danger";
  return createPortal(
    <div
      className="fixed inset-0 z-[60] flex items-start justify-center overflow-y-auto bg-black/55 p-8"
      onClick={onCancel}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        style={{ boxShadow: "var(--dialog-shadow)" }}
        className="w-[480px] max-w-full overflow-hidden rounded-xl border border-line2 bg-panel text-ink"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex gap-3.5 px-5 pb-4 pt-5">
          <span
            className="grid h-10 w-10 shrink-0 place-items-center rounded-[11px]"
            style={{
              background: danger ? "rgb(251 113 133 / .14)" : "rgb(245 165 36 / .15)",
              color: danger ? "var(--rose)" : "var(--accent)",
            }}
          >
            <svg viewBox="0 0 24 24" className="h-5 w-5" fill="none" stroke="currentColor" strokeWidth={2} strokeLinejoin="round">
              <path d="M12 3 2 20h20L12 3Z" />
              <path d="M12 10v4M12 17v.5" strokeLinecap="round" />
            </svg>
          </span>

          <div className="flex min-w-0 flex-1 flex-col gap-2">
            <h2 className="text-[16px] font-bold leading-[1.35]">{title}</h2>
            <p className="text-[13px] leading-[1.55] text-ink2">{body}</p>

            {list.length > 0 ? (
              <ul className="mt-0.5 flex flex-col gap-1">
                {list.map((it) => (
                  <li
                    key={it.a}
                    className="flex items-center gap-2 rounded-sm bg-fill px-2.5 py-[7px] text-[12.5px]"
                  >
                    <span className="font-semibold">{it.a}</span>
                    <span className="text-ink3">{it.b}</span>
                  </li>
                ))}
              </ul>
            ) : null}

            {options.length > 0 ? (
              <div role="radiogroup" aria-label={title} className="mt-0.5 flex flex-col gap-1.5">
                {options.map((o) => {
                  const on = o.value === optionValue;
                  return (
                    <label
                      key={o.value}
                      className="flex min-h-10 cursor-pointer items-center gap-2.5 rounded-md border px-3 text-[13px] text-ink"
                      style={{ borderColor: on ? "rgb(245 165 36 / .6)" : "var(--line2)" }}
                    >
                      <input
                        type="radio"
                        checked={on}
                        onChange={() => onOptionChange?.(o.value)}
                        className="h-4 w-4 accent-[#F5A524]"
                      />
                      {o.label}
                    </label>
                  );
                })}
              </div>
            ) : null}

            {note ? <p className="mt-0.5 text-[12px] leading-[1.5] text-ink3">{note}</p> : null}
            {children}
          </div>
        </div>

        <div className="flex justify-end gap-2 border-t border-line bg-fill px-5 py-3.5">
          <Button onClick={onCancel} disabled={busy}>
            {cancelLabel}
          </Button>
          <Button
            variant="primary"
            disabled={busy}
            onClick={onConfirm}
            className={danger ? "!bg-rose !text-rose-ink" : ""}
          >
            {busy ? "Đang xử lý…" : okLabel}
          </Button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
