// Node "Tải ảnh lên" (NodeKind::ImageUpload) — nut "Chọn ảnh…" mo dialog he thong qua
// `pickFiles(true)` (crates/lvc-app pick_files, loc png/jpg/jpeg/webp), gop them vao danh sach
// duong dan da chon (khu trung), hien ten file (khong hien duong dan day du tren khung nho).
import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { pickFiles } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import NodeShell from "./NodeShell";

type ImageUploadKind = Extract<NodeKind, { kind: "image_upload" }>;
export type ImageUploadRFNode = RFNodeBase<ImageUploadKind, "image_upload">;

function baseName(p: string): string {
  const parts = p.split(/[/\\]/);
  return parts[parts.length - 1] ?? p;
}

export default function ImageUploadNode({ id, data, selected }: NodeProps<ImageUploadRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const [pickError, setPickError] = useState<string | null>(null);

  async function handlePick(): Promise<void> {
    setPickError(null);
    try {
      const picked = await pickFiles(true);
      if (picked.length === 0) {
        return;
      }
      const merged = Array.from(new Set([...data.paths, ...picked]));
      updateNodeData(id, { ...data, paths: merged });
    } catch (e) {
      setPickError(errorText(e));
    }
  }

  return (
    <>
      <NodeShell label={NODE_LABEL_VI.image_upload} selected={selected} errors={errors} runState={runState}>
        <button
          type="button"
          className="nodrag rounded border border-slate-300 px-2 py-1 text-xs hover:bg-slate-50"
          onClick={() => void handlePick()}
        >
          Chọn ảnh…
        </button>
        {data.paths.length > 0 ? (
          <ul className="max-h-20 overflow-auto text-[10px] text-slate-600">
            {data.paths.map((p) => (
              <li key={p} className="truncate" title={p}>
                {baseName(p)}
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-[10px] text-slate-400">Chưa chọn ảnh</p>
        )}
        {pickError ? <p className="text-[10px] text-red-600">{pickError}</p> : null}
      </NodeShell>
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        style={{ background: PORT_TYPE_COLOR.image }}
      />
    </>
  );
}
