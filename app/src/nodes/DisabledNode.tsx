// Node chung cho 5 loai P4 (image_from_url/image_gen/remove_logo/ensure_h264/concat,
// `NodeKind::supported_in_p3() == false`) — khong the them tu Palette o P3 (xem Palette.tsx),
// nhung van dang ky trong `nodeTypes` (Workflow.tsx) de mot workflow JSON tu tuong lai (P4) chua
// cac node nay khong lam canvas vo/trang khi mo lai — hien mo, khong sua duoc, dung cong dung
// `PORTS` de canh van ve dung.
import { Handle, Position, type NodeProps } from "@xyflow/react";

import { NODE_LABEL_VI, PORT_TYPE_COLOR, PORTS } from "../graph/ports";
import type { RFNode } from "../graph/adapter";

function handleTop(index: number, total: number): string {
  return `${((index + 1) * 100) / (total + 1)}%`;
}

export default function DisabledNode({ data }: NodeProps<RFNode>) {
  const kind = data.kind;
  const spec = PORTS[kind];

  return (
    <>
      {spec.inputs.map((p, i) => (
        <Handle
          key={p.name}
          type="target"
          position={Position.Left}
          id={p.name}
          style={{ top: handleTop(i, spec.inputs.length), background: PORT_TYPE_COLOR[p.type] }}
        />
      ))}
      <div className="w-44 cursor-not-allowed rounded-lg border border-dashed border-slate-300 bg-slate-100 p-2 opacity-70">
        <div className="flex items-center justify-between gap-2 text-xs font-semibold text-slate-500">
          <span>{NODE_LABEL_VI[kind]}</span>
          <span className="rounded bg-slate-200 px-1 text-[10px]">P4</span>
        </div>
        <p className="mt-1 text-[10px] text-slate-400">Chưa hỗ trợ ở P3</p>
      </div>
      {spec.outputs.map((p, i) => (
        <Handle
          key={p.name}
          type="source"
          position={Position.Right}
          id={p.name}
          style={{ top: handleTop(i, spec.outputs.length), background: PORT_TYPE_COLOR[p.type] }}
        />
      ))}
    </>
  );
}
