// Khung modal dung chung (overlay + the trang) — thay cac modal tu dung lap lai cung mot pattern
// `fixed inset-0 z-50 flex items-center justify-center bg-black/30` (vd `ImportTxtModal` cu).
// THUAN hien thi: khong tu quan ly trang thai mo/dong, nhan `onClose` tu component cha.
import type { ReactNode } from "react";

export default function Dialog({
  title,
  onClose,
  width = "32rem",
  children,
}: {
  title: string;
  onClose: () => void;
  /** Be rong toi da (CSS), mac dinh 32rem — dialog phuc tap hon dung "40rem"/"44rem". */
  width?: string;
  children: ReactNode;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4"
      onClick={onClose}
    >
      <div
        className="flex max-h-[85vh] w-full flex-col rounded-xl border border-white/10 bg-neutral-900 shadow-2xl"
        style={{ maxWidth: width }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-white/10 px-4 py-3">
          <h2 className="text-sm font-semibold text-white">{title}</h2>
          <button
            type="button"
            className="rounded-md p-1 text-neutral-500 hover:bg-white/10 hover:text-neutral-200"
            onClick={onClose}
            aria-label="Đóng"
          >
            <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
              <path d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </div>
        <div className="flex-1 overflow-auto p-4">{children}</div>
      </div>
    </div>
  );
}
