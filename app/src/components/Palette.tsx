// Bang node keo tha (Task 12 brief, P4a): cac node trong `ADDABLE_KINDS` keo duoc vao canvas (hien
// tai ca 4 kind); kind nao khong thuoc tap nay se hien mo, khong keo duoc.
// Workflow.tsx doc `DRAG_KIND_MIME` tren `onDrop` cua `<ReactFlow>` de biet loai node vua tha.
import type { AddableNodeKind } from "../graph/ports";
import { NODE_KIND_ORDER, NODE_LABEL_VI, ADDABLE_KINDS } from "../graph/ports";
import type { NodeKindTag } from "../ipc/types";
import { useGraphStore } from "../store/graph";
import { useWorkflowUi } from "../store/workflowUi";

/** Kieu MIME tuy dat dung de mang `NodeKindTag` qua HTML5 drag-and-drop. */
export const DRAG_KIND_MIME = "application/lvc-node-kind";

// P-lumina: Palette gio la mot POPUP toi noi ben canh thanh cong cu trai (khong con aside co dinh) —
// bam "+" tren CanvasToolbar de mo. Keo-tha va nhap-de-them van hoat dong nhu cu.
// `at` (toa do FLOW): khi mo bang CHUOT PHAI tren canvas, node moi dat DUNG cho con tro thay vi so le.
export default function Palette({ at }: { at?: { x: number; y: number } }) {
  return (
    <div className="flex w-44 flex-col gap-1.5 rounded-2xl border border-white/10 bg-[#17171a]/95 p-2 shadow-2xl backdrop-blur-md">
      <h2 className="px-1 text-[10px] font-semibold uppercase tracking-wide text-slate-400">Thêm node</h2>
      {NODE_KIND_ORDER.map((kind) => (
        <PaletteItem key={kind} kind={kind} at={at} />
      ))}
    </div>
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

function PaletteItem({ kind, at }: { kind: NodeKindTag; at?: { x: number; y: number } }) {
  const enabled = ADDABLE_KINDS.has(kind);
  const addNode = useGraphStore((s) => s.addNode);
  const nodeCount = useGraphStore((s) => s.nodes.length);
  const closeAddMenu = useWorkflowUi((s) => s.closeAddMenu);

  function add(): void {
    addNode(kind as AddableNodeKind, at ?? nextClickPosition(nodeCount));
    closeAddMenu();
  }

  return (
    <div
      role="button"
      tabIndex={enabled ? 0 : -1}
      draggable={enabled}
      onClick={() => {
        if (enabled) {
          add();
        }
      }}
      onKeyDown={(e) => {
        if (enabled && (e.key === "Enter" || e.key === " ")) {
          e.preventDefault();
          add();
        }
      }}
      onDragStart={(e) => {
        if (!enabled) {
          e.preventDefault();
          return;
        }
        e.dataTransfer.setData(DRAG_KIND_MIME, kind);
        e.dataTransfer.effectAllowed = "move";
        closeAddMenu();
      }}
      title={enabled ? "Nhấp hoặc kéo vào canvas" : "Chưa hỗ trợ"}
      aria-disabled={!enabled}
      className={
        "select-none rounded-lg border px-2 py-1.5 text-xs transition-colors " +
        (enabled
          ? "cursor-grab border-white/10 bg-white/5 text-slate-200 hover:bg-white/10 active:cursor-grabbing"
          : "cursor-not-allowed border-dashed border-white/10 bg-white/[0.02] text-slate-500")
      }
    >
      {NODE_LABEL_VI[kind]}
    </div>
  );
}
