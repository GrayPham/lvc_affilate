// Thanh tren (cung la thanh tieu de — cua so khong vien): o tim, doi sang/toi, cai dat, chuong thong
// bao, chip tai khoan (goi + ma thiet bi), nut cua so. Chuan LVC Fox (dong bo apps/manager Topbar).
// Vung keo `data-tauri-drag-region` chi co hieu luc tren CHINH phan tu mang thuoc tinh; nut thi khong.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Bell, ChevronDown, Copy, KeyRound, Moon, Search, Settings as SettingsIcon, Sun, X } from "lucide-react";
import { useUiStore } from "../store/uiStore";
import { deviceId } from "../ipc/commands";
import { maskDeviceId } from "../format/device";
import { getVersion } from "@tauri-apps/api/app";
import { BTN_ICON_ROUND } from "./buttonStyles";
import { cx } from "./primitives";
import { WindowButtons } from "./WindowButtons";

const ACCOUNT_NAME = "LVC Shop";
const ROUND = BTN_ICON_ROUND;

/** Menu tha xuong neo duoi mot nut — dong khi bam ra ngoai / Esc. */
function Dropdown({ open, onClose, children, width = 290 }: { open: boolean; onClose: () => void; children: ReactNode; width?: number }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div
      ref={ref}
      style={{ width }}
      className="absolute right-0 top-[calc(100%+6px)] z-50 overflow-hidden rounded-2xl border border-line bg-panel p-1.5 shadow-panel"
    >
      {children}
    </div>
  );
}
export function TopBar() {
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);
  const activeTab = useUiStore((s) => s.activeTab);
  const setActiveTab = useUiStore((s) => s.setActiveTab);
  const [search, setSearch] = useState("");
  const [bell, setBell] = useState(false);
  const [account, setAccount] = useState(false);
  const [device, setDevice] = useState<string | null>(null);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    void deviceId().then(setDevice).catch(() => undefined);
    void getVersion().then(setVersion).catch(() => undefined);
  }, []);

  const copyDevice = () => {
    if (device) void navigator.clipboard.writeText(device).catch(() => undefined);
  };

  return (
    <header data-tauri-drag-region className="flex h-[68px] shrink-0 items-center gap-2.5 pl-5 pr-3">
      {/* O tim — placeholder cho tung tool tu noi (vd tim workflow/kich ban). */}
      <div className="relative min-w-[220px] max-w-[560px] flex-[1_1_560px]">
        <Search size={16} className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted" />
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && setSearch("")}
          placeholder="Tìm workflow, kịch bản, tài khoản…"
          spellCheck={false}
          className="h-11 w-full rounded-full border border-line bg-panel pl-11 pr-10 text-[13.5px] text-fg shadow-card placeholder:text-muted focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20"
        />
        {search && (
          <button type="button" onClick={() => setSearch("")} title="Xoá tìm kiếm" className="absolute right-3.5 top-1/2 -translate-y-1/2 rounded-full p-0.5 text-muted hover:text-fg">
            <X size={14} />
          </button>
        )}
      </div>

      <div data-tauri-drag-region className="h-full min-w-4 flex-1" />

      <button type="button" title="Đổi sáng/tối" aria-label="Đổi sáng/tối" className={ROUND} onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
        {theme === "dark" ? <Moon size={16} /> : <Sun size={16} />}
      </button>
      <button type="button" title="Cài đặt" aria-label="Cài đặt" className={cx(ROUND, activeTab === "settings" && "text-accent!")} onClick={() => setActiveTab("settings")}>
        <SettingsIcon size={16} />
      </button>

      <div className="relative">
        <button type="button" title="Thông báo" aria-label="Thông báo" className={ROUND} onClick={() => { setBell((v) => !v); setAccount(false); }}>
          <Bell size={16} />
        </button>
        <Dropdown open={bell} onClose={() => setBell(false)} width={340}>
          <div className="px-2.5 pb-1.5 pt-1 text-[13px] font-semibold">Thông báo</div>
          <div className="px-2.5 py-6 text-center text-[12.5px] text-muted">Chưa có thông báo nào.</div>
        </Dropdown>
      </div>

      <div className="relative">
        <button
          type="button"
          onClick={() => { setAccount((v) => !v); setBell(false); }}
          className="ml-1 inline-flex h-11 shrink-0 items-center gap-2 rounded-full border border-line bg-panel py-1 pl-1 pr-3 shadow-card transition-colors hover:bg-hover"
        >
          <span className="lvc-btn-primary inline-flex h-9 w-9 items-center justify-center rounded-full text-[12.5px] font-bold">LV</span>
          <span className="max-w-[130px] truncate text-[13px] font-semibold">{ACCOUNT_NAME}</span>
          <ChevronDown size={15} className="text-muted" />
        </button>
        <Dropdown open={account} onClose={() => setAccount(false)}>
          <div className="mb-2 flex items-center gap-2.5 px-1 pt-1">
            <span className="lvc-btn-primary inline-flex h-10 w-10 items-center justify-center rounded-full text-[13px] font-bold">LV</span>
            <div className="min-w-0">
              <div className="truncate text-[13.5px] font-semibold">{ACCOUNT_NAME}</div>
              <div className="flex items-center gap-1 text-[11.5px] text-muted">
                <KeyRound size={11} className="text-brand" /> Bản phát triển
              </div>
            </div>
          </div>
          <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 rounded-lg bg-panel-2 px-2.5 py-2 text-[12px]">
            <dt className="text-muted">Gói</dt>
            <dd className="truncate text-right">Bản phát triển</dd>
            <dt className="text-muted">Phiên bản</dt>
            <dd className="text-right tabular-nums">{version ?? "…"}</dd>
            <dt className="text-muted">Mã thiết bị</dt>
            <dd className="flex min-w-0 items-center justify-end gap-1">
              <span className="truncate font-mono text-[11px]" title={device ?? undefined}>{device ? maskDeviceId(device) : "…"}</span>
              <button type="button" title="Chép" disabled={!device} onClick={copyDevice} className="shrink-0 rounded p-0.5 text-muted hover:bg-hover hover:text-fg">
                <Copy size={12} />
              </button>
            </dd>
          </dl>
          <p className="mt-2 px-1 text-[11px] leading-4 text-muted">Mã thiết bị là định danh một lần cài đặt — gửi kèm khi báo lỗi để được hỗ trợ.</p>
        </Dropdown>
      </div>

      <WindowButtons />
    </header>
  );
}
