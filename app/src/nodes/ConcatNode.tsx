// Node "Ghép phân cảnh" (kind `concat`) — nhan nhieu video (cong `in`) -> 1 video (cong `out`).
// Than gon kieu Lumina: the toi + icon ghep. Preview video ket qua khi da chay (nodeVideos).
import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { assetUrl } from "../ipc/tauri";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar, { type ToolbarAction } from "./NodeFloatingToolbar";
import { MuteButton, useHoverPlay } from "./useHoverPlay";
import Lightbox from "../components/Lightbox";

type ConcatKind = Extract<NodeKind, { kind: "concat" }>;
export type ConcatRFNode = RFNodeBase<ConcatKind, "concat">;

export default function ConcatNode({ id, selected }: NodeProps<ConcatRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const videoPath = useRunStore((s) => s.nodeVideos[id]);
  const videoSrc = assetUrl(videoPath);
  const [zoom, setZoom] = useState(false);
  const hp = useHoverPlay();

  const actions: ToolbarAction[] = [
    {
      key: "expand",
      title: videoSrc ? "Phóng to video" : "Chưa có video (chạy để ghép)",
      disabled: !videoSrc,
      onClick: () => setZoom(true),
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
        </svg>
      ),
    },
  ];

  return (
    <>
      <NodeFloatingToolbar nodeId={id} actions={actions} />
      <Handle type="target" position={Position.Left} id="in" style={{ background: PORT_TYPE_COLOR.video }} />
      <MediaCard label={NODE_LABEL_VI.concat} kind="concat" selected={selected} errors={errors} runState={runState} wide>
        <div
          className="relative flex aspect-video w-full items-center justify-center bg-gradient-to-br from-fill2 to-fill"
          {...(videoSrc ? hp.hover : {})}
          title={videoSrc ? "Rê chuột để xem; Phóng to để phát đầy đủ" : undefined}
        >
          {videoSrc ? (
            <>
              <video ref={hp.videoRef} src={videoSrc} className="pointer-events-none absolute inset-0 h-full w-full bg-black object-contain" muted={hp.muted} loop playsInline preload="metadata" />
              <MuteButton />
            </>
          ) : (
            <span className="flex flex-col items-center gap-1.5 text-fg">
              <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="4" width="9" height="7" rx="1.5" />
                <rect x="2" y="13" width="9" height="7" rx="1.5" />
                <path d="M13 12h6M19 12l-2.5-2.5M19 12l-2.5 2.5" />
                <rect x="20" y="8.5" width="2" height="7" rx="1" />
              </svg>
              <span className="text-[12px] font-semibold">NỐI VIDEO</span>
              <span className="text-[10px] text-muted">kéo các cảnh vào cổng trái →</span>
            </span>
          )}
        </div>
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
      {zoom && videoSrc ? <Lightbox src={videoSrc} kind="video" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
