// Node "Kịch bản AI" (kind `script_gen`) — nhập chủ đề/brief -> AI (Gemini/OpenAI) viết N phân cảnh
// -> cổng ra `out` (text, mỗi dòng một cảnh) nối thẳng vào cổng `prompt` của node "Tạo video Dola".
// Cổng vào `brief` (text, tuỳ chọn) thay cho ô "Chủ đề" khi được nối. Bí mật (API key) do backend
// giữ; ở đây chỉ khai TÊN key + nhà cung cấp. Mọi chỉnh sửa lưu qua `updateNodeData` như các node khác.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type ScriptGenKind = Extract<NodeKind, { kind: "script_gen" }>;
export type ScriptGenRFNode = RFNodeBase<ScriptGenKind, "script_gen">;

const FIELD_CLASS =
  "nodrag w-full rounded-md border border-line bg-fill px-1.5 py-1 text-xs text-ink placeholder-slate-500 outline-none focus:border-sky-400/60";

export default function ScriptGenNode({ id, data, selected }: NodeProps<ScriptGenRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle
        type="target"
        position={Position.Left}
        id="brief"
        style={{ background: PORT_TYPE_COLOR.text }}
      />
      <NodeShell label={NODE_LABEL_VI.script_gen} kind="script_gen" selected={selected} errors={errors} runState={runState}>
        <div className="flex flex-col gap-1.5">
          <textarea
            className={`${FIELD_CLASS} h-16 resize-none`}
            value={data.topic}
            placeholder="Chủ đề / sản phẩm (bỏ trống nếu nối cổng brief)"
            onChange={(e) => updateNodeData(id, { ...data, topic: e.target.value })}
          />
          <div className="flex items-center gap-2 text-[11px] text-ink2">
            <label className="flex items-center gap-1">
              Số cảnh
              <input
                type="number"
                min={1}
                className={`${FIELD_CLASS} w-14`}
                value={data.num_scenes}
                onChange={(e) =>
                  updateNodeData(id, { ...data, num_scenes: Math.max(1, Number(e.target.value) || 1) })
                }
              />
            </label>
            <select
              className={`${FIELD_CLASS} flex-1`}
              value={data.provider}
              title="Nhà cung cấp AI"
              onChange={(e) =>
                updateNodeData(id, { ...data, provider: e.target.value as ScriptGenKind["provider"] })
              }
            >
              <option value="gemini">Gemini</option>
              <option value="openai">OpenAI</option>
            </select>
          </div>
          <input
            className={FIELD_CLASS}
            value={data.credential}
            placeholder="Tên API key trong kho"
            onChange={(e) => updateNodeData(id, { ...data, credential: e.target.value })}
          />
          <label className="flex items-center gap-1.5 text-[11px] text-ink2">
            <input
              type="checkbox"
              className="nodrag"
              checked={data.save_as_script}
              onChange={(e) => updateNodeData(id, { ...data, save_as_script: e.target.checked })}
            />
            Lưu thành kịch bản khi chạy
          </label>
          {data.save_as_script ? (
            <input
              className={FIELD_CLASS}
              value={data.script_name}
              placeholder="Tên kịch bản (bỏ trống để tự đặt tên)"
              onChange={(e) => updateNodeData(id, { ...data, script_name: e.target.value })}
            />
          ) : null}
        </div>
      </NodeShell>
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        style={{ background: PORT_TYPE_COLOR.text }}
      />
    </>
  );
}
