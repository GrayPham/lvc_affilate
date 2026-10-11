// Node "Tạo ảnh Seedance (API Seedream)" (kind `seedance_image`) — API CHÍNH THỨC Volcengine:
// prompt (bắt buộc) -> 1 ảnh (cổng `out`). Người dùng nhập API key Volcengine Ark + (thường)
// model/endpoint id của họ ở màn Tài khoản AI → API key; node chỉ truyền TÊN credential.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type SeedanceImageKind = Extract<NodeKind, { kind: "seedance_image" }>;
export type SeedanceImageRFNode = RFNodeBase<SeedanceImageKind, "seedance_image">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 placeholder-slate-500 outline-none focus:border-sky-400/60";

export default function SeedanceImageNode({ id, data, selected }: NodeProps<SeedanceImageRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ background: PORT_TYPE_COLOR.text }} />
      <NodeShell label={NODE_LABEL_VI.seedance_image} selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2 text-[11px] text-slate-400">
            <input
              className={`${FIELD_CLASS} flex-1`}
              value={data.model}
              placeholder="Model / endpoint id (rỗng = mặc định)"
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
            placeholder="Tên API key Seedance (bắt buộc)"
            onChange={(e) => updateNodeData(id, { ...data, credential: e.target.value })}
          />
        </div>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
    </>
  );
}
