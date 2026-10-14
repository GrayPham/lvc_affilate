// Node "Tạo video MuseAI" (kind `museai_video`) — prompt (bắt buộc, cổng `prompt`) -> 1 video
// (cổng `out`). `account` là id credential MuseAI (`login`) trong kho Tài khoản AI — rỗng = tự
// chọn. Cổng `refs` nhận ảnh tham chiếu dùng chung cho mọi cảnh (xem `node-museai-video/CLAUDE.md` mục
// "Tham số").
// Thân node chỉ là THẺ MEDIA (preview kết quả), khớp `Veo3VideoNode`/`VideoNode` (Dola) — tham số
// đầy đủ ở `components/ApiNodeParamsPanel.tsx` (hiện khi node được chọn). Thanh công cụ nổi dùng
// ĐÚNG bộ hành động của Dola (Phóng to/Cắt/Chụp khung hình/Tách audio/Tách audio↔video/Tải xuống)
// qua `mediaActions.tsx` — review người dùng 2026-10-13: "muse, veo3 đều giống như dola chứ".
import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { useWorkflowUi } from "../store/workflowUi";
import { assetUrl } from "../ipc/tauri";
import Lightbox from "../components/Lightbox";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar from "./NodeFloatingToolbar";
import OutputPreview from "./OutputPreview";
import { buildVideoResultActions } from "./mediaActions";

type MuseaiVideoKind = Extract<NodeKind, { kind: "museai_video" }>;
export type MuseaiVideoRFNode = RFNodeBase<MuseaiVideoKind, "museai_video">;

export default function MuseaiVideoNode({ id, selected }: NodeProps<MuseaiVideoRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const selfPos = useGraphStore((s) => s.nodes.find((n) => n.id === id)?.position);
  const addNodeConnected = useGraphStore((s) => s.addNodeConnected);
  const openCapture = useWorkflowUi((s) => s.openCapture);
  const openTrim = useWorkflowUi((s) => s.openTrim);
  const runState = useRunStore((s) => s.nodes[id]);
  const outputPath = useRunStore((s) => s.nodeVideos[id]);
  const src = assetUrl(outputPath);
  const [zoom, setZoom] = useState(false);

  const actions = buildVideoResultActions({
    nodeId: id,
    nodeLabel: NODE_LABEL_VI.museai_video,
    src,
    path: outputPath,
    selfPos,
    addNodeConnected,
    openTrim,
    openCapture,
    onExpand: () => setZoom(true),
  });

  return (
    <>
      <NodeFloatingToolbar nodeId={id} actions={actions} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ top: "30%", background: PORT_TYPE_COLOR.text }} />
      {/* Ảnh tham chiếu — dùng CHUNG cho mọi cảnh của node, giống cổng `refs` của Veo3. */}
      <Handle type="target" position={Position.Left} id="refs" style={{ top: "70%", background: PORT_TYPE_COLOR.image }} />
      <MediaCard label={NODE_LABEL_VI.museai_video} kind="museai_video" selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="video" path={outputPath} label="Video" />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
      {zoom && src ? <Lightbox src={src} kind="video" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
