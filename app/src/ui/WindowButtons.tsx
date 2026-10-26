// Bộ nút cửa sổ TỰ VẼ (thu nhỏ / phóng to / đóng) — cửa sổ đặt `decorations: false` nên Windows
// KHÔNG vẽ nút nào, app phải tự lo. Tách khỏi `TopBar` để màn khoá bản quyền (chưa render TopBar)
// dùng lại ĐÚNG bộ nút đó: hai màn phải giống hệt nhau, không được mỗi nơi một kiểu.
//
// Ngoài Tauri (xem thử bằng trình duyệt) thì không render gì — trình duyệt đã có nút cửa sổ riêng.
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";

import { cx } from "./primitives";

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function WindowButtons() {
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
  if (!IN_TAURI) return null;
  const w = () => getCurrentWindow();
  const btn = "inline-flex h-8 w-9 items-center justify-center rounded-lg text-muted transition-colors hover:bg-hover hover:text-fg";
  return (
    <div className="ml-1 flex items-center gap-0.5">
      <button type="button" title="Thu nhỏ" aria-label="Thu nhỏ" className={btn} onClick={() => w().minimize().catch(() => {})}>
        <Minus size={16} />
      </button>
      <button type="button" title={maximized ? "Khôi phục" : "Phóng to"} aria-label={maximized ? "Khôi phục" : "Phóng to"} className={btn} onClick={() => w().toggleMaximize().catch(() => {})}>
        <Square size={13} />
      </button>
      <button type="button" title="Đóng" aria-label="Đóng" className={cx(btn, "hover:bg-danger hover:text-white")} onClick={() => w().close().catch(() => {})}>
        <X size={17} />
      </button>
    </div>
  );
}
