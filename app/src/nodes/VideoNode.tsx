// Node "Tạo video Dola" (kind `dola_video`, truoc P4a la `video`).
// P-lumina: THAN node gio gon lai — chi la the media (vung xem truoc video + pill nhan + cham trang
// thai). Cac tham so (che do/model/ti le/thoi luong/logo/tai khoan) da chuyen xuong BANG DUOI khi
// node duoc chon (xem `components/VideoParamsPanel.tsx`) de node gon dep kieu Lumina. Van giu 2 cong
// vao (prompt=text, refs=image) + 1 cong ra (video), mau theo kieu cong; chinh sua van luu qua
// `updateNodeData` cua store nhu cu.
// Thanh cong cu noi them 3 nut: "Tách audio"/"Tách audio↔video" (them node `audio_extract`/
// `audio_split` da NOI SAN tu cong `out` cua video nay, dung `addNodeConnected` — khac trim/frame
// chay ffmpeg NGAY qua bang rieng, hai nut nay chi tao node trong do thi, chay khi bam "Chạy") va
// "Tải xuống" (them node `save` da noi san — tai `check.ps1`/README, day la cach don gian nhat tai
// dung co che co san thay vi phat minh lenh IPC luu-tep moi).
import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { useWorkflowUi } from "../store/workflowUi";
import { assetUrl } from "../ipc/tauri";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar, { type ToolbarAction } from "./NodeFloatingToolbar";
import { useHoverPlay } from "./useHoverPlay";
import Lightbox from "../components/Lightbox";

type VideoKind = Extract<NodeKind, { kind: "dola_video" }>;
export type VideoRFNode = RFNodeBase<VideoKind, "dola_video">;

export default function VideoNode({ id, data, selected }: NodeProps<VideoRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const selfPos = useGraphStore((s) => s.nodes.find((n) => n.id === id)?.position);
  const addNodeConnected = useGraphStore((s) => s.addNodeConnected);
  const openCapture = useWorkflowUi((s) => s.openCapture);
  const openTrim = useWorkflowUi((s) => s.openTrim);
  const runState = useRunStore((s) => s.nodes[id]);
  // Video HOAN TAT cua node nay (neu co) — preview mp4 ket qua (asset:// cua Tauri).
  const videoPath = useRunStore((s) => s.nodeVideos[id]);
  const videoSrc = assetUrl(videoPath);
  const [zoom, setZoom] = useState(false);
  const hp = useHoverPlay();

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
    {
      key: "trim",
      title: videoSrc ? "Cắt video (chọn đoạn → thêm vào canvas)" : "Chưa có video (chạy để tạo)",
      disabled: !videoSrc,
      onClick: () => {
        const base = selfPos ?? { x: 0, y: 0 };
        openTrim({ nodeId: id, src: videoSrc, path: videoPath ?? "", label: NODE_LABEL_VI.dola_video, x: base.x, y: base.y });
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
      title: videoSrc ? "Chụp khung hình (bắt khung → thêm ảnh vào canvas)" : "Chưa có video (chạy để tạo)",
      disabled: !videoSrc,
      onClick: () => {
        const base = selfPos ?? { x: 0, y: 0 };
        openCapture({ nodeId: id, src: videoSrc, label: NODE_LABEL_VI.dola_video, x: base.x, y: base.y });
      },
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" />
          <circle cx="12" cy="13" r="4" />
        </svg>
      ),
    },
    {
      key: "audio-extract",
      title: "Tách audio (thêm node Tách audio, nối từ video này)",
      onClick: () => {
        const base = selfPos ?? { x: 0, y: 0 };
        addNodeConnected(
          "audio_extract",
          { x: base.x + 340, y: base.y - 90 },
          { nodeId: id, handleId: "out", handleType: "source" },
        );
      },
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M9 18V5l12-2v13" />
          <circle cx="6" cy="18" r="3" />
          <circle cx="18" cy="16" r="3" />
        </svg>
      ),
    },
    {
      key: "audio-split",
      title: "Tách audio↔video (thêm node Tách audio↔video, nối từ video này)",
      onClick: () => {
        const base = selfPos ?? { x: 0, y: 0 };
        addNodeConnected(
          "audio_split",
          { x: base.x + 340, y: base.y + 90 },
          { nodeId: id, handleId: "out", handleType: "source" },
        );
      },
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 4v4" />
          <path d="M12 8L6 14" />
          <path d="M12 8l6 6" />
          <path d="M6 14v4" />
          <path d="M18 14v4" />
        </svg>
      ),
    },
    {
      key: "download",
      title: videoSrc ? "Tải xuống (thêm node Lưu, nối từ video này)" : "Chưa có video (chạy để tạo)",
      disabled: !videoSrc,
      onClick: () => {
        const base = selfPos ?? { x: 0, y: 0 };
        addNodeConnected(
          "save",
          { x: base.x + 340, y: base.y + 270 },
          { nodeId: id, handleId: "out", handleType: "source" },
        );
      },
      icon: (
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
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
      <MediaCard label={NODE_LABEL_VI.dola_video} kind="dola_video" selected={selected} errors={errors} runState={runState} wide>
        {/* Vung xem truoc: video ket qua neu da chay xong; chua co thi placeholder toi + glyph play. */}
        <div
          className="relative flex aspect-video w-full items-center justify-center bg-gradient-to-br from-fill2 to-fill"
          {...(videoSrc ? hp.hover : {})}
          title={videoSrc ? "Rê chuột để xem; Phóng to để phát đầy đủ" : undefined}
        >
          {videoSrc ? (
            <video
              ref={hp.videoRef}
              src={videoSrc}
              className="pointer-events-none absolute inset-0 h-full w-full bg-black object-contain"
              muted
              loop
              playsInline
              preload="metadata"
            />
          ) : (
            <span className="flex h-11 w-11 items-center justify-center rounded-full bg-fill2 text-ink">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                <path d="M8 5v14l11-7z" />
              </svg>
            </span>
          )}
          <span className="pointer-events-none absolute bottom-2 left-2 rounded bg-black/55 px-1.5 py-0.5 text-[10px] text-ink backdrop-blur-sm">
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
