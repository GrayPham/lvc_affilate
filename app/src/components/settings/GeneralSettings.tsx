// Nhóm "Chung" — số luồng chạy đồng thời + số node song song + tài khoản (các giá trị "mặc định
// cho node tạo video" kiểu Dola đã bỏ: giờ nhiều loại node, không riêng Dola — mỗi node tự cấu
// hình trên canvas).
//
// "Số luồng"/"Số node song song" áp dụng NGAY; "Tài khoản/đầu ra" có hiệu lực từ lần mở app kế tiếp.
import { THREADS_MAX, THREADS_MIN, useSettingsStore } from "../../store/settings";
import { INPUT_CLASS, Switch } from "../../ui/primitives";
import { RowNote, SettingCard, SettingRow } from "./SettingCard";

export default function GeneralSettings() {
  const draft = useSettingsStore((s) => s.draft);
  const setDraft = useSettingsStore((s) => s.setDraft);

  if (!draft) {
    return null;
  }

  const threadsBad =
    !Number.isInteger(draft.threads) || draft.threads < THREADS_MIN || draft.threads > THREADS_MAX;
  const nodeThreadsBad =
    !Number.isInteger(draft.nodeThreads) ||
    draft.nodeThreads < THREADS_MIN ||
    draft.nodeThreads > THREADS_MAX;

  return (
    <>
      <SettingCard title="Chạy đồng thời" description="Áp dụng ngay, không cần khởi động lại">
        <SettingRow
          label="Số luồng"
          hint={`Bao nhiêu video được tạo cùng lúc. Từ ${THREADS_MIN} đến ${THREADS_MAX}.`}
          htmlFor="threads"
        >
          <input
            id="threads"
            type="number"
            min={THREADS_MIN}
            max={THREADS_MAX}
            className={`w-28 ${INPUT_CLASS} text-right font-mono`}
            value={draft.threads}
            onChange={(e) => setDraft({ threads: Number(e.target.value) })}
          />
          {threadsBad ? (
            <span className="text-xs text-rose">
              Phải là số nguyên từ {THREADS_MIN} đến {THREADS_MAX}.
            </span>
          ) : (
            <RowNote>
              Job đang chạy không bị cắt — giới hạn mới có hiệu lực từ job kế tiếp.
            </RowNote>
          )}
        </SettingRow>
        <SettingRow
          label="Số node chạy song song"
          hint={`Bao nhiêu node đồ thị chạy cùng lúc (1 = tuần tự). Từ ${THREADS_MIN} đến ${THREADS_MAX}.`}
          htmlFor="nodeThreads"
          last
        >
          <input
            id="nodeThreads"
            type="number"
            min={THREADS_MIN}
            max={THREADS_MAX}
            className={`w-28 ${INPUT_CLASS} text-right font-mono`}
            value={draft.nodeThreads}
            onChange={(e) => setDraft({ nodeThreads: Number(e.target.value) })}
          />
          {nodeThreadsBad ? (
            <span className="text-xs text-rose">
              Phải là số nguyên từ {THREADS_MIN} đến {THREADS_MAX}.
            </span>
          ) : (
            <RowNote>
              MuseAI 1 tài khoản: mở phiên nối tiếp, nhưng tạo video chồng nhau nên vẫn nhanh hơn.
            </RowNote>
          )}
        </SettingRow>
      </SettingCard>

      <SettingCard title="Tài khoản" description="Có hiệu lực từ lần mở ứng dụng kế tiếp">
        <SettingRow
          label="Tự xoay chế độ khi hết lượt"
          hint="Hết lượt chế độ đang dùng thì tự chuyển sang chế độ còn lượt thay vì báo lỗi."
          last
        >
          <Switch
            checked={draft.modeFallback}
            label="Tự xoay chế độ khi hết lượt"
            onChange={(modeFallback) => setDraft({ modeFallback })}
          />
        </SettingRow>
      </SettingCard>
    </>
  );
}
