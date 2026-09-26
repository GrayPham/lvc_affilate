// Node "Tải video lên" (kind `load_video`) — chọn 1 video từ máy -> cổng `out` (video). Dùng làm
// nguồn cho Cắt video / Chụp khung hình / Ghép phân cảnh. Thân node xem trước video (asset://).
// P-lumina: thân kéo di chuyển; nút Thay/Xoá/Phóng to ở thanh công cụ nổi; nhấp đúp tên để đổi.
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

type LoadVideoKind = Extract<NodeKind, { kind: "load_video" }>;
export type LoadVideoRFNode = RFNodeBase<LoadVideoKind, "load_video">;

export default function LoadVideoNode({ id, data, selected }: NodeProps<LoadVideoRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const [pickError, setPickError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(false);

  async function pick(): Promise<void> {
    setPickError(null);
    try {
      const picked = await pickFiles(false);
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
  const label = (data.label ?? "").trim() || NODE_LABEL_VI.load_video;

  const actions: ToolbarAction[] = [
    {
      key: "replace",
      title: path ? "Thay video (tải lên video khác)" : "Chọn video (tải lên)",
      onClick: () => void pick(),
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <path d="M17 8l-5-5-5 5" />
          <path d="M12 3v12" />
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
      title: "Xoá video",
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
        wide
        onRename={(name) => updateNodeData(id, { ...data, label: name })}
      >
        <div className="relative flex aspect-video w-full items-center justify-center bg-gradient-to-br from-[#1f1f26] to-[#111114]">
          {src ? (
            <video src={src} className="nodrag absolute inset-0 h-full w-full bg-black object-contain" controls muted playsInline preload="metadata" />
          ) : (
            <div className="flex flex-col items-center gap-2 text-slate-400">
              <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="4" width="20" height="16" rx="2" />
                <path d="m10 9 5 3-5 3z" />
              </svg>
              <button
                type="button"
                className="nodrag rounded-md border border-white/15 bg-white/5 px-2.5 py-1 text-[11px] text-slate-200 hover:bg-white/10"
                onClick={() => void pick()}
              >
                Chọn video…
              </button>
            </div>
          )}
        </div>
        {pickError ? <p className="px-2 py-1 text-[10px] text-red-400">{pickError}</p> : null}
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
      {zoom && src ? <Lightbox src={src} kind="video" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
