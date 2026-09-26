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
import { assetUrl } from "../ipc/tauri";
import { errorText } from "../ipc/error_text";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

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

  const count = data.paths.length;
  const firstSrc = count > 0 ? assetUrl(data.paths[0]) : "";
  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <MediaCard label={NODE_LABEL_VI.image_upload} selected={selected} errors={errors} runState={runState}>
        {/* Than the = vung tha/chon anh. Bam vao ca vung de mo dialog chon anh. */}
        <button
          type="button"
          className="nodrag relative flex aspect-square w-full flex-col items-center justify-center gap-1.5 overflow-hidden bg-gradient-to-b from-white/[0.03] to-transparent text-slate-400 hover:bg-white/[0.06]"
          onClick={() => void handlePick()}
          title="Chọn ảnh…"
        >
          {count > 0 && firstSrc ? (
            <>
              {/* Preview anh dau tien lam nen phu kin the (asset:// cua Tauri) */}
              <img
                src={firstSrc}
                alt=""
                className="absolute inset-0 h-full w-full object-cover"
                onError={(e) => {
                  (e.currentTarget as HTMLImageElement).style.display = "none";
                }}
              />
              {count > 1 ? (
                <span className="absolute bottom-2 right-2 rounded-md bg-black/70 px-1.5 py-0.5 text-[10px] font-medium text-white">
                  +{count - 1}
                </span>
              ) : null}
            </>
          ) : count > 0 ? (
            <span className="text-[11px] text-slate-200">{count} ảnh đã chọn</span>
          ) : (
            <>
              <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <circle cx="9" cy="9" r="1.6" />
                <path d="m21 15-4.5-4.5L7 20" />
              </svg>
              <span className="text-[11px]">Chọn ảnh…</span>
            </>
          )}
        </button>
        {count > 0 ? (
          <ul className="nodrag max-h-16 space-y-0.5 overflow-auto border-t border-white/10 bg-black/20 px-2 py-1 text-[10px] text-slate-400">
            {data.paths.map((p) => (
              <li key={p} className="truncate" title={p}>
                {baseName(p)}
              </li>
            ))}
          </ul>
        ) : null}
        {pickError ? <p className="px-2 py-1 text-[10px] text-red-400">{pickError}</p> : null}
      </MediaCard>
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        style={{ background: PORT_TYPE_COLOR.image }}
      />
    </>
  );
}
