// Node "Tạo ảnh Google (API Imagen)" (kind `google_image`) — API CHÍNH THỨC: prompt (bắt buộc) ->
// 1 ảnh (cổng `out`). Người dùng nhập API key Google của chính họ (cùng key Gemini) ở màn Tài khoản
// AI → API key; node chỉ truyền TÊN (label) credential, không thấy key. Khác hẳn node Veo3 (trình
// duyệt Flow) — đây là HTTP thuần.
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

type GoogleImageKind = Extract<NodeKind, { kind: "google_image" }>;
export type GoogleImageRFNode = RFNodeBase<GoogleImageKind, "google_image">;

const ASPECT_LABEL_VI: Record<string, string> = { portrait: "Dọc", landscape: "Ngang", square: "Vuông" };

export default function GoogleImageNode({ id, data, selected }: NodeProps<GoogleImageRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);
  const outputPath = useRunStore((s) => s.nodeVideos[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ background: PORT_TYPE_COLOR.text }} />
      <MediaCard label={NODE_LABEL_VI.google_image} kind="google_image" selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="image" path={outputPath} label={`Ảnh · ${ASPECT_LABEL_VI[data.aspect] ?? data.aspect}`} />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.image }} />
    </>
  );
}
