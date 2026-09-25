// Node "Tạo video Dola" (kind `dola_video`, truoc P4a la `video`).
// P-lumina: THAN node gio gon lai — chi la the media (vung xem truoc video + pill nhan + cham trang
// thai). Cac tham so (che do/model/ti le/thoi luong/logo/tai khoan) da chuyen xuong BANG DUOI khi
// node duoc chon (xem `components/VideoParamsPanel.tsx`) de node gon dep kieu Lumina. Van giu 2 cong
// vao (prompt=text, refs=image) + 1 cong ra (video), mau theo kieu cong; chinh sua van luu qua
// `updateNodeData` cua store nhu cu.
import { Handle, Position, type NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI, PORT_TYPE_COLOR } from "../graph/ports";
import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import MediaCard from "./MediaCard";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type VideoKind = Extract<NodeKind, { kind: "dola_video" }>;
export type VideoRFNode = RFNodeBase<VideoKind, "dola_video">;

export default function VideoNode({ id, data, selected }: NodeProps<VideoRFNode>) {
  const errors = useGraphStore((s) => s.nodeErrors[id]);
  const runState = useRunStore((s) => s.nodes[id]);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
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
        {/* Vung xem truoc video (placeholder toi + glyph play + badge "Video" + ti le hien tai). */}
        <div className="relative flex aspect-video w-full items-center justify-center bg-gradient-to-br from-[#1f1f26] to-[#111114]">
          <span className="flex h-11 w-11 items-center justify-center rounded-full bg-white/10 text-white">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          </span>
          <span className="absolute bottom-2 left-2 rounded bg-black/55 px-1.5 py-0.5 text-[10px] text-slate-200 backdrop-blur-sm">
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
    </>
  );
}
