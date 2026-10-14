// Node "Lưu" (NodeKind::Save) — nut "Chọn thư mục…" mo dialog he thong qua `pickDirectory()`
// (crates/lvc-app pick_directory). Nhan ca ba cong vao (`video`, `image`, `audio`, deu many=true) —
// server (`validate.rs::check_missing_required`) doi it nhat MOT trong ba co canh noi toi, khong
// bat buoc ca ba.
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
        style={{ top: "25%", background: PORT_TYPE_COLOR.video }}
      />
      <Handle
        type="target"
        position={Position.Left}
        id="image"
        style={{ top: "50%", background: PORT_TYPE_COLOR.image }}
      />
      <Handle
        type="target"
        position={Position.Left}
        id="audio"
        style={{ top: "75%", background: PORT_TYPE_COLOR.audio }}
      />
      <NodeShell label={NODE_LABEL_VI.save} kind="save" selected={selected} errors={errors} runState={runState}>
        <button
          type="button"
          className="nodrag rounded-md border border-line bg-fill px-2 py-1 text-xs text-ink hover:bg-fill2"
          onClick={() => void handlePick()}
        >
          Chọn thư mục…
        </button>
        <p className="truncate text-[10px] text-ink2" title={data.out_dir}>
          {data.out_dir || "Chưa chọn thư mục"}
        </p>
        {pickError ? <p className="text-[10px] text-rose">{pickError}</p> : null}
      </NodeShell>
    </>
  );
}
