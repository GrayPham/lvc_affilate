// Node "Tải ảnh lên" (NodeKind::ImageUpload). P-lumina: THAN node de KEO di chuyen (bam-keo =
// di chuyen card); moi HANH DONG (Thêm/Thay thế/Xoá hết/Phóng to) o THANH CONG CU NOI khi chon node.
// Moi thumbnail co nut × rieng (hover) de xoa nhanh. Duong dan luu qua `updateNodeData` nhu cu.
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
import NodeFloatingToolbar, { type ToolbarAction } from "./NodeFloatingToolbar";
import Lightbox from "../components/Lightbox";

type ImageUploadKind = Extract<NodeKind, { kind: "image_upload" }>;
export type ImageUploadRFNode = RFNodeBase<ImageUploadKind, "image_upload">;

const ICON = {
  add: (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
      <path d="M12 5v14M5 12h14" />
    </svg>
  ),
  replace: (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 2v6h-6" />
      <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
      <path d="M3 22v-6h6" />
      <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
    </svg>
  ),
  trash: (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14" />
    </svg>
  ),
  expand: (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
    </svg>
  ),
};

export default function ImageUploadNode({ id, data, selected }: NodeProps<ImageUploadRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const [pickError, setPickError] = useState<string | null>(null);
  const [zoom, setZoom] = useState<string>("");

  async function pick(replace: boolean): Promise<void> {
    setPickError(null);
    try {
      const picked = await pickFiles(true);
      if (picked.length === 0) {
        return;
      }
      const merged = replace ? picked : [...data.paths, ...picked];
      updateNodeData(id, { ...data, paths: Array.from(new Set(merged)) });
    } catch (e) {
      setPickError(errorText(e));
    }
  }

  function removeAt(path: string): void {
    updateNodeData(id, { ...data, paths: data.paths.filter((p) => p !== path) });
  }

  const count = data.paths.length;

  const actions: ToolbarAction[] = [
    { key: "add", title: "Thêm ảnh", icon: ICON.add, onClick: () => void pick(false) },
    { key: "replace", title: "Thay thế toàn bộ", icon: ICON.replace, onClick: () => void pick(true), disabled: count === 0 },
    {
      key: "expand",
      title: "Phóng to",
      icon: ICON.expand,
      onClick: () => setZoom(assetUrl(data.paths[0])),
      disabled: count === 0,
    },
    {
      key: "clear",
      title: "Xoá hết",
      icon: ICON.trash,
      danger: true,
      onClick: () => updateNodeData(id, { ...data, paths: [] }),
      disabled: count === 0,
    },
  ];

  return (
    <>
      <NodeFloatingToolbar nodeId={id} actions={actions} />
      <MediaCard
        label={(data.label ?? "").trim() || NODE_LABEL_VI.image_upload}
        selected={selected}
        errors={errors}
        runState={runState}
        onRename={(name) => updateNodeData(id, { ...data, label: name })}
      >
        {count === 0 ? (
          // Chua co anh: than the KEO duoc; nut "Chọn ảnh" (nodrag) o giua de chon lan dau.
          <div className="flex aspect-square w-full flex-col items-center justify-center gap-2 bg-gradient-to-b from-white/[0.03] to-transparent text-slate-400">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <circle cx="9" cy="9" r="1.6" />
              <path d="m21 15-4.5-4.5L7 20" />
            </svg>
            <button
              type="button"
              className="nodrag rounded-md border border-white/15 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10"
              onClick={() => void pick(false)}
            >
              Chọn ảnh…
            </button>
          </div>
        ) : (
          // Luoi thumbnail — than KEO duoc; nut × tren moi anh (nodrag) va o "+" (nodrag).
          <div className="grid grid-cols-3 gap-1.5 p-2">
            {data.paths.slice(0, 8).map((p) => (
              <div key={p} className="group relative aspect-square overflow-hidden rounded-md border border-white/10 bg-black/30">
                <img
                  src={assetUrl(p)}
                  alt=""
                  className="h-full w-full object-cover"
                  onError={(e) => {
                    (e.currentTarget as HTMLImageElement).style.visibility = "hidden";
                  }}
                />
                <button
                  type="button"
                  className="nodrag absolute right-0.5 top-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-black/70 text-white opacity-0 transition-opacity hover:bg-red-600 group-hover:opacity-100"
                  onClick={(e) => {
                    e.stopPropagation();
                    removeAt(p);
                  }}
                  title="Xoá ảnh này"
                  aria-label="Xoá ảnh này"
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
                    <path d="M6 6l12 12M18 6L6 18" />
                  </svg>
                </button>
              </div>
            ))}
            {count > 8 ? (
              <div className="flex aspect-square items-center justify-center rounded-md border border-white/10 bg-black/30 text-[11px] text-slate-300">
                +{count - 8}
              </div>
            ) : null}
            <button
              type="button"
              className="nodrag flex aspect-square items-center justify-center rounded-md border border-dashed border-white/20 bg-white/[0.03] text-slate-400 hover:bg-white/[0.08] hover:text-slate-200"
              onClick={() => void pick(false)}
              title="Thêm ảnh"
              aria-label="Thêm ảnh"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
                <path d="M12 5v14M5 12h14" />
              </svg>
            </button>
          </div>
        )}
        {pickError ? <p className="px-2 py-1 text-[10px] text-red-400">{pickError}</p> : null}
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
      {zoom ? <Lightbox src={zoom} kind="image" onClose={() => setZoom("")} /> : null}
    </>
  );
}
