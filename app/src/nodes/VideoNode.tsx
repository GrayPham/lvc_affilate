// Node "Tạo video Dola" (kind `dola_video`, truoc P4a la `video`).
// P-lumina: THAN node gio gon lai — chi la the media (vung xem truoc video + pill nhan + cham trang
// thai). Cac tham so (che do/model/ti le/thoi luong/logo/tai khoan) da chuyen xuong BANG DUOI khi
// node duoc chon (xem `components/VideoParamsPanel.tsx`) de node gon dep kieu Lumina. Van giu 2 cong
// vao (prompt=text, refs=image) + 1 cong ra (video), mau theo kieu cong; chinh sua van luu qua
// `updateNodeData` cua store nhu cu.
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
import Lightbox from "../components/Lightbox";

type VideoKind = Extract<NodeKind, { kind: "dola_video" }>;
export type VideoRFNode = RFNodeBase<VideoKind, "dola_video">;

export default function VideoNode({ id, data, selected }: NodeProps<VideoRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  // Video HOAN TAT cua node nay (neu co) — preview mp4 ket qua (asset:// cua Tauri).
  const videoPath = useRunStore((s) => s.nodeVideos[id]);
  const videoSrc = assetUrl(videoPath);
  const [zoom, setZoom] = useState(false);

  const actions: ToolbarAction[] = [
    {
      key: "expand",
      title: videoSrc ? "Phóng to video" : "Chưa có video (chạy để tạo)",
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
      <Handle
        type="target"
        position={Position.Left}
        id="prompt"
        style={{ top: "30%", background: PORT_TYPE_COLOR.text }}
      />
      <Handle
        type="target"
        position={Position.Left}
        id="refs"
        style={{ top: "70%", background: PORT_TYPE_COLOR.image }}
      />
      <MediaCard label={NODE_LABEL_VI.dola_video} selected={selected} errors={errors} runState={runState} wide>
        {/* Vung xem truoc: video ket qua neu da chay xong; chua co thi placeholder toi + glyph play. */}
        <div className="relative flex aspect-video w-full items-center justify-center bg-gradient-to-br from-[#1f1f26] to-[#111114]">
          {videoSrc ? (
            <video
              src={videoSrc}
              className="nodrag absolute inset-0 h-full w-full bg-black object-contain"
              controls
              muted
              playsInline
              preload="metadata"
            />
          ) : (
            <span className="flex h-11 w-11 items-center justify-center rounded-full bg-white/10 text-white">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M8 5v14l11-7z" />
              </svg>
            </span>
          )}
          <span className="pointer-events-none absolute bottom-2 left-2 rounded bg-black/55 px-1.5 py-0.5 text-[10px] text-slate-200 backdrop-blur-sm">
            Video · {data.ratio}
          </span>
        </div>
      </MediaCard>
      <Handle
        type="source"
        position={Position.Right}
        id="out"
        style={{ background: PORT_TYPE_COLOR.video }}
      />
      {zoom && videoSrc ? <Lightbox src={videoSrc} kind="video" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
