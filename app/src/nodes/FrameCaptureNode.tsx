// Node "Chụp khung hình" (kind `frame_capture`) — nhận 1 video (cổng `in`) -> chụp 1 khung tại giây
// `time` -> ảnh PNG (cổng `out`, kiểu image; nối được vào node Lưu). ffmpeg chạy phía Rust.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type FrameCaptureKind = Extract<NodeKind, { kind: "frame_capture" }>;
export type FrameCaptureRFNode = RFNodeBase<FrameCaptureKind, "frame_capture">;

export default function FrameCaptureNode({ id, data, selected }: NodeProps<FrameCaptureRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="in" style={{ background: PORT_TYPE_COLOR.video }} />
      <NodeShell label={NODE_LABEL_VI.frame_capture} kind="frame_capture" selected={selected} errors={errors} runState={runState}>
        <label className="flex items-center gap-2 text-[11px] text-muted">
          Thời điểm
          <input
            type="number"
            min={0}
            step={0.5}
            className="nodrag w-16 rounded-md border border-line bg-fill px-1.5 py-1 text-xs text-fg outline-none focus:border-sky-400/60"
            value={data.time}
            onChange={(e) => updateNodeData(id, { ...data, time: Number(e.target.value) || 0 })}
          />
          giây
        </label>
        <p className="text-[10px] text-ink3">Xuất 1 ảnh PNG — nối vào node Lưu</p>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
    </>
  );
}
