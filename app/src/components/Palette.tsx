// Bang node keo tha (Task 12 brief): 4 node P3 keo duoc vao canvas, 5 node P4 hien mo (khong keo
// duoc, `SUPPORTED_IN_P3` la nguon su that duy nhat cho viec nay — dung chung voi `DisabledNode`).
// Workflow.tsx doc `DRAG_KIND_MIME` tren `onDrop` cua `<ReactFlow>` de biet loai node vua tha.
import { NODE_KIND_ORDER, NODE_LABEL_VI, SUPPORTED_IN_P3 } from "../graph/ports";
import type { NodeKindTag } from "../ipc/types";

/** Kieu MIME tuy dat dung de mang `NodeKindTag` qua HTML5 drag-and-drop. */
export const DRAG_KIND_MIME = "application/lvc-node-kind";

export default function Palette() {
  return (
    <aside className="flex w-40 shrink-0 flex-col gap-2 overflow-y-auto border-r border-slate-200 bg-white p-2">
      <h2 className="text-[10px] font-semibold uppercase tracking-wide text-slate-400">Node</h2>
      {NODE_KIND_ORDER.map((kind) => (
        <PaletteItem key={kind} kind={kind} />
      ))}
    </aside>
  );
}

function PaletteItem({ kind }: { kind: NodeKindTag }) {
  const enabled = SUPPORTED_IN_P3.has(kind);

  return (
    <div
      draggable={enabled}
      onDragStart={(e) => {
        if (!enabled) {
          e.preventDefault();
          return;
        }
        e.dataTransfer.setData(DRAG_KIND_MIME, kind);
        e.dataTransfer.effectAllowed = "move";
      }}
      title={enabled ? "Kéo vào canvas" : "Chưa hỗ trợ ở P3"}
      aria-disabled={!enabled}
      className={
        "select-none rounded border px-2 py-1.5 text-xs transition-colors " +
        (enabled
          ? "cursor-grab border-slate-300 bg-white hover:bg-slate-50 active:cursor-grabbing"
          : "cursor-not-allowed border-dashed border-slate-200 bg-slate-50 text-slate-400")
      }
    >
      {NODE_LABEL_VI[kind]}
    </div>
  );
}
