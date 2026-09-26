// Node "Tải ảnh lên" (NodeKind::ImageUpload). P-lumina: MOI NODE = 1 ANH (gon, khong roi). Than
// node de KEO di chuyen; nut Thay the/Xoa/Phong to o THANH CONG CU NOI khi chon. Nhap dup ten (pill)
// de doi ten -> dung lam nhan tham chieu. Muon nhieu anh: dung nhieu node (moi node mot vai tro).
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

export default function ImageUploadNode({ id, data, selected }: NodeProps<ImageUploadRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const [pickError, setPickError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(false);

  // Chon 1 anh (lay anh dau neu ho chon nhieu) — moi node chi giu DUNG mot anh.
  async function pick(): Promise<void> {
    setPickError(null);
    try {
      const picked = await pickFiles(true);
      if (picked.length === 0) {
        return;
      }
      updateNodeData(id, { ...data, paths: [picked[0]!] });
    } catch (e) {
      setPickError(errorText(e));
    }
  }

  const path = data.paths[0] ?? "";
  const src = assetUrl(path);
  const label = (data.label ?? "").trim() || NODE_LABEL_VI.image_upload;

  const actions: ToolbarAction[] = [
    {
      key: "replace",
      title: path ? "Thay ảnh khác" : "Chọn ảnh",
      onClick: () => void pick(),
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 2v6h-6" />
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
          <path d="M3 22v-6h6" />
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
        </svg>
      ),
    },
    {
      key: "expand",
      title: "Phóng to",
      disabled: !path,
      onClick: () => setZoom(true),
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
        </svg>
      ),
    },
    {
      key: "clear",
      title: "Xoá ảnh",
      danger: true,
      disabled: !path,
      onClick: () => updateNodeData(id, { ...data, paths: [] }),
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14" />
        </svg>
      ),
    },
  ];

  return (
    <>
      <NodeFloatingToolbar nodeId={id} actions={actions} />
      <MediaCard
        label={label}
        selected={selected}
        errors={errors}
        runState={runState}
        onRename={(name) => updateNodeData(id, { ...data, label: name })}
      >
        {path ? (
          // 1 anh — hien day khung (giu ti le, nen den); keo than de di chuyen node.
          <img
            src={src}
            alt=""
            className="block max-h-80 w-full bg-black object-contain"
            onError={(e) => {
              (e.currentTarget as HTMLImageElement).style.visibility = "hidden";
            }}
          />
        ) : (
          // Chua co anh: than the KEO duoc; nut "Chọn ảnh" (nodrag) o giua.
          <div className="flex aspect-square w-full flex-col items-center justify-center gap-2 bg-gradient-to-b from-white/[0.03] to-transparent text-slate-400">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <circle cx="9" cy="9" r="1.6" />
              <path d="m21 15-4.5-4.5L7 20" />
            </svg>
            <button
              type="button"
              className="nodrag rounded-md border border-white/15 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10"
              onClick={() => void pick()}
            >
              Chọn ảnh…
            </button>
          </div>
        )}
        {pickError ? <p className="px-2 py-1 text-[10px] text-red-400">{pickError}</p> : null}
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
      {zoom && src ? <Lightbox src={src} kind="image" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
