// Node "Ghi chú" (crates/nodes/node-note) — annotation tinh tren canvas, KHONG co cong vao/ra (xem
// CLAUDE.md node-note). Dung de ghi luu y/cach dung/cac buoc setup truoc khi chay mot template, de
// nguoi load template sau doc node nay la dung duoc ngay, khong can hoi lai nguoi tao. Khac mau
// (vang, kieu sticky-note) de phan biet ro voi cac node chuc nang khac tren canvas.
import type { NodeProps } from "@xyflow/react";
import type { Node as RFNodeBase } from "@xyflow/react";

import type { NodeKind } from "../ipc/types";
import { useGraphStore } from "../store/graph";
import NodeFloatingToolbar from "./NodeFloatingToolbar";

type NoteKind = Extract<NodeKind, { kind: "note" }>;
export type NoteRFNode = RFNodeBase<NoteKind, "note">;

export default function NoteNode({ id, data, selected }: NodeProps<NoteRFNode>) {
  const updateNodeData = useGraphStore((s) => s.updateNodeData);

  return (
    <>
      <NodeFloatingToolbar nodeId={id} />
      <div
        className={
          "nowheel w-64 rounded-2xl border bg-amber-200/95 p-2.5 text-slate-900 shadow-lg transition-colors dark:bg-amber-100/90 " +
          (selected ? "border-amber-500" : "border-amber-400/60")
        }
      >
        <div className="mb-1.5 flex items-center gap-1.5">
          <span aria-hidden className="text-sm leading-none">
            📝
          </span>
          <span className="text-[11px] font-semibold uppercase tracking-wide text-amber-800">Ghi chú</span>
        </div>
        <textarea
          className="nodrag h-32 w-full resize-y rounded-md border border-amber-400/50 bg-amber-50/80 p-1.5 text-xs text-slate-900 placeholder-amber-700/50 outline-none focus:border-amber-600"
          value={data.text}
          placeholder={"Lưu ý / cách dùng / các bước setup template...\nVí dụ:\n1. Điền API key ở node Tạo video\n2. Chọn thư mục lưu ở node Lưu\n3. Đổi chủ đề ở node Kịch bản AI rồi bấm Chạy"}
          onChange={(e) => updateNodeData(id, { ...data, text: e.target.value })}
        />
      </div>
    </>
  );
}
