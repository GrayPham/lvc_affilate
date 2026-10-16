// Thanh tieu de tu ve cho cua so KHONG VIEN (decorations:false) — chuan LVC Fox.
// Vung keo = `data-tauri-drag-region` (chi co hieu luc tren CHINH phan tu mang thuoc tinh, nut
// con khong duoc mang). Nut thu nho / phong to / dong goi API cua so Tauri v2.
import { useEffect, useState, type SVGProps } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function Icon(props: SVGProps<SVGSVGElement>) {
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" {...props} />;
}

/** Thanh keo cua so + nut dieu khien, mong, trong suot (de lo khung gradient ben duoi). */
export function WindowChrome() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!IN_TAURI) return;
    const w = getCurrentWindow();
    let alive = true;
    const sync = () => w.isMaximized().then((m) => alive && setMaximized(m)).catch(() => {});
    sync();
    const un = w.onResized(sync);
    return () => {
      alive = false;
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  const w = () => getCurrentWindow();
  const btn =
    "inline-flex h-7 w-9 items-center justify-center rounded-lg text-sidebar-muted transition-colors hover:bg-sidebar-hover hover:text-sidebar-fg";

  return (
    <header
      data-tauri-drag-region
      className="flex h-8 shrink-0 items-center gap-2.5 pr-1.5"
    >
      <span data-tauri-drag-region className="h-full min-w-4 flex-1" />
      {IN_TAURI && (
        <div className="flex items-center gap-0.5">
          <button type="button" title="Thu nhỏ" aria-label="Thu nhỏ" className={btn} onClick={() => w().minimize().catch(() => {})}>
            <Icon className="h-4 w-4"><path d="M5 12h14" /></Icon>
          </button>
          <button
            type="button"
            title={maximized ? "Khôi phục" : "Phóng to"}
            aria-label={maximized ? "Khôi phục" : "Phóng to"}
            className={btn}
            onClick={() => w().toggleMaximize().catch(() => {})}
          >
            {maximized ? (
              <Icon className="h-[13px] w-[13px]"><rect x="8" y="4" width="12" height="12" rx="1.5" /><path d="M4 8v11a1 1 0 0 0 1 1h11" /></Icon>
            ) : (
              <Icon className="h-[13px] w-[13px]"><rect x="4" y="4" width="16" height="16" rx="2" /></Icon>
            )}
          </button>
          <button
            type="button"
            title="Đóng"
            aria-label="Đóng"
            className="inline-flex h-7 w-9 items-center justify-center rounded-lg text-sidebar-muted transition-colors hover:bg-danger hover:text-white"
            onClick={() => w().close().catch(() => {})}
          >
            <Icon className="h-[17px] w-[17px]"><path d="M6 6l12 12M18 6L6 18" /></Icon>
          </button>
        </div>
      )}
    </header>
  );
}
