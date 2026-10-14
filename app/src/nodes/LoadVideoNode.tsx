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
import { useWorkflowUi } from "../store/workflowUi";
import { pickFiles } from "../ipc/commands";
import { assetUrl } from "../ipc/tauri";
import { errorText } from "../ipc/error_text";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar, { type ToolbarAction } from "./NodeFloatingToolbar";
import { useHoverPlay } from "./useHoverPlay";
import Lightbox from "../components/Lightbox";

type LoadVideoKind = Extract<NodeKind, { kind: "load_video" }>;
export type LoadVideoRFNode = RFNodeBase<LoadVideoKind, "load_video">;

export default function LoadVideoNode({ id, data, selected }: NodeProps<LoadVideoRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);
  const selfPos = useGraphStore((s) => s.nodes.find((n) => n.id === id)?.position);
  const openCapture = useWorkflowUi((s) => s.openCapture);
  const openTrim = useWorkflowUi((s) => s.openTrim);
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const [pickError, setPickError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(false);
  const hp = useHoverPlay();

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
      key: "trim",
      title: "Cắt video (chọn đoạn → thêm vào canvas)",
      disabled: !path,
      onClick: () => {
        const base = selfPos ?? { x: 0, y: 0 };
        openTrim({ nodeId: id, src, path, label, x: base.x, y: base.y });
      },
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="6" cy="6" r="3" />
          <circle cx="6" cy="18" r="3" />
          <path d="M20 4L8.12 15.88M14.47 14.48L20 20M8.12 8.12L12 12" />
        </svg>
      ),
    },
    {
      key: "frame",
      title: "Chụp khung hình (bắt khung → thêm ảnh vào canvas)",
      disabled: !path,
      onClick: () => {
        const base = selfPos ?? { x: 0, y: 0 };
        openCapture({ nodeId: id, src, label, x: base.x, y: base.y });
      },
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
          <circle cx="12" cy="13" r="4" />
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
        <div
          className="relative flex aspect-video w-full items-center justify-center bg-gradient-to-br from-[#1f1f26] to-[#111114]"
          {...(src ? hp.hover : {})}
          title={src ? "Rê chuột để xem; Phóng to để phát đầy đủ" : undefined}
        >
          {src ? (
            <video ref={hp.videoRef} src={src} className="pointer-events-none absolute inset-0 h-full w-full bg-black object-contain" muted loop playsInline preload="metadata" />
          ) : (
            <div className="flex flex-col items-center gap-2 text-ink2">
              <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="4" width="20" height="16" rx="2" />
                <path d="m10 9 5 3-5 3z" />
              </svg>
              <button
                type="button"
                className="nodrag rounded-md border border-line2 bg-fill px-2.5 py-1 text-[11px] text-ink hover:bg-fill2"
                onClick={() => void pick()}
              >
                Chọn video…
              </button>
            </div>
          )}
        </div>
        {pickError ? <p className="px-2 py-1 text-[10px] text-rose">{pickError}</p> : null}
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
      {zoom && src ? <Lightbox src={src} kind="video" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
