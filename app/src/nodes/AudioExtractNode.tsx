// Node "Tách audio" (kind `audio_extract`) — nhận 1 video (cổng `in`) -> tách âm thanh -> file
// audio (cổng `out`, kiểu audio). Thân node có chọn định dạng ra; ffmpeg chạy phía Rust
// (node-audio-extract).
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { AudioFormat, NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type AudioExtractKind = Extract<NodeKind, { kind: "audio_extract" }>;
export type AudioExtractRFNode = RFNodeBase<AudioExtractKind, "audio_extract">;

const FORMATS: readonly AudioFormat[] = ["mp3", "aac", "wav"];

export default function AudioExtractNode({ id, data, selected }: NodeProps<AudioExtractRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="in" style={{ background: PORT_TYPE_COLOR.video }} />
      <NodeShell label={NODE_LABEL_VI.audio_extract} selected={selected} errors={errors} runState={runState}>
        <label className="flex items-center gap-2 text-[11px] text-slate-400">
          Định dạng
          <select
            className="nodrag rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 outline-none focus:border-sky-400/60"
            value={data.format}
            onChange={(e) => updateNodeData(id, { ...data, format: e.target.value as AudioFormat })}
          >
            {FORMATS.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        <p className="text-[10px] text-slate-500">Xuất 1 file audio — nối vào node Lưu</p>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.audio }} />
    </>
  );
}
