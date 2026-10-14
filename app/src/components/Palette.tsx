// Bang node keo tha (Task 12 brief, P4a): cac node trong `ADDABLE_KINDS` keo duoc vao canvas (hien
// tai ca 4 kind); kind nao khong thuoc tap nay se hien mo, khong keo duoc.
// Workflow.tsx doc `DRAG_KIND_MIME` tren `onDrop` cua `<ReactFlow>` de biet loai node vua tha.
import { useState } from "react";

import type { AddableNodeKind } from "../graph/ports";
import {
  ADDABLE_KINDS,
  CATEGORY_LABEL_VI,
  CATEGORY_ORDER,
  NODE_CATEGORY_VI,
  NODE_KIND_ORDER,
  NODE_LABEL_VI,
} from "../graph/ports";
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
  const [query, setQuery] = useState("");

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

  // Loc theo tu tim (khop ten hien thi, khong phan biet hoa/thuong/dau) roi GOM NHOM theo
  // `NODE_CATEGORY_VI` — danh sach kind da dai ~20 muc, liet ke phang khong cuon se tran ra ngoai
  // canvas va kho chon (phan anh nguoi dung: "làm sao mà chọn node").
  const q = query.trim().toLowerCase();
  const filtered = q === "" ? kinds : kinds.filter((k) => NODE_LABEL_VI[k].toLowerCase().includes(q));
  // Danh sach ~20 muc — gom nhom lai moi render (khong can useMemo, chi phi khong dang ke).
  const byCategory = new Map<string, NodeKindTag[]>();
  for (const k of filtered) {
    const cat = NODE_CATEGORY_VI[k];
    (byCategory.get(cat) ?? byCategory.set(cat, []).get(cat)!).push(k);
  }
  const groups = CATEGORY_ORDER.map((cat) => ({ cat, kinds: byCategory.get(cat) ?? [] })).filter(
    (g) => g.kinds.length > 0,
  );

  return (
    <div className="flex max-h-[70vh] w-56 flex-col gap-1.5 rounded-2xl border border-line bg-panel/95 p-2 shadow-2xl backdrop-blur-md">
      <h2 className="px-1 text-[10px] font-semibold uppercase tracking-wide text-ink2">
        {connectFrom ? "Nối tới node" : "Thêm node"}
      </h2>
      <input
        type="search"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Tìm node…"
        className="nodrag w-full rounded-lg border border-line bg-fill px-2 py-1 text-xs text-ink placeholder-slate-500 outline-none focus:border-sky-400/60"
      />
      <div className="flex flex-col gap-2 overflow-y-auto pr-0.5">
        {groups.length === 0 ? (
          <p className="px-1 py-1 text-[11px] text-ink3">
            {kinds.length === 0 ? "Không có node nào nối được cổng này." : "Không khớp node nào."}
          </p>
        ) : (
          groups.map((g) => (
            <div key={g.cat} className="flex flex-col gap-1">
              <span className="px-1 text-[9px] font-semibold uppercase tracking-wider text-ink3">
                {CATEGORY_LABEL_VI[g.cat] ?? g.cat}
              </span>
              {g.kinds.map((kind) => (
                <PaletteItem key={kind} kind={kind} at={at} connectFrom={connectFrom} />
              ))}
            </div>
          ))
        )}
      </div>
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
          ? "cursor-grab border-line bg-fill text-ink hover:bg-fill2 active:cursor-grabbing"
          : "cursor-not-allowed border-dashed border-line bg-fill text-ink3")
      }
    >
      {NODE_LABEL_VI[kind]}
    </div>
  );
}
