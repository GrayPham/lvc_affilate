// Bang node keo tha (Task 12 brief, P4a): cac node trong `ADDABLE_KINDS` keo duoc vao canvas (hien
// tai ca 4 kind); kind nao khong thuoc tap nay se hien mo, khong keo duoc.
// Workflow.tsx doc `DRAG_KIND_MIME` tren `onDrop` cua `<ReactFlow>` de biet loai node vua tha.
import type { AddableNodeKind } from "../graph/ports";
import { NODE_KIND_ORDER, NODE_LABEL_VI, ADDABLE_KINDS } from "../graph/ports";
import { connectionForNewNode, type PendingConnect } from "../graph/connect";
import type { NodeKindTag } from "../ipc/types";
import { useGraphStore } from "../store/graph";
import { useWorkflowUi } from "../store/workflowUi";

/** Kieu MIME tuy dat dung de mang `NodeKindTag` qua HTML5 drag-and-drop. */
export const DRAG_KIND_MIME = "application/lvc-node-kind";

// P-lumina: Palette gio la mot POPUP toi noi ben canh thanh cong cu trai (khong con aside co dinh) —
// bam "+" tren CanvasToolbar de mo. Keo-tha va nhap-de-them van hoat dong nhu cu.
// `at` (toa do FLOW): khi mo bang CHUOT PHAI tren canvas, node moi dat DUNG cho con tro thay vi so le.
// `connectFrom`: khi menu mo do KEO MOT DAY ra cho trong — node moi se noi san voi handle nguon.
export default function Palette({
  at,
  connectFrom,
}: {
  at?: { x: number; y: number };
  connectFrom?: Omit<PendingConnect, "kind"> | null;
}) {
  const nodes = useGraphStore((s) => s.nodes);

  // Khi menu mo do KEO MOT DAY: chi hien cac loai node NOI DUOC voi cong dang keo (yeu cau: "đâu ra
  // các node có thể nối được"). Suy tu kind cua node nguon + khop cong theo kieu (connect.ts).
  let kinds: readonly NodeKindTag[] = NODE_KIND_ORDER;
  if (connectFrom) {
    const fromNode = nodes.find((n) => n.id === connectFrom.nodeId);
    if (fromNode) {
      const from: PendingConnect = { ...connectFrom, kind: fromNode.data.kind };
      kinds = NODE_KIND_ORDER.filter((k) => connectionForNewNode(from, "?", k) !== null);
    }
  }

  return (
    <div className="flex w-44 flex-col gap-1.5 rounded-2xl border border-white/10 bg-[#17171a]/95 p-2 shadow-2xl backdrop-blur-md">
      <h2 className="px-1 text-[10px] font-semibold uppercase tracking-wide text-slate-400">
        {connectFrom ? "Nối tới node" : "Thêm node"}
      </h2>
      {kinds.length === 0 ? (
        <p className="px-1 py-1 text-[11px] text-slate-500">Không có node nào nối được cổng này.</p>
      ) : (
        kinds.map((kind) => <PaletteItem key={kind} kind={kind} at={at} connectFrom={connectFrom} />)
      )}
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

function PaletteItem({
  kind,
  at,
  connectFrom,
}: {
  kind: NodeKindTag;
  at?: { x: number; y: number };
  connectFrom?: Omit<PendingConnect, "kind"> | null;
}) {
  const enabled = ADDABLE_KINDS.has(kind);
  const addNode = useGraphStore((s) => s.addNode);
  const addNodeConnected = useGraphStore((s) => s.addNodeConnected);
  const nodeCount = useGraphStore((s) => s.nodes.length);
  const closeAddMenu = useWorkflowUi((s) => s.closeAddMenu);

  function add(): void {
    const position = at ?? nextClickPosition(nodeCount);
    if (connectFrom) {
      addNodeConnected(kind as AddableNodeKind, position, connectFrom);
    } else {
      addNode(kind as AddableNodeKind, position);
    }
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
