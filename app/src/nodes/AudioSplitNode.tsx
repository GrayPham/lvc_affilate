// Node "Tách audio↔video" (kind `audio_split`) — nhận 1 video (cổng `in`) -> tách thành 2 nhánh:
// `video` (video không tiếng) + `audio` (chỉ tiếng). Không tham số; ffmpeg chạy phía Rust
// (node-audio-split).
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type AudioSplitKind = Extract<NodeKind, { kind: "audio_split" }>;
export type AudioSplitRFNode = RFNodeBase<AudioSplitKind, "audio_split">;

export default function AudioSplitNode({ id, selected }: NodeProps<AudioSplitRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="in" style={{ background: PORT_TYPE_COLOR.video }} />
      <NodeShell label={NODE_LABEL_VI.audio_split} selected={selected} errors={errors} runState={runState}>
        <p className="text-[10px] text-slate-500">Xuất video không tiếng + 1 file audio riêng</p>
      </NodeShell>
      <Handle
        type="source"
        position={Position.Right}
        id="video"
        style={{ top: "35%", background: PORT_TYPE_COLOR.video }}
      />
      <Handle
        type="source"
        position={Position.Right}
        id="audio"
        style={{ top: "65%", background: PORT_TYPE_COLOR.audio }}
      />
    </>
  );
}
