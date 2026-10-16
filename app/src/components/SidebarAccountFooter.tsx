// Chan thanh ben — ma thiet bi, nut doi theme, so phien ban.
//
// Ban ban giao `references/lvc-ui-design/screens/Sidebar.dc.html` con co the "Gói sử dụng"
// (Free/Unlimited), bo dem "1/5 thiết bị" va nut "Upgrade". KHONG dung o day: app chua co he thong
// license nao (khong server, khong ma kich hoat, khong dang ky thiet bi), nen moi so lieu do se la
// so BIA va nut Upgrade se bam khong chay.
//
// Phan GIU LAI la phan co that: ma thiet bi (`commands::device`) — nguoi dung gui kem khi bao loi,
// va la cho moc san cho license sau nay. Khi co co che license that thi dung tiep the "Gói" theo
// dung ban thiet ke do.
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import { deviceId } from "../ipc/commands";
import { maskDeviceId } from "../format/device";
import { useUiStore } from "../store/uiStore";

export default function SidebarAccountFooter() {
  const theme = useUiStore((s) => s.theme);
  const toggleTheme = useUiStore((s) => s.toggleTheme);
  const collapsed = useUiStore((s) => s.sidebarCollapsed);

  const [version, setVersion] = useState<string | null>(null);
  const [device, setDevice] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    // Hai thu nay doc mot lan roi khong doi nua trong ca phien. Loi (vd chay ngoai Tauri khi
    // phat trien) chi lam mat dong hien thi, khong duoc lam vo thanh ben.
    void getVersion().then(setVersion).catch(() => undefined);
    void deviceId().then(setDevice).catch(() => undefined);
  }, []);

  async function copyDevice(): Promise<void> {
    if (device === null) {
      return;
    }
    try {
      await navigator.clipboard.writeText(device);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // Khong chep duoc (quyen clipboard) — khong bao loi do, nguoi dung van doc duoc ma day du
      // qua tooltip.
    }
  }

  return (
    <div className="mt-auto flex flex-col gap-2.5">
      {/* Ma thiet bi — an khi thu gon thanh ben (khong du cho de doc). */}
      {collapsed || device === null ? null : (
        <div className="flex flex-col gap-1 rounded-md border border-sidebar-line bg-sidebar-hover px-2.5 py-2">
          <span className="text-[10px] font-semibold tracking-[.1em] text-sidebar-muted uppercase">
            Mã thiết bị
          </span>
          <div className="flex items-center gap-1.5">
            <span
              title={device}
              className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-sidebar-muted"
            >
              {maskDeviceId(device)}
            </span>
            <button
              type="button"
              onClick={() => void copyDevice()}
              title="Sao chép mã đầy đủ"
              aria-label="Sao chép mã thiết bị"
              className="shrink-0 rounded-sm px-1.5 py-0.5 text-[10px] text-sidebar-muted hover:bg-sidebar-hover hover:text-sidebar-fg"
            >
              {copied ? "Đã chép" : "Chép"}
            </button>
          </div>
        </div>
      )}

      {/* Đổi theme — bản bàn giao có cả hai giao diện Tối/Sáng. */}
      <button
        type="button"
        onClick={toggleTheme}
        title={collapsed ? (theme === "dark" ? "Giao diện sáng" : "Giao diện tối") : undefined}
        className={
          "flex min-h-[36px] items-center rounded-md text-[12.5px] text-sidebar-muted hover:bg-sidebar-hover hover:text-sidebar-fg " +
          (collapsed ? "justify-center px-0" : "gap-2.5 px-2")
        }
      >
        {theme === "dark" ? (
          <svg viewBox="0 0 24 24" className="h-[17px] w-[17px] shrink-0" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
            <path d="M20 14.5A8.5 8.5 0 0 1 9.5 4a8.5 8.5 0 1 0 10.5 10.5Z" />
          </svg>
        ) : (
          <svg viewBox="0 0 24 24" className="h-[17px] w-[17px] shrink-0" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round">
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
          </svg>
        )}
        {collapsed ? null : theme === "dark" ? "Giao diện sáng" : "Giao diện tối"}
      </button>

      {collapsed ? null : (
        <span className="border-t border-sidebar-line px-2 pt-2 text-[10px] text-sidebar-muted">
          {/* Doc THAT tu Tauri — truoc day viet cung "1.0.0", tang phien ban la dong nay noi doi. */}
          Phiên bản {version ?? "…"}
        </span>
      )}
    </div>
  );
}
