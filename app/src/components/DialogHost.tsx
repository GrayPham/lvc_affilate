// Giao dien dialog trong-app (P-lumina) — mount MOT LAN o App. Doc dialog dang mo tu `useDialogStore`,
// ve modal toi (confirm/prompt/alert), resolve promise khi nguoi dung chon. Esc = huy, Enter = OK.
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { useDialogStore, type DialogReq } from "../store/dialog";

export default function DialogHost() {
  const current = useDialogStore((s) => s.current);
  const close = useDialogStore((s) => s.close);
  return current ? <DialogView key={dialogKey(current)} req={current} onDone={close} /> : null;
}

function dialogKey(req: DialogReq): string {
  return `${req.kind}:${"message" in req ? req.message : ""}:${req.title ?? ""}`;
}

function DialogView({ req, onDone }: { req: DialogReq; onDone: () => void }) {
  const [value, setValue] = useState(req.kind === "prompt" ? (req.defaultValue ?? "") : "");
  const inputRef = useRef<HTMLInputElement>(null);
  const areaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (req.kind === "prompt") {
      const el = req.multiline ? areaRef.current : inputRef.current;
      el?.focus();
      el?.select?.();
    }
  }, [req]);

  function cancel(): void {
    if (req.kind === "confirm") {
      req.resolve(false);
    } else if (req.kind === "prompt") {
      req.resolve(null);
    } else {
      req.resolve();
    }
    onDone();
  }

  function ok(): void {
    if (req.kind === "confirm") {
      req.resolve(true);
    } else if (req.kind === "prompt") {
      req.resolve(value);
    } else {
      req.resolve();
    }
    onDone();
  }

  const danger = (req.kind === "confirm" || req.kind === "alert") && req.danger;

  return createPortal(
    <div
      className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 p-4"
      onClick={cancel}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          cancel();
        }
      }}
      role="presentation"
    >
      <div
        className="w-full max-w-sm rounded-2xl border border-line bg-[#17171a] p-4 text-ink shadow-2xl"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        {req.title ? <h2 className="mb-1 text-sm font-semibold">{req.title}</h2> : null}
        {"message" in req && req.message ? (
          <p className="mb-3 whitespace-pre-line text-[13px] leading-relaxed text-ink2">{req.message}</p>
        ) : null}

        {req.kind === "prompt" ? (
          req.multiline ? (
            <textarea
              ref={areaRef}
              className="mb-3 min-h-[88px] w-full resize-y rounded-md border border-line bg-fill px-2.5 py-1.5 text-sm outline-none focus:border-sky-400/60"
              value={value}
              placeholder={req.placeholder}
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                  ok();
                }
              }}
            />
          ) : (
            <input
              ref={inputRef}
              className="mb-3 w-full rounded-md border border-line bg-fill px-2.5 py-1.5 text-sm outline-none focus:border-sky-400/60"
              value={value}
              placeholder={req.placeholder}
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  ok();
                }
              }}
            />
          )
        ) : null}

        <div className="flex justify-end gap-2">
          {req.kind !== "alert" ? (
            <button
              type="button"
              className="rounded-md px-3 py-1.5 text-sm text-ink2 hover:bg-fill2"
              onClick={cancel}
            >
              {req.kind === "confirm" ? (req.cancelText ?? "Huỷ") : (req.cancelText ?? "Huỷ")}
            </button>
          ) : null}
          <button
            type="button"
            autoFocus={req.kind !== "prompt"}
            className={
              "rounded-md px-3 py-1.5 text-sm font-medium text-ink " +
              (danger ? "bg-red-600 hover:bg-red-500" : "bg-sky-600 hover:bg-sky-500")
            }
            onClick={ok}
          >
            {req.kind === "alert"
              ? (req.okText ?? "OK")
              : req.kind === "confirm"
                ? (req.okText ?? "Đồng ý")
                : (req.okText ?? "OK")}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
