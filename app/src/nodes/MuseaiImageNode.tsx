// Node "Tạo ảnh MuseAI" (kind `museai_image`) — prompt (bắt buộc, cổng `prompt`) -> 1 ảnh tĩnh
// (cổng `out`). `account` là id credential MuseAI (`login`) — CÙNG tài khoản dùng được cho
// `museai_video`. Provider tự gắn hướng dẫn "ảnh tĩnh, không phải video" vào prompt trước khi gửi
// (xem `lvc_museai::protocol::client::build_image_prompt`) — ở đây chỉ cần nhập mô tả chủ đề.
// Thân node chỉ là THẺ MEDIA (preview kết quả), khớp `GrokImageNode`/`MuseaiVideoNode` — tham số
// đầy đủ ở `components/ApiNodeParamsPanel.tsx` (hiện khi node được chọn). Thanh công cụ nổi: Phóng
// to + Tải xuống (qua `mediaActions.tsx`, khớp bộ hành động chung — review người dùng 2026-10-13).
import { useState } from "react";
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { assetUrl } from "../ipc/tauri";
import Lightbox from "../components/Lightbox";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar from "./NodeFloatingToolbar";
import OutputPreview from "./OutputPreview";
import { buildImageResultActions } from "./mediaActions";

type MuseaiImageKind = Extract<NodeKind, { kind: "museai_image" }>;
export type MuseaiImageRFNode = RFNodeBase<MuseaiImageKind, "museai_image">;

export default function MuseaiImageNode({ id, selected }: NodeProps<MuseaiImageRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const selfPos = useGraphStore((s) => s.nodes.find((n) => n.id === id)?.position);
  const addNodeConnected = useGraphStore((s) => s.addNodeConnected);
  const runState = useRunStore((s) => s.nodes[id]);
  const outputPath = useRunStore((s) => s.nodeVideos[id]);
  const src = assetUrl(outputPath);
  const [zoom, setZoom] = useState(false);

  const actions = buildImageResultActions({
    nodeId: id,
    nodeLabel: NODE_LABEL_VI.museai_image,
    src,
    selfPos,
    addNodeConnected,
    onExpand: () => setZoom(true),
  });

  return (
    <>
      <NodeFloatingToolbar nodeId={id} actions={actions} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ top: "35%", background: PORT_TYPE_COLOR.text }} />
      {/* Ảnh tham chiếu để khoá nhân vật/bối cảnh — cùng kiểu cổng `refs` của museai_video. */}
      <Handle type="target" position={Position.Left} id="refs" style={{ top: "65%", background: PORT_TYPE_COLOR.image }} />
      <MediaCard label={NODE_LABEL_VI.museai_image} kind="museai_image" selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="image" path={outputPath} label="Ảnh" />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
      {zoom && src ? <Lightbox src={src} kind="image" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
