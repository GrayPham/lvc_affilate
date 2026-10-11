// Node "Tạo video Google (API Veo)" (kind `google_video`) — API CHÍNH THỨC: prompt (bắt buộc) +
// ảnh gốc (cổng `ref`, chỉ dùng khi mode=i2v) -> 1 video (cổng `out`). Người dùng nhập API key
// Google của chính họ; node chỉ truyền TÊN (label) credential. HTTP thuần (khác node Veo3 trình
// duyệt Flow).
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type GoogleVideoKind = Extract<NodeKind, { kind: "google_video" }>;
export type GoogleVideoRFNode = RFNodeBase<GoogleVideoKind, "google_video">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 placeholder-slate-500 outline-none focus:border-sky-400/60";

export default function GoogleVideoNode({ id, data, selected }: NodeProps<GoogleVideoRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ top: "30%", background: PORT_TYPE_COLOR.text }} />
      <Handle type="target" position={Position.Left} id="ref" style={{ top: "70%", background: PORT_TYPE_COLOR.image }} />
      <NodeShell label={NODE_LABEL_VI.google_video} selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2 text-[11px] text-slate-400">
            <select
              className={`${FIELD_CLASS} flex-1`}
              value={data.mode}
              title="Chế độ"
              onChange={(e) => updateNodeData(id, { ...data, mode: e.target.value })}
            >
              <option value="t2v">Chữ → video (t2v)</option>
              <option value="i2v">Ảnh → video (i2v)</option>
            </select>
            <select
              className={FIELD_CLASS}
              value={data.aspect}
              title="Tỉ lệ khung"
              onChange={(e) => updateNodeData(id, { ...data, aspect: e.target.value })}
            >
              <option value="portrait">Dọc</option>
              <option value="landscape">Ngang</option>
              <option value="square">Vuông</option>
            </select>
          </div>
          <input
            className={FIELD_CLASS}
            value={data.model}
            placeholder="Model (rỗng = veo-3.1-generate-preview)"
            onChange={(e) => updateNodeData(id, { ...data, model: e.target.value })}
          />
          <input
            className={FIELD_CLASS}
            value={data.credential}
            placeholder="Tên API key Google (bắt buộc)"
            onChange={(e) => updateNodeData(id, { ...data, credential: e.target.value })}
          />
        </div>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
    </>
  );
}
