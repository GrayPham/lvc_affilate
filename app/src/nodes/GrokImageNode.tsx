// Node "Tạo ảnh Grok (API xAI)" (kind `grok_image`) — API CHÍNH THỨC xAI: prompt (bắt buộc) -> 1
// ảnh (cổng `out`). Người dùng nhập API key xAI; node chỉ truyền TÊN credential. xAI image API
// không nhận tỉ lệ khung nên tham số chỉ có model + credential.
// P-lumina: thân node chỉ còn là THẺ MEDIA (preview kết quả) — tham số đầy đủ chuyển xuống
// `components/ApiNodeParamsPanel.tsx` (hiện khi node được chọn).
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar from "./NodeFloatingToolbar";
import OutputPreview from "./OutputPreview";

type GrokImageKind = Extract<NodeKind, { kind: "grok_image" }>;
export type GrokImageRFNode = RFNodeBase<GrokImageKind, "grok_image">;

export default function GrokImageNode({ id, selected }: NodeProps<GrokImageRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const outputPath = useRunStore((s) => s.nodeVideos[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ background: PORT_TYPE_COLOR.text }} />
      <MediaCard label={NODE_LABEL_VI.grok_image} selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="image" path={outputPath} label="Ảnh" />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
    </>
  );
}
