// Chan thanh ben — chi con so phien ban. Ma thiet bi + nut doi theme da chuyen len thanh tren
// (TopBar / chip tai khoan), dong bo chuan LVC Fox voi app quan ly (khong lap lai o hai cho).
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useUiStore } from "../store/uiStore";

export default function SidebarAccountFooter() {
  const collapsed = useUiStore((s) => s.sidebarCollapsed);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    // Doc THAT tu Tauri; loi (vd chay ngoai Tauri khi phat trien) chi lam mat dong, khong vo thanh ben.
    void getVersion().then(setVersion).catch(() => undefined);
  }, []);

  if (collapsed) return null;
  return (
    <div className="mt-auto">
      <span className="block border-t border-sidebar-line px-2 pt-2 text-[10px] text-sidebar-muted">
        Phiên bản {version ?? "…"}
      </span>
    </div>
  );
}
