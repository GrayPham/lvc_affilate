// Node "Lưu" (NodeKind::Save) — nut "Chọn thư mục…" mo dialog he thong qua `pickDirectory()`
// (crates/lvc-app pick_directory). Nhan ca hai cong vao (`video`, `image`, deu many=true) — server
// (`validate.rs::check_missing_required`) doi it nhat MOT trong hai co canh noi toi, khong bat
// buoc ca hai.
import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { pickDirectory } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import NodeShell from "./NodeShell";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type SaveKind = Extract<NodeKind, { kind: "save" }>;
export type SaveRFNode = RFNodeBase<SaveKind, "save">;

export default function SaveNode({ id, data, selected }: NodeProps<SaveRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const [pickError, setPickError] = useState<string | null>(null);

  async function handlePick(): Promise<void> {
    setPickError(null);
    try {
      const dir = await pickDirectory();
      if (dir === null) {
        return;
      }
      updateNodeData(id, { ...data, out_dir: dir });
    } catch (e) {
      setPickError(errorText(e));
    }
  }

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle
        type="target"
        position={Position.Left}
        id="video"
        style={{ top: "35%", background: PORT_TYPE_COLOR.video }}
      />
      <Handle
        type="target"
        position={Position.Left}
        id="image"
        style={{ top: "65%", background: PORT_TYPE_COLOR.image }}
      />
      <NodeShell label={NODE_LABEL_VI.save} selected={selected} errors={errors} runState={runState}>
        <button
          type="button"
          className="nodrag rounded-md border border-white/10 bg-white/5 px-2 py-1 text-xs text-slate-200 hover:bg-white/10"
          onClick={() => void handlePick()}
        >
          Chọn thư mục…
        </button>
        <p className="truncate text-[10px] text-slate-400" title={data.out_dir}>
          {data.out_dir || "Chưa chọn thư mục"}
        </p>
        {pickError ? <p className="text-[10px] text-red-400">{pickError}</p> : null}
      </NodeShell>
    </>
  );
}
