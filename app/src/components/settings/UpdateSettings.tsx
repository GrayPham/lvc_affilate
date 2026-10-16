// Nhóm "Cập nhật" — phiên bản đang chạy (đọc THẬT từ Tauri, không viết cứng) + kiểm tra thủ công.
// Dùng chung store với `UpdateBanner` (App.tsx) nên bấm ở đây cũng làm banner hiện nếu có bản mới.
import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import { useUpdaterStore } from "../../store/updater";
import { Button } from "../../ui/primitives";
import { RowNote, SettingCard, SettingRow } from "./SettingCard";

const PHASE_TEXT: Record<string, string> = {
  idle: "",
  checking: "Đang kiểm tra…",
  up_to_date: "Đã là bản mới nhất.",
  available: "Có bản mới — xem thông báo ở góc dưới bên phải.",
  downloading: "Đang tải bản cập nhật…",
  error: "Kiểm tra thất bại, thử lại.",
};

export default function UpdateSettings() {
  const [version, setVersion] = useState<string | null>(null);
  const phase = useUpdaterStore((s) => s.phase);
  const checkForUpdate = useUpdaterStore((s) => s.checkForUpdate);

  useEffect(() => {
    void getVersion().then(setVersion);
  }, []);

  const busy = phase === "checking" || phase === "downloading";

  return (
    <SettingCard title="Cập nhật" description="Tải từ kho phát hành chính thức">
      <SettingRow label="Phiên bản đang chạy" last>
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-mono text-[13px] text-fg">{version ? `v${version}` : "…"}</span>
          <Button disabled={busy} onClick={() => void checkForUpdate()}>
            Kiểm tra cập nhật
          </Button>
        </div>
        {PHASE_TEXT[phase] ? <RowNote>{PHASE_TEXT[phase]}</RowNote> : null}
      </SettingRow>
    </SettingCard>
  );
}
