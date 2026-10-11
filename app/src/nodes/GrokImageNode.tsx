// Node "Tạo ảnh Grok (API xAI)" (kind `grok_image`) — API CHÍNH THỨC xAI: prompt (bắt buộc) -> 1
// ảnh (cổng `out`). Người dùng nhập API key xAI; node chỉ truyền TÊN credential. xAI image API
// không nhận tỉ lệ khung nên form chỉ có model + credential.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type GrokImageKind = Extract<NodeKind, { kind: "grok_image" }>;
export type GrokImageRFNode = RFNodeBase<GrokImageKind, "grok_image">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 placeholder-slate-500 outline-none focus:border-sky-400/60";

export default function GrokImageNode({ id, data, selected }: NodeProps<GrokImageRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ background: PORT_TYPE_COLOR.text }} />
      <NodeShell label={NODE_LABEL_VI.grok_image} selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <input
            className={FIELD_CLASS}
            value={data.model}
            placeholder="Model (rỗng = grok-2-image-1212)"
            onChange={(e) => updateNodeData(id, { ...data, model: e.target.value })}
          />
          <input
            className={FIELD_CLASS}
            value={data.credential}
            placeholder="Tên API key Grok (bắt buộc)"
            onChange={(e) => updateNodeData(id, { ...data, credential: e.target.value })}
          />
        </div>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
    </>
  );
}
