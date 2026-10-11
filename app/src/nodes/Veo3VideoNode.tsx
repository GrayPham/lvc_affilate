// Node "Tạo video Veo3" (kind `veo3_video`) — prompt (bắt buộc, t2v/r2v) + ảnh tham chiếu (cổng
// `refs`, chỉ dùng khi mode="r2v", tối đa 6 ảnh) -> 1 video (cổng `out`). Form đơn giản kiểu
// `ScriptGenNode` (P5 chưa làm panel Lumina riêng — xem spec "Không làm ở P5"). `account` là id
// credential Google (`login`) trong kho Tài khoản AI — BẮT BUỘC, chưa có "auto".
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type Veo3VideoKind = Extract<NodeKind, { kind: "veo3_video" }>;
export type Veo3VideoRFNode = RFNodeBase<Veo3VideoKind, "veo3_video">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 placeholder-slate-500 outline-none focus:border-sky-400/60";

export default function Veo3VideoNode({ id, data, selected }: NodeProps<Veo3VideoRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ top: "30%", background: PORT_TYPE_COLOR.text }} />
      <Handle type="target" position={Position.Left} id="refs" style={{ top: "70%", background: PORT_TYPE_COLOR.image }} />
      <NodeShell label={NODE_LABEL_VI.veo3_video} selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2 text-[11px] text-slate-400">
            <select
              className={`${FIELD_CLASS} flex-1`}
              value={data.mode}
              title="Chế độ"
              onChange={(e) => updateNodeData(id, { ...data, mode: e.target.value })}
            >
              <option value="t2v">Chữ → video (t2v)</option>
              <option value="r2v">Ảnh tham chiếu → video (r2v, tối đa 6 ảnh)</option>
            </select>
            <select
              className={FIELD_CLASS}
              value={data.aspect}
              title="Tỉ lệ khung"
              onChange={(e) => updateNodeData(id, { ...data, aspect: e.target.value })}
            >
              <option value="portrait">Dọc</option>
              <option value="landscape">Ngang</option>
            </select>
          </div>
          <input
            className={FIELD_CLASS}
            value={data.model}
            placeholder="Model (rỗng = mặc định theo mode)"
            onChange={(e) => updateNodeData(id, { ...data, model: e.target.value })}
          />
          <label className="flex items-center gap-2 text-[11px] text-slate-400">
            Số biến thể (seed_count)
            <input
              type="number"
              min={1}
              className={`${FIELD_CLASS} w-16`}
              value={data.seed_count}
              onChange={(e) =>
                updateNodeData(id, { ...data, seed_count: Math.max(1, Number(e.target.value) || 1) })
              }
            />
          </label>
          <input
            className={FIELD_CLASS}
            value={data.account}
            placeholder="Tài khoản (tên/label credential Google, bắt buộc)"
            onChange={(e) => updateNodeData(id, { ...data, account: e.target.value })}
          />
          <div className="flex items-center gap-1.5 text-[10px] text-slate-500">
            <span className="rounded bg-white/10 px-1 py-0.5 text-[9px] uppercase tracking-wide text-slate-400">
              Sắp có
            </span>
            <span>Chế độ trình duyệt chống phát hiện (Camoufox)</span>
          </div>
        </div>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
    </>
  );
}
