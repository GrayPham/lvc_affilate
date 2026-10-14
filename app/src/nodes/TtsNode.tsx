// Node "Tạo giọng đọc AI (TTS)" (kind `tts`) — prompt (bắt buộc, cổng `prompt`) -> 1 audio (cổng
// `out`). `credential` là tên/label API key (ElevenLabs/MiniMax) trong kho "API key" — rỗng = tự
// chọn key bật đầu tiên của provider đã chọn. Thân node chỉ là THẺ MEDIA (preview kết quả), khớp
// `MuseaiVideoNode`/`Veo3VideoNode` — tham số đầy đủ ở `components/ApiNodeParamsPanel.tsx` (hiện
// khi node được chọn). Thanh công cụ nổi dùng bộ hành động audio (Phóng to/Tải xuống — không Cắt/
// Chụp khung hình/Tách audio vì nguồn đã là audio, không phải video).
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
import { buildAudioResultActions } from "./mediaActions";

type TtsKind = Extract<NodeKind, { kind: "tts" }>;
export type TtsRFNode = RFNodeBase<TtsKind, "tts">;

export default function TtsNode({ id, selected }: NodeProps<TtsRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const selfPos = useGraphStore((s) => s.nodes.find((n) => n.id === id)?.position);
  const addNodeConnected = useGraphStore((s) => s.addNodeConnected);
  const runState = useRunStore((s) => s.nodes[id]);
  const outputPath = useRunStore((s) => s.nodeVideos[id]);
  const src = assetUrl(outputPath);
  const [zoom, setZoom] = useState(false);

  const actions = buildAudioResultActions({
    nodeId: id,
    nodeLabel: NODE_LABEL_VI.tts,
    src,
    selfPos,
    addNodeConnected,
    onExpand: () => setZoom(true),
  });

  return (
    <>
      <NodeFloatingToolbar nodeId={id} actions={actions} />
      <Handle type="target" position={Position.Left} id="prompt" style={{ background: PORT_TYPE_COLOR.text }} />
      <MediaCard label={NODE_LABEL_VI.tts} kind="tts" selected={selected} errors={errors} runState={runState} wide>
        <OutputPreview kind="audio" path={outputPath} label="Audio" />
      </MediaCard>
      <Handle type="source" position={Position.Right} id="out" style={{ background: PORT_TYPE_COLOR.audio }} />
      {zoom && src ? <Lightbox src={src} kind="audio" onClose={() => setZoom(false)} /> : null}
    </>
  );
}
