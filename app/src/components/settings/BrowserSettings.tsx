// Nhóm "Trình duyệt" — kênh chạy Dola + thư mục profile tài khoản.
//
// Proxy KHÔNG ở đây: proxy gán theo TỪNG tài khoản (tab Tài khoản → "Gán proxy"), không có proxy
// chung cho cả ứng dụng — đưa một ô proxy toàn cục vào đây sẽ gây hiểu nhầm là nó áp cho mọi phiên.
import { useState } from "react";

import { pickDirectory } from "../../ipc/commands";
import { errorText } from "../../ipc/error_text";
import { useSettingsStore } from "../../store/settings";
import { Button, INPUT_CLASS, Segmented } from "../../ui/primitives";
import type { DolaChannel } from "../../ipc/types";
import { RowNote, SettingCard, SettingRow } from "./SettingCard";

const CHANNEL_OPTIONS: ReadonlyArray<readonly [DolaChannel, string]> = [
  ["api", "API"],
  ["browser", "Trình duyệt"],
];

export default function BrowserSettings() {
  const draft = useSettingsStore((s) => s.draft);
  const setDraft = useSettingsStore((s) => s.setDraft);
  const [error, setError] = useState<string | null>(null);

  if (!draft) {
    return null;
  }

  async function pickAccountsDir(): Promise<void> {
    setError(null);
    try {
      const dir = await pickDirectory();
      if (dir !== null) {
        setDraft({ accountsDir: dir });
      }
    } catch (e) {
      setError(errorText(e));
    }
  }

  return (
    <SettingCard title="Trình duyệt" description="Có hiệu lực từ lần mở ứng dụng kế tiếp">
      <SettingRow
        label="Kênh chạy Dola"
        hint="API nhanh hơn vì không mở Chromium. Lỗi ký chữ thì tự lùi về trình duyệt."
      >
        <Segmented
          label="Kênh chạy Dola"
          value={draft.dolaChannel}
          options={CHANNEL_OPTIONS}
          onChange={(dolaChannel) => setDraft({ dolaChannel })}
        />
      </SettingRow>

      <SettingRow
        label="Thư mục tài khoản"
        hint="Nơi chứa profile Chromium của từng tài khoản (cookie, phiên đăng nhập)."
        htmlFor="accountsDir"
        last
      >
        <div className="flex flex-wrap gap-1.5">
          <input
            id="accountsDir"
            type="text"
            className={`flex-[1_1_220px] ${INPUT_CLASS} font-mono text-[12.5px]`}
            placeholder="accounts (trong thư mục dữ liệu ứng dụng)"
            value={draft.accountsDir}
            onChange={(e) => setDraft({ accountsDir: e.target.value })}
          />
          <Button onClick={() => void pickAccountsDir()}>Chọn…</Button>
        </div>
        <RowNote>
          Đổi thư mục này KHÔNG di chuyển profile đã có — các tài khoản cũ sẽ không thấy nữa cho tới
          khi bạn tự chuyển thư mục hoặc đổi lại.
        </RowNote>
        {error ? <span className="text-xs text-rose">{error}</span> : null}
      </SettingRow>
    </SettingCard>
  );
}
