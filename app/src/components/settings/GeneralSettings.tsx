// Nhóm "Chung" — số luồng + giá trị mặc định cho node tạo video (chế độ/model/tỉ lệ/thời lượng).
//
// "Số luồng" áp dụng NGAY (engine đọc lại giới hạn ở lần xin permit kế tiếp, không cắt job đang
// chạy); mọi thứ khác là giá trị MẶC ĐỊNH cho node mới, workflow đã lưu không bị đổi theo.
import { allowedDurations } from "../../graph/rules";
import { THREADS_MAX, THREADS_MIN, useSettingsStore } from "../../store/settings";
import { INPUT_CLASS, Segmented, Switch } from "../../ui/primitives";
import { ModeSchema, ModelSchema, RatioSchema } from "../../ipc/types";
import type { Mode, Model, Ratio } from "../../ipc/types";
import { RowNote, SettingCard, SettingRow } from "./SettingCard";

const MODE_LABEL_VI: Record<Mode, string> = {
  fast: "Nhanh",
  pro: "Pro",
  pro_max: "Pro Max",
  agent: "Agent",
};

const MODEL_LABEL_VI: Record<Model, string> = {
  "seedance-2.0": "Seedance 2.0",
  "seedance-2.5": "Seedance 2.5",
  "seedance-1.0": "Seedance 1.0",
};

export default function GeneralSettings() {
  const draft = useSettingsStore((s) => s.draft);
  const setDraft = useSettingsStore((s) => s.setDraft);

  if (!draft) {
    return null;
  }

  const durations = allowedDurations(draft.defaultMode, draft.defaultModel);
  const threadsBad =
    !Number.isInteger(draft.threads) || draft.threads < THREADS_MIN || draft.threads > THREADS_MAX;

  /** Đổi chế độ/model có thể làm thời lượng hiện tại không còn hợp lệ — kéo về lựa chọn đầu tiên
   * còn hợp lệ thay vì để người dùng tự phát hiện qua thông báo lỗi. */
  function setMode(defaultMode: Mode): void {
    const next = allowedDurations(defaultMode, draft!.defaultModel);
    const defaultDuration = next.includes(draft!.defaultDuration) ? draft!.defaultDuration : next[0]!;
    setDraft({ defaultMode, defaultDuration });
  }

  function setModel(defaultModel: Model): void {
    const next = allowedDurations(draft!.defaultMode, defaultModel);
    const defaultDuration = next.includes(draft!.defaultDuration) ? draft!.defaultDuration : next[0]!;
    setDraft({ defaultModel, defaultDuration });
  }

  return (
    <>
      <SettingCard title="Chạy đồng thời" description="Áp dụng ngay, không cần khởi động lại">
        <SettingRow
          label="Số luồng"
          hint={`Bao nhiêu video được tạo cùng lúc. Từ ${THREADS_MIN} đến ${THREADS_MAX}.`}
          htmlFor="threads"
          last
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
      </SettingCard>

      <SettingCard title="Mặc định cho node tạo video" description="Workflow đã lưu không bị đổi theo">
        <SettingRow label="Chế độ" hint="Nhanh rẻ hơn; Pro/Pro Max cho chất lượng cao hơn.">
          <Segmented
            label="Chế độ"
            value={draft.defaultMode}
            options={ModeSchema.options.map((m) => [m, MODE_LABEL_VI[m]] as const)}
            onChange={setMode}
          />
        </SettingRow>

        <SettingRow label="Model">
          <Segmented
            label="Model"
            value={draft.defaultModel}
            options={ModelSchema.options.map((m) => [m, MODEL_LABEL_VI[m]] as const)}
            onChange={setModel}
          />
        </SettingRow>

        <SettingRow label="Tỉ lệ khung hình">
          <Segmented
            label="Tỉ lệ khung hình"
            value={draft.defaultRatio}
            options={RatioSchema.options.map((r) => [r, r] as const)}
            onChange={(defaultRatio: Ratio) => setDraft({ defaultRatio })}
          />
        </SettingRow>

        <SettingRow
          label="Thời lượng"
          hint="Danh sách thay đổi theo chế độ và model đã chọn ở trên."
          last
        >
          <Segmented
            label="Thời lượng"
            value={String(draft.defaultDuration)}
            options={durations.map((d) => [String(d), `${d}s`] as const)}
            onChange={(v) => setDraft({ defaultDuration: Number(v) })}
          />
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
