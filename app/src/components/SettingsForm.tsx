// Form "Cài đặt" (Task 13 brief) — bound truc tiep vao `store/settings.ts` (`draft`/`saved`/
// `warnings`/`dirty`). Xuat ham thuan `validateSettings` (tested rieng o
// `tests/settings_form.test.ts`, cung mot quy uoc voi `graph/rules.ts` cua Task 12 — mirror phia
// client CHI de UX muot, server (`AppSettings::from_map`, crates/lvc-core/src/settings.rs) van la
// nguon su that cuoi cung va tu chuan hoa + tra `warnings` rieng): chan nut "Lưu" khi ro rang sai
// (threads ngoai [`THREADS_MIN`,`THREADS_MAX`], thời lượng mặc định khong nam trong
// `allowedDurations(mode, model)` — hai dieu duy nhat CHAC CHAN server se bac, xem
// `crates/lvc-core/src/settings.rs::parse_threads`/`parse_duration`). Hai thu muc (accounts/
// output) KHONG bi bat buoc non-empty: rong la gia tri HOP LE, nghia la "de app tu chon"
// (`crates/lvc-engine/src/settings.rs::resolve_dir` — rong sau trim -> `app_data_dir/accounts`
// hoac `app_data_dir/downloads`), khong phai loi.
//
// `toAppSettings`/`THREADS_MAX`/`THREADS_MIN` chuyen sang `store/settings.ts` (review Task 13 fix
// round 1, Minor #2 — tranh store<->component gan-vong-tron).
import { useState } from "react";

import {
  ModeSchema,
  ModelSchema,
  RatioSchema,
  type AppSettings,
  type Mode,
  type Model,
  type Ratio,
} from "../ipc/types";
import { allowedDurations } from "../graph/rules";
import { pickDirectory } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { THREADS_MAX, THREADS_MIN, useSettingsStore } from "../store/settings";

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

/**
 * Loi hien thi (chan nut "Lưu") — mirror hai dieu kien server CHAC CHAN se bac (xem doc module).
 * Danh sach rong = du dieu kien de goi `settingsSet` (server van co the them canh bao khac ve cac
 * truong khong mirror o day, vd bool/mode/model/ratio go tay ngoai UI — khong the xay ra qua form
 * nay vi toan la select/checkbox).
 */
export function validateSettings(draft: AppSettings): string[] {
  const errors: string[] = [];
  if (!Number.isInteger(draft.threads) || draft.threads < THREADS_MIN || draft.threads > THREADS_MAX) {
    errors.push(`Số luồng phải là số nguyên từ ${THREADS_MIN} đến ${THREADS_MAX}.`);
  }
  const allowed = allowedDurations(draft.defaultMode, draft.defaultModel);
  if (!allowed.includes(draft.defaultDuration)) {
    errors.push(
      `Thời lượng mặc định ${draft.defaultDuration}s không hợp lệ với chế độ/model đã chọn (cho phép ${allowed.join(", ")}s).`,
    );
  }
  return errors;
}

export default function SettingsForm() {
  const draft = useSettingsStore((s) => s.draft);
  const dirty = useSettingsStore((s) => s.dirty);
  const saving = useSettingsStore((s) => s.saving);
  const loading = useSettingsStore((s) => s.loading);
  const warnings = useSettingsStore((s) => s.warnings);
  const error = useSettingsStore((s) => s.error);
  const setDraft = useSettingsStore((s) => s.setDraft);
  const save = useSettingsStore((s) => s.save);

  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const [pickError, setPickError] = useState<string | null>(null);
  const [pickingField, setPickingField] = useState<"accountsDir" | "outputDir" | null>(null);

  if (loading || !draft) {
    return <p className="p-4 text-xs text-slate-400">Đang tải cài đặt…</p>;
  }

  const errors = validateSettings(draft);
  const durations = allowedDurations(draft.defaultMode, draft.defaultModel);

  function setMode(mode: Mode): void {
    const next = allowedDurations(mode, draft!.defaultModel);
    const defaultDuration = next.includes(draft!.defaultDuration) ? draft!.defaultDuration : next[0]!;
    setDraft({ defaultMode: mode, defaultDuration });
  }

  function setModel(model: Model): void {
    const next = allowedDurations(draft!.defaultMode, model);
    const defaultDuration = next.includes(draft!.defaultDuration) ? draft!.defaultDuration : next[0]!;
    setDraft({ defaultModel: model, defaultDuration });
  }

  async function pickDir(field: "accountsDir" | "outputDir"): Promise<void> {
    setPickError(null);
    setPickingField(field);
    try {
      const dir = await pickDirectory();
      if (dir !== null) {
        if (field === "accountsDir") {
          setDraft({ accountsDir: dir });
        } else {
          setDraft({ outputDir: dir });
        }
      }
    } catch (e) {
      setPickError(errorText(e));
    } finally {
      setPickingField(null);
    }
  }

  async function handleSave(): Promise<void> {
    setSavedMessage(null);
    const ok = await save();
    if (ok) {
      setSavedMessage("Đã lưu cài đặt.");
    }
  }

  return (
    <div className="flex max-w-xl flex-col gap-4 p-4">
      <section className="flex flex-col gap-2">
        <label className="flex items-center justify-between gap-2 text-sm">
          <span>
            Số luồng đồng thời
            <span className="ml-1 text-[10px] text-emerald-600">(áp dụng ngay)</span>
          </span>
          <input
            type="number"
            min={THREADS_MIN}
            max={THREADS_MAX}
            className="w-24 rounded border border-slate-300 px-2 py-1 text-right text-sm"
            value={draft.threads}
            onChange={(e) => setDraft({ threads: Number(e.target.value) })}
          />
        </label>

        <label className="flex items-center justify-between gap-2 text-sm">
          <span>Chế độ im lặng (silent mode)</span>
          <input
            type="checkbox"
            checked={draft.silentMode}
            onChange={(e) => setDraft({ silentMode: e.target.checked })}
          />
        </label>

        <label className="flex items-center justify-between gap-2 text-sm">
          <span>
            Tự xoay chế độ khi hết lượt (mode fallback)
            <span className="ml-1 text-[10px] text-amber-600">(cần khởi động lại)</span>
          </span>
          <input
            type="checkbox"
            checked={draft.modeFallback}
            onChange={(e) => setDraft({ modeFallback: e.target.checked })}
          />
        </label>
      </section>

      <section className="flex flex-col gap-2">
        <div>
          <p className="text-sm">
            Thư mục tài khoản
            <span className="ml-1 text-[10px] text-amber-600">(cần khởi động lại)</span>
          </p>
          <div className="flex items-center gap-2">
            <input
              type="text"
              className="flex-1 rounded border border-slate-300 px-2 py-1 text-xs"
              placeholder="(mặc định trong thư mục dữ liệu ứng dụng)"
              value={draft.accountsDir}
              onChange={(e) => setDraft({ accountsDir: e.target.value })}
            />
            <button
              type="button"
              disabled={pickingField === "accountsDir"}
              className="shrink-0 rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
              onClick={() => void pickDir("accountsDir")}
            >
              Chọn…
            </button>
          </div>
        </div>

        <div>
          <p className="text-sm">
            Thư mục đầu ra
            <span className="ml-1 text-[10px] text-amber-600">(cần khởi động lại)</span>
          </p>
          <div className="flex items-center gap-2">
            <input
              type="text"
              className="flex-1 rounded border border-slate-300 px-2 py-1 text-xs"
              placeholder="downloads (mặc định)"
              value={draft.outputDir}
              onChange={(e) => setDraft({ outputDir: e.target.value })}
            />
            <button
              type="button"
              disabled={pickingField === "outputDir"}
              className="shrink-0 rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
              onClick={() => void pickDir("outputDir")}
            >
              Chọn…
            </button>
          </div>
        </div>
        {pickError ? <p className="text-xs text-red-600">{pickError}</p> : null}
      </section>

      <section className="grid grid-cols-2 gap-2">
        <label className="flex flex-col gap-1 text-xs text-slate-600">
          Chế độ mặc định
          <select
            className="rounded border border-slate-300 px-2 py-1 text-sm"
            value={draft.defaultMode}
            onChange={(e) => setMode(e.target.value as Mode)}
          >
            {ModeSchema.options.map((m) => (
              <option key={m} value={m}>
                {MODE_LABEL_VI[m]}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-xs text-slate-600">
          Model mặc định
          <select
            className="rounded border border-slate-300 px-2 py-1 text-sm"
            value={draft.defaultModel}
            onChange={(e) => setModel(e.target.value as Model)}
          >
            {ModelSchema.options.map((m) => (
              <option key={m} value={m}>
                {MODEL_LABEL_VI[m]}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-xs text-slate-600">
          Tỉ lệ mặc định
          <select
            className="rounded border border-slate-300 px-2 py-1 text-sm"
            value={draft.defaultRatio}
            onChange={(e) => setDraft({ defaultRatio: e.target.value as Ratio })}
          >
            {RatioSchema.options.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
        </label>

        <label className="flex flex-col gap-1 text-xs text-slate-600">
          Thời lượng mặc định
          <select
            className="rounded border border-slate-300 px-2 py-1 text-sm"
            value={draft.defaultDuration}
            onChange={(e) => setDraft({ defaultDuration: Number(e.target.value) })}
          >
            {durations.map((d) => (
              <option key={d} value={d}>
                {d}s
              </option>
            ))}
          </select>
        </label>
      </section>

      {errors.length > 0 ? (
        <ul className="rounded border border-red-200 bg-red-50 p-2 text-xs text-red-700">
          {errors.map((e) => (
            <li key={e}>{e}</li>
          ))}
        </ul>
      ) : null}

      {warnings.length > 0 ? (
        <ul className="rounded border border-amber-200 bg-amber-50 p-2 text-xs text-amber-800">
          {warnings.map((w) => (
            <li key={w}>{w}</li>
          ))}
        </ul>
      ) : null}

      {error ? <p className="text-xs text-red-600">{error}</p> : null}
      {savedMessage ? <p className="text-xs text-emerald-700">{savedMessage}</p> : null}

      <div>
        <button
          type="button"
          disabled={!dirty || saving || errors.length > 0}
          className="rounded bg-slate-900 px-4 py-1.5 text-sm text-white hover:bg-slate-700 disabled:opacity-50"
          onClick={() => void handleSave()}
        >
          {saving ? "Đang lưu…" : "Lưu"}
        </button>
      </div>
    </div>
  );
}
