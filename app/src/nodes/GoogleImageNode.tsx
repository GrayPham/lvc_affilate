// Node "Tạo ảnh Google (API Imagen)" (kind `google_image`) — API CHÍNH THỨC: prompt (bắt buộc) ->
// 1 ảnh (cổng `out`). Người dùng nhập API key Google của chính họ (cùng key Gemini) ở màn Tài khoản
// AI → API key; node chỉ truyền TÊN (label) credential, không thấy key. Khác hẳn node Veo3 (trình
// duyệt Flow) — đây là HTTP thuần.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type GoogleImageKind = Extract<NodeKind, { kind: "google_image" }>;
export type GoogleImageRFNode = RFNodeBase<GoogleImageKind, "google_image">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 placeholder-slate-500 outline-none focus:border-sky-400/60";

export default function GoogleImageNode({ id, data, selected }: NodeProps<GoogleImageRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ background: PORT_TYPE_COLOR.text }} />
      <NodeShell label={NODE_LABEL_VI.google_image} selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2 text-[11px] text-slate-400">
            <input
              className={`${FIELD_CLASS} flex-1`}
              value={data.model}
              placeholder="Model (rỗng = gemini-3.1-flash-image)"
              onChange={(e) => updateNodeData(id, { ...data, model: e.target.value })}
            />
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
            value={data.credential}
            placeholder="Tên API key Google (bắt buộc)"
            onChange={(e) => updateNodeData(id, { ...data, credential: e.target.value })}
          />
        </div>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
    </>
  );
}
