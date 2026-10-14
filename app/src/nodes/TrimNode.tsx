// Node "Cắt video" (kind `trim`) — nhận 1 video (cổng `in`) -> cắt theo [start, end] giây -> video
// (cổng `out`). Thân node có 2 ô nhập thời gian; ffmpeg chạy phía Rust (node-trim). end=0 = tới hết.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type TrimKind = Extract<NodeKind, { kind: "trim" }>;
export type TrimRFNode = RFNodeBase<TrimKind, "trim">;

const NUM_CLASS =
  "nodrag w-16 rounded-md border border-line bg-fill px-1.5 py-1 text-xs text-ink outline-none focus:border-sky-400/60";

export default function TrimNode({ id, data, selected }: NodeProps<TrimRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="in" style={{ background: PORT_TYPE_COLOR.video }} />
      <NodeShell label={NODE_LABEL_VI.trim} kind="trim" selected={selected} errors={errors} runState={runState}>
        <div className="flex items-center gap-2 text-[11px] text-ink2">
          <label className="flex items-center gap-1">
            Từ
            <input
              type="number"
              min={0}
              step={0.5}
              className={NUM_CLASS}
              value={data.start}
              onChange={(e) => updateNodeData(id, { ...data, start: Number(e.target.value) || 0 })}
            />
          </label>
          <label className="flex items-center gap-1">
            Đến
            <input
              type="number"
              min={0}
              step={0.5}
              className={NUM_CLASS}
              value={data.end}
              onChange={(e) => updateNodeData(id, { ...data, end: Number(e.target.value) || 0 })}
            />
          </label>
        </div>
        <p className="text-[10px] text-ink3">giây · Đến = 0 nghĩa là cắt tới hết</p>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
    </>
  );
}
