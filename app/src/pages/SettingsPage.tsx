// Man "Cài đặt" (Task 13): form `SettingsForm` (bound vao `store/settings.ts`) + khu vuc
// `LegacyImport` ("Nhập dữ liệu từ bản Python"). Chi viec goi `load()` luc mount — moi logic doc/
// ghi/validate nam trong store + component con.
import { useEffect } from "react";

import { useSettingsStore } from "../store/settings";
import SettingsForm from "../components/SettingsForm";
import LegacyImport from "../components/LegacyImport";

export default function SettingsPage() {
  const load = useSettingsStore((s) => s.load);

  useEffect(() => {
    void load();
    // `load` la action zustand (tham chieu on dinh) — chi can chay dung mot lan luc mount trang.
  }, []);

  return (
    <div className="h-full overflow-auto">
      <SettingsForm />
      <div className="max-w-xl px-4 pb-4">
        <LegacyImport />
      </div>
    </div>
  );
}
