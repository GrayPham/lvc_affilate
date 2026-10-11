// Node "Tạo ảnh Veo3 (Google Labs Flow)" (kind `veo3_image`) — prompt (bắt buộc) + ảnh tham chiếu
// (tuỳ chọn, cổng `refs`) -> 1 ảnh (cổng `out`). Form đơn giản kiểu `ScriptGenNode`/
// `AudioExtractNode` (P5 chưa làm panel Lumina riêng cho 2 node Veo3 — xem spec "Không làm ở P5").
// `account` là id credential Google (`login`) trong kho Tài khoản AI — BẮT BUỘC, chưa có "auto".
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type Veo3ImageKind = Extract<NodeKind, { kind: "veo3_image" }>;
export type Veo3ImageRFNode = RFNodeBase<Veo3ImageKind, "veo3_image">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-white/10 bg-white/5 px-1.5 py-1 text-xs text-slate-100 placeholder-slate-500 outline-none focus:border-sky-400/60";

const MODELS: ReadonlyArray<readonly [string, string]> = [
  ["GEM_PIX_2", "Banana Pro (GEM_PIX_2)"],
  ["NARWHAL", "Banana 2 (NARWHAL)"],
  ["HARBOR_SEAL", "Lite (HARBOR_SEAL)"],
];

export default function Veo3ImageNode({ id, data, selected }: NodeProps<Veo3ImageRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ top: "30%", background: PORT_TYPE_COLOR.text }} />
      <Handle type="target" position={Position.Left} id="refs" style={{ top: "70%", background: PORT_TYPE_COLOR.image }} />
      <NodeShell label={NODE_LABEL_VI.veo3_image} selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-2 text-[11px] text-slate-400">
            <select
              className={`${FIELD_CLASS} flex-1`}
              value={data.model}
              title="Model tạo ảnh"
              onChange={(e) => updateNodeData(id, { ...data, model: e.target.value })}
            >
              {MODELS.map(([v, label]) => (
                <option key={v} value={v}>
                  {label}
                </option>
              ))}
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
          <div className="flex items-center gap-2 text-[11px] text-slate-400">
            <label className="flex flex-1 items-center gap-1">
              Seed
              <input
                type="number"
                className={FIELD_CLASS}
                value={data.seed ?? ""}
                placeholder="rỗng = ngẫu nhiên"
                onChange={(e) =>
                  updateNodeData(id, {
                    ...data,
                    seed: e.target.value === "" ? null : Number(e.target.value),
                  })
                }
              />
            </label>
          </div>
          <input
            className={FIELD_CLASS}
            value={data.account}
            placeholder="Tài khoản (tên/label credential Google, bắt buộc)"
            onChange={(e) => updateNodeData(id, { ...data, account: e.target.value })}
          />
        </div>
      </NodeShell>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
    </>
  );
}
