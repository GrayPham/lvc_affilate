// Node "Prompt" (NodeKind::Prompt, spec §4.2) — textarea nhieu dong, dinh dang TXT P0: moi dong
// khong rong la mot canh, hint `ratio=16:9|dur=10|...` (P0) chi hien nhu placeholder, KHONG parse
// o day (Rust `scenes::split_bulk`/`bulk_scenes` moi la nguon phan tich that qua `workflow_validate`).
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import NodeShell from "./NodeShell";

type PromptKind = Extract<NodeKind, { kind: "prompt" }>;
export type PromptRFNode = RFNodeBase<PromptKind, "prompt">;

function countScenes(text: string): number {
  return text.split("\n").filter((line) => line.trim().length > 0).length;
}

export default function PromptNode({ id, data, selected }: NodeProps<PromptRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeShell label={NODE_LABEL_VI.prompt} selected={selected} errors={errors} runState={runState}>
        <textarea
          className="nodrag h-24 w-full resize-none rounded border border-slate-300 p-1 text-xs"
          value={data.text}
          placeholder={"cảnh 1\ncảnh 2 ratio=16:9|dur=10\ncảnh 3"}
          onChange={(e) => updateNodeData(id, { ...data, text: e.target.value })}
        />
        <p className="text-[10px] text-slate-500">{countScenes(data.text)} cảnh</p>
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
