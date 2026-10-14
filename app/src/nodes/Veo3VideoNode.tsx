// Node "Tạo video Veo3" (kind `veo3_video`) — prompt (bắt buộc, t2v/r2v) + ảnh tham chiếu (cổng
// `refs`, chỉ dùng khi mode="r2v", tối đa 6 ảnh) -> 1 video (cổng `out`). `account` là id credential
// Google (`login`) trong kho Tài khoản AI — rỗng = tự chọn.
// P-lumina: thân node chỉ còn là THẺ MEDIA (preview kết quả) — khớp cùng kiểu với `VideoNode`
// (Dola) thay vì nhét hết tham số lên thân node nhỏ trên canvas; tham số đầy đủ chuyển xuống
// `components/ApiNodeParamsPanel.tsx` (hiện khi node được chọn — xem component đó). Thanh công cụ
// nổi dùng ĐÚNG bộ hành động của Dola (`mediaActions.tsx`) — review người dùng 2026-10-13: "muse,
// veo3 đều giống như dola chứ".
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

type Veo3VideoKind = Extract<NodeKind, { kind: "veo3_video" }>;
export type Veo3VideoRFNode = RFNodeBase<Veo3VideoKind, "veo3_video">;

export default function Veo3VideoNode({ id, data, selected }: NodeProps<Veo3VideoRFNode>) {
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
    nodeLabel: NODE_LABEL_VI.veo3_video,
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
      <Handle type="target" position={Position.Left} id="refs" style={{ top: "70%", background: PORT_TYPE_COLOR.image }} />
      <MediaCard label={NODE_LABEL_VI.veo3_video} kind="veo3_video" selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="video" path={outputPath} label={`Video · ${data.aspect === "landscape" ? "Ngang" : "Dọc"}`} />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.video }} />
      {zoom && src ? <Lightbox src={src} kind="video" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
