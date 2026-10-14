// Bang tham so DUOI (P-lumina) — hien khi mot node `dola_video` duoc chon: dropdown model, ti le
// (aspect), nhan do phan giai "1080p", thoi luong, link "Tùy chọn nâng cao" (mo che do/logo/tai
// khoan), va so luong "1×". Ngoai ra hien CAC CHIP THAM CHIEU: cac node noi vao cong `refs` cua
// node nay, moi chip co "×" de xoa canh. Moi chinh sua luu qua `updateNodeData` cua store — DUNG
// mo hinh du lieu cu, chi doi noi hien thi (than node gio gon lai, xem `nodes/VideoNode.tsx`).
import { useState } from "react";

import {
  LogoModeSchema,
  ModeSchema,
  ModelSchema,
  RatioSchema,
  type LogoMode,
  type Mode,
  type Model,
  type Ratio,
} from "../ipc/types";
import { allowedDurations } from "../graph/rules";
import { referenceChips } from "../graph/refs";
import { useGraphStore } from "../store/graph";
import PromptField from "./PromptField";

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

const LOGO_LABEL_VI: Record<LogoMode, string> = {
  off: "Tắt",
  crop: "Cắt viền",
  delogo: "Xoá logo",
  auto: "Tự động",
};

const SELECT_CLASS =
  "rounded-md border border-line bg-fill px-2 py-1 text-xs text-ink outline-none focus:border-sky-400/60";

export default function VideoParamsPanel() {
  const selectedNodeId = useGraphStore((s) => s.selectedNodeId);
  const nodes = useGraphStore((s) => s.nodes);
  const edges = useGraphStore((s) => s.edges);
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const onEdgesChange = useGraphStore((s) => s.onEdgesChange);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const node = selectedNodeId ? nodes.find((n) => n.id === selectedNodeId) : undefined;
  if (!node || node.data.kind !== "dola_video") {
    return null;
  }
  const id = node.id;
  const data = node.data;

  const durations = allowedDurations(data.mode, data.model);
  const durationValue = durations.includes(data.duration) ? data.duration : durations[0]!;
  const chips = referenceChips(id, nodes, edges);

  function setMode(mode: Mode): void {
    const nextDurations = allowedDurations(mode, data.model);
    const duration = nextDurations.includes(data.duration) ? data.duration : nextDurations[0]!;
    updateNodeData(id, { ...data, mode, duration });
  }

  function setModel(model: Model): void {
    const nextDurations = allowedDurations(data.mode, model);
    const duration = nextDurations.includes(data.duration) ? data.duration : nextDurations[0]!;
    updateNodeData(id, { ...data, model, duration });
  }

  return (
    <div className="pointer-events-auto flex max-w-[calc(100%-2rem)] flex-col gap-2 rounded-2xl border border-line bg-panel/95 px-3 py-2.5 text-ink shadow-2xl backdrop-blur-md">
      {/* Chip tham chieu (cong refs) */}
      {chips.length > 0 ? (
        <div className="flex flex-wrap items-center gap-1.5 border-b border-line pb-2">
          <span className="text-[11px] text-ink2">Tham chiếu:</span>
          {chips.map((chip) => (
            <span
              key={chip.edgeId}
              className="flex items-center gap-1 rounded-md bg-fill2 py-0.5 pl-2 pr-1 text-[11px] text-ink"
              title={chip.label}
            >
              <span className="max-w-[9rem] truncate">{chip.label}</span>
              <button
                type="button"
                title="Bỏ tham chiếu"
                aria-label={`Bỏ tham chiếu ${chip.label}`}
                className="flex h-4 w-4 items-center justify-center rounded text-ink2 hover:bg-fill2 hover:text-ink"
                onClick={() => onEdgesChange([{ type: "remove", id: chip.edgeId }])}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      ) : null}

      {/* O prompt — kieu Lumina: sua ngay trong panel video; go khi CHUA noi node Prompt se tu tao +
          noi (xem doc `PromptField`). */}
      <PromptField id={id} />

      {/* Hang tham so chinh */}
      <div className="flex flex-wrap items-center gap-2">
        <select className={SELECT_CLASS} value={data.model} onChange={(e) => setModel(e.target.value as Model)} title="Model">
          {ModelSchema.options.map((m) => (
            <option key={m} value={m}>
              {MODEL_LABEL_VI[m]}
            </option>
          ))}
        </select>
        <select className={SELECT_CLASS} value={data.ratio} onChange={(e) => updateNodeData(id, { ...data, ratio: e.target.value as Ratio })} title="Tỉ lệ khung hình">
          {RatioSchema.options.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
        <span className="rounded-md bg-fill px-2 py-1 text-xs text-ink2" title="Độ phân giải">
          1080p
        </span>
        <select
          className={SELECT_CLASS}
          value={durationValue}
          onChange={(e) => updateNodeData(id, { ...data, duration: Number(e.target.value) })}
          title="Thời lượng"
        >
          {durations.map((d) => (
            <option key={d} value={d}>
              {d}s
            </option>
          ))}
        </select>
        <button
          type="button"
          className="text-xs text-rest hover:text-rest"
          onClick={() => setShowAdvanced((v) => !v)}
        >
          Tùy chọn nâng cao {showAdvanced ? "▲" : "▼"}
        </button>
        <span className="ml-auto rounded-md bg-fill px-2 py-1 text-xs text-ink2" title="Số lượng video">
          1×
        </span>
      </div>

      {/* Nang cao: che do / logo / tai khoan */}
      {showAdvanced ? (
        <div className="flex flex-wrap items-center gap-2 border-t border-line pt-2">
          <label className="flex items-center gap-1 text-[11px] text-ink2">
            Chế độ
            <select className={SELECT_CLASS} value={data.mode} onChange={(e) => setMode(e.target.value as Mode)}>
              {ModeSchema.options.map((m) => (
                <option key={m} value={m}>
                  {MODE_LABEL_VI[m]}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-1 text-[11px] text-ink2">
            Logo
            <select className={SELECT_CLASS} value={data.logo} onChange={(e) => updateNodeData(id, { ...data, logo: e.target.value as LogoMode })}>
              {LogoModeSchema.options.map((l) => (
                <option key={l} value={l}>
                  {LOGO_LABEL_VI[l]}
                </option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-1 text-[11px] text-ink2">
            Tài khoản
            <input
              type="text"
              className={SELECT_CLASS + " w-40"}
              placeholder="(tự chọn)"
              value={data.account}
              onChange={(e) => updateNodeData(id, { ...data, account: e.target.value })}
            />
          </label>
        </div>
      ) : null}
    </div>
  );
}
