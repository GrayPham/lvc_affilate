// Nhóm "Chạy tác vụ" — hàng chờ ở tab Tác vụ xử sự thế nào. Ba cài đặt này được bộ chạy đọc LẠI
// trước mỗi workflow (`crates/lvc-app/src/commands/queue.rs::spawn_runner`) nên đổi giữa lúc đang
// chạy vẫn có tác dụng cho mục kế tiếp.
import { useSettingsStore } from "../../store/settings";
import { Segmented } from "../../ui/primitives";
import type { MissingAccountPolicy, OnErrorPolicy } from "../../ipc/types";
import { RowNote, SettingCard, SettingRow } from "./SettingCard";

const ON_ERROR_OPTIONS: ReadonlyArray<readonly [OnErrorPolicy, string]> = [
  ["skip", "Bỏ qua, chạy tiếp"],
  ["stop", "Dừng cả hàng chờ"],
];

/** Khớp `lvc_core::settings::run::REST_CHOICES` — giá trị ngoài danh sách này bị backend bác. */
const REST_OPTIONS: ReadonlyArray<readonly [string, string]> = [
  ["0", "Không nghỉ"],
  ["30", "30 giây"],
  ["60", "1 phút"],
  ["180", "3 phút"],
];

const MISSING_OPTIONS: ReadonlyArray<readonly [MissingAccountPolicy, string]> = [
  ["skip", "Bỏ qua, chạy tiếp"],
  ["wait", "Chờ tối đa 30 phút"],
];

export default function RunSettings() {
  const draft = useSettingsStore((s) => s.draft);
  const setDraft = useSettingsStore((s) => s.setDraft);

  if (!draft) {
    return null;
  }

  return (
    <SettingCard title="Chạy tác vụ" description="Áp dụng cho hàng chờ ở tab Tác vụ">
      <SettingRow label="Khi một workflow lỗi" hint="Lỗi ghi rõ node và lý do trong hàng chờ.">
        <Segmented
          label="Khi một workflow lỗi"
          value={draft.onError}
          options={ON_ERROR_OPTIONS}
          onChange={(onError) => setDraft({ onError })}
        />
        {draft.onError === "stop" ? (
          <RowNote>
            Các mục sau vẫn ở trạng thái “Đang chờ” — xem lại lỗi rồi bấm Bắt đầu để chạy tiếp.
          </RowNote>
        ) : null}
      </SettingRow>

      <SettingRow label="Nghỉ giữa hai workflow" hint="Giảm rủi ro bị gắn cờ khi chạy liên tục.">
        <Segmented
          label="Nghỉ giữa hai workflow"
          value={String(draft.restBetween)}
          options={REST_OPTIONS}
          onChange={(v) => setDraft({ restBetween: Number(v) })}
        />
      </SettingRow>

      <SettingRow
        label="Thiếu tài khoản khi tới lượt"
        hint="Áp dụng cho node AI chưa có tài khoản dùng được."
        last
      >
        <Segmented
          label="Thiếu tài khoản khi tới lượt"
          value={draft.missingAccount}
          options={MISSING_OPTIONS}
          onChange={(missingAccount) => setDraft({ missingAccount })}
        />
      </SettingRow>
    </SettingCard>
  );
}
