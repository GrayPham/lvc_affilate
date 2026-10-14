// Node "Tạo ảnh Veo3 (Google Labs Flow)" (kind `veo3_image`) — prompt (bắt buộc) + ảnh tham chiếu
// (tuỳ chọn, cổng `refs`) -> 1 ảnh (cổng `out`). `account` là id credential Google (`login`) trong
// kho Tài khoản AI — rỗng = tự chọn.
// P-lumina: thân node chỉ còn là THẺ MEDIA (preview kết quả) — tham số đầy đủ chuyển xuống
// `components/ApiNodeParamsPanel.tsx` (hiện khi node được chọn). Thanh công cụ nổi: Phóng to + Tải
// xuống (qua `mediaActions.tsx`, khớp bộ hành động chung — review người dùng 2026-10-13).
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

type Veo3ImageKind = Extract<NodeKind, { kind: "veo3_image" }>;
export type Veo3ImageRFNode = RFNodeBase<Veo3ImageKind, "veo3_image">;

export default function Veo3ImageNode({ id, data, selected }: NodeProps<Veo3ImageRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const selfPos = useGraphStore((s) => s.nodes.find((n) => n.id === id)?.position);
  const addNodeConnected = useGraphStore((s) => s.addNodeConnected);
  const runState = useRunStore((s) => s.nodes[id]);
  const outputPath = useRunStore((s) => s.nodeVideos[id]);
  const src = assetUrl(outputPath);
  const [zoom, setZoom] = useState(false);

  const actions = buildImageResultActions({
    nodeId: id,
    nodeLabel: NODE_LABEL_VI.veo3_image,
    src,
    selfPos,
    addNodeConnected,
    onExpand: () => setZoom(true),
  });

  return (
    <>
      <NodeFloatingToolbar nodeId={id} actions={actions} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ top: "30%", background: PORT_TYPE_COLOR.text }} />
      <Handle type="target" position={Position.Left} id="refs" style={{ top: "70%", background: PORT_TYPE_COLOR.image }} />
      <MediaCard label={NODE_LABEL_VI.veo3_image} selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="image" path={outputPath} label={`Ảnh · ${data.aspect === "landscape" ? "Ngang" : "Dọc"}`} />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
      {zoom && src ? <Lightbox src={src} kind="image" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
