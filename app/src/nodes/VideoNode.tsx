// Node "Tạo video Dola" (kind `dola_video`, truoc P4a la `video`) — chon mode/model/ti le/thoi luong/logo + tai khoan.
// Thoi luong loc theo `allowedDurations(mode, model)` (mirror `Mode::allowed_durations`,
// crates/lvc-core/src/mode.rs:53). Tai khoan: `account_list` CHUA co (Task 10 dang lam) —
// dung o nhap van ban, rong = tu chon (khop hanh vi server: `Node.account` rong -> engine tu
// chon tai khoan con han muc). TODO(T14): doi sang dropdown khi `account_list` san sang.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import {
  LogoModeSchema,
  ModeSchema,
  ModelSchema,
  RatioSchema,
  type LogoMode,
  type Mode,
  type Model,
  type NodeKind,
  type Ratio,
} from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { allowedDurations } from "../graph/rules";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";

type VideoKind = Extract<NodeKind, { kind: "dola_video" }>;
export type VideoRFNode = RFNodeBase<VideoKind, "dola_video">;

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

export default function VideoNode({ id, data, selected }: NodeProps<VideoRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  const durations = allowedDurations(data.mode, data.model);
  const durationValue = durations.includes(data.duration) ? data.duration : durations[0]!;

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
    <>
      <Handle
        type="target"
        position={Position.Left}
        id="prompt"
        style={{ top: "30%", background: PORT_TYPE_COLOR.text }}
      />
      <Handle
        type="target"
        position={Position.Left}
        id="refs"
        style={{ top: "70%", background: PORT_TYPE_COLOR.image }}
      />
      <NodeShell label={NODE_LABEL_VI.dola_video} selected={selected} errors={errors} runState={runState}>
        <label className="text-[10px] text-slate-500">
          Chế độ
          <select
            className="nodrag w-full rounded border border-slate-300 px-1 py-0.5 text-xs"
            value={data.mode}
            onChange={(e) => setMode(e.target.value as Mode)}
          >
            {ModeSchema.options.map((m) => (
              <option key={m} value={m}>
                {MODE_LABEL_VI[m]}
              </option>
            ))}
          </select>
        </label>
        <label className="text-[10px] text-slate-500">
          Model
          <select
            className="nodrag w-full rounded border border-slate-300 px-1 py-0.5 text-xs"
            value={data.model}
            onChange={(e) => setModel(e.target.value as Model)}
          >
            {ModelSchema.options.map((m) => (
              <option key={m} value={m}>
                {MODEL_LABEL_VI[m]}
              </option>
            ))}
          </select>
        </label>
        <label className="text-[10px] text-slate-500">
          Tỉ lệ
          <select
            className="nodrag w-full rounded border border-slate-300 px-1 py-0.5 text-xs"
            value={data.ratio}
            onChange={(e) => updateNodeData(id, { ...data, ratio: e.target.value as Ratio })}
          >
            {RatioSchema.options.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
        </label>
        <label className="text-[10px] text-slate-500">
          Thời lượng
          <select
            className="nodrag w-full rounded border border-slate-300 px-1 py-0.5 text-xs"
            value={durationValue}
            onChange={(e) => updateNodeData(id, { ...data, duration: Number(e.target.value) })}
          >
            {durations.map((d) => (
              <option key={d} value={d}>
                {d}s
              </option>
            ))}
          </select>
        </label>
        <label className="text-[10px] text-slate-500">
          Logo
          <select
            className="nodrag w-full rounded border border-slate-300 px-1 py-0.5 text-xs"
            value={data.logo}
            onChange={(e) => updateNodeData(id, { ...data, logo: e.target.value as LogoMode })}
          >
            {LogoModeSchema.options.map((l) => (
              <option key={l} value={l}>
                {LOGO_LABEL_VI[l]}
              </option>
            ))}
          </select>
        </label>
        <label className="text-[10px] text-slate-500">
          Tài khoản
          <input
            type="text"
            className="nodrag w-full rounded border border-slate-300 px-1 py-0.5 text-xs"
            placeholder="(tự chọn)"
            value={data.account}
            onChange={(e) => updateNodeData(id, { ...data, account: e.target.value })}
          />
        </label>
      </NodeShell>
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        style={{ background: PORT_TYPE_COLOR.video }}
      />
    </>
  );
}
