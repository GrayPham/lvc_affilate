// Bang node keo tha (Task 12 brief, P4a): cac node trong `ADDABLE_KINDS` keo duoc vao canvas (hien
// tai ca 4 kind); kind nao khong thuoc tap nay se hien mo, khong keo duoc.
// Workflow.tsx doc `DRAG_KIND_MIME` tren `onDrop` cua `<ReactFlow>` de biet loai node vua tha.
import type { AddableNodeKind } from "../graph/ports";
import { NODE_KIND_ORDER, NODE_LABEL_VI, ADDABLE_KINDS } from "../graph/ports";
import type { NodeKindTag } from "../ipc/types";
import { useGraphStore } from "../store/graph";

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

/**
 * Vi tri khi THEM BANG NHAP CHUOT (phuong an du phong cho keo-tha): so le theo so node hien co de
 * cac node moi khong chong len nhau — nguoi dung keo lai sau. Keo-tha HTML5 chi chay trong WebView2
 * khi cua so Tauri tat `dragDropEnabled` (tauri.conf.json); nhap chuot luon chay.
 */
function nextClickPosition(count: number): { x: number; y: number } {
  const step = 40;
  return { x: 80 + (count % 8) * step, y: 80 + (count % 8) * step };
}

function PaletteItem({ kind }: { kind: NodeKindTag }) {
  const enabled = ADDABLE_KINDS.has(kind);
  const addNode = useGraphStore((s) => s.addNode);
  const nodeCount = useGraphStore((s) => s.nodes.length);

  return (
    <div
      role="button"
      tabIndex={enabled ? 0 : -1}
      draggable={enabled}
      onClick={() => {
        if (enabled) {
          addNode(kind as AddableNodeKind, nextClickPosition(nodeCount));
        }
      }}
      onKeyDown={(e) => {
        if (enabled && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          addNode(kind as AddableNodeKind, nextClickPosition(nodeCount));
        }
      }}
      onDragStart={(e) => {
        if (!enabled) {
          e.preventDefault();
          return;
        }
        e.dataTransfer.setData(DRAG_KIND_MIME, kind);
        e.dataTransfer.effectAllowed = "move";
      }}
      title={enabled ? "Nhấp hoặc kéo vào canvas" : "Chưa hỗ trợ"}
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
