// Node "Tạo video Google (API Veo)" (kind `google_video`) — API CHÍNH THỨC: prompt (bắt buộc) +
// ảnh gốc (cổng `ref`, chỉ dùng khi mode=i2v) -> 1 video (cổng `out`). Người dùng nhập API key
// Google của chính họ; node chỉ truyền TÊN (label) credential. HTTP thuần (khác node Veo3 trình
// duyệt Flow).
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

type GoogleVideoKind = Extract<NodeKind, { kind: "google_video" }>;
export type GoogleVideoRFNode = RFNodeBase<GoogleVideoKind, "google_video">;

const ASPECT_LABEL_VI: Record<string, string> = { portrait: "Dọc", landscape: "Ngang", square: "Vuông" };

export default function GoogleVideoNode({ id, data, selected }: NodeProps<GoogleVideoRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const outputPath = useRunStore((s) => s.nodeVideos[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ top: "30%", background: PORT_TYPE_COLOR.text }} />
      <Handle type="target" position={Position.Left} id="ref" style={{ top: "70%", background: PORT_TYPE_COLOR.image }} />
      <MediaCard label={NODE_LABEL_VI.google_video} kind="google_video" selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="video" path={outputPath} label={`Video · ${ASPECT_LABEL_VI[data.aspect] ?? data.aspect}`} />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
    </>
  );
}
