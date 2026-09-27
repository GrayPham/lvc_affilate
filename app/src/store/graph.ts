// Store zustand cho canvas Workflow (Task 12): nodes/edges/node dang chon, undo (20 buoc), va
// loi kiem hop le theo node (tu `workflow_validate`, hien qua badge tren tung node). Luu tam
// canvas hien tai vao `localStorage` (debounce) de reload trang khong mat viec — xem `persist()`
// cuoi file.
import { create } from "zustand";
import { addEdge, applyEdgeChanges, applyNodeChanges } from "@xyflow/react";
import type { Connection, EdgeChange, NodeChange } from "@xyflow/react";

import type { RFEdge, RFNode } from "../graph/adapter";
import { fromGraph, migrateGraphJson, toGraph } from "../graph/adapter";
import { isValidConnection as checkValidConnection } from "../graph/rules";
import { connectionForNewNode, type PendingConnect } from "../graph/connect";
import type { AddableNodeKind } from "../graph/ports";
import { GraphSchema, type Graph, type GraphError, type NodeKind } from "../ipc/types";

const MAX_HISTORY = 20;
const STORAGE_KEY = "lvc.workflow.canvas.v1";

/** Du lieu mac dinh khi tha mot loai node moi tu Palette — khop dung cac truong `NodeKind`. */
const DEFAULT_DATA: Record<AddableNodeKind, () => NodeKind> = {
  prompt: () => ({ kind: "prompt", text: "" }),
  image_upload: () => ({ kind: "image_upload", paths: [] }),
  load_video: () => ({ kind: "load_video", paths: [] }),
  dola_video: () => ({
    kind: "dola_video",
    mode: "fast",
    model: "seedance-2.0",
    ratio: "9:16",
    duration: 10,
    logo: "off",
    account: "",
  }),
  concat: () => ({ kind: "concat" }),
  save: () => ({ kind: "save", out_dir: "" }),
  trim: () => ({ kind: "trim", start: 0, end: 0 }),
  frame_capture: () => ({ kind: "frame_capture", time: 0 }),
  script_gen: () => ({
    kind: "script_gen",
    topic: "",
    num_scenes: 3,
    language: "vi",
    style: "",
    system: "",
    credential: "",
    provider: "gemini",
  }),
};

function genId(prefix: string): string {
  const hasRandomUuid =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function";
  const rand = hasRandomUuid ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return `${prefix}_${rand}`;
}

interface HistoryEntry {
  nodes: RFNode[];
  edges: RFEdge[];
}

interface GraphStoreState {
  nodes: RFNode[];
  edges: RFEdge[];
  selectedNodeId: string | null;
  history: HistoryEntry[];
  /** Bộ nhớ tạm cho copy/paste (node + cạnh NỘI BỘ giữa chúng); không lưu ra localStorage. */
  clipboard: { nodes: RFNode[]; edges: RFEdge[] } | null;
  /** Loi `workflow_validate` gan nhat, gom theo nodeId (xem `Workflow.tsx`/`RunBar.tsx`). */
  nodeErrors: Record<string, GraphError[]>;
  /** Loi khong gan duoc voi node nao (vd `empty`) — hien nguyen dang trong RunBar. */
  globalErrors: GraphError[];

  onNodesChange: (changes: NodeChange<RFNode>[]) => void;
  onEdgesChange: (changes: EdgeChange<RFEdge>[]) => void;
  onConnect: (connection: Connection) => void;
  addNode: (kind: AddableNodeKind, position: { x: number; y: number }) => void;
  /**
   * Them node MOI tai `position` va noi ngay voi handle dang keo (`from`) neu tim duoc cong khop
   * kieu (kieu Lumina: keo "+" ra cho trong -> chon node -> da noi san). Neu khong khop kieu thi
   * chi them node, khong tao canh.
   */
  addNodeConnected: (
    kind: AddableNodeKind,
    position: { x: number; y: number },
    from: Omit<PendingConnect, "kind">,
  ) => void;
  updateNodeData: (id: string, data: NodeKind) => void;
  /** Thêm nhiều node "Tải ảnh lên" từ khung hình đã chụp (Lumina) — mỗi ảnh 1 node, xếp dọc cạnh
   * `origin`; chọn các node vừa thêm. */
  addImageNodes: (items: ReadonlyArray<{ path: string; label: string }>, origin: { x: number; y: number }) => void;
  /** Thêm 1 node "Tải video lên" từ video đã cắt (Lumina trim) cạnh `origin`; chọn node vừa thêm. */
  addVideoNode: (path: string, label: string, origin: { x: number; y: number }) => void;
  /** Xoa mot node + MOI canh noi toi/tu no; bo chon neu dang chon node do. */
  deleteNode: (id: string) => void;
  setSelected: (id: string | null) => void;
  /** Chọn tất cả node (Ctrl+A). */
  selectAll: () => void;
  /** Chép các node đang chọn (+ cạnh nội bộ) vào clipboard (Ctrl+C). */
  copySelection: () => void;
  /** Dán clipboard: tạo node id mới, lệch vị trí, chọn các node vừa dán (Ctrl+V). */
  paste: () => void;
  /** Xoá mọi node đang chọn (+ cạnh liên quan) (Delete khi chọn nhiều / sau Ctrl+X). */
  deleteSelection: () => void;
  undo: () => void;
  loadGraph: (graph: Graph) => void;
  resetGraph: () => void;
  setValidation: (errors: GraphError[] | null) => void;
  toGraph: () => Graph;
}

/** Loi gan duoc voi (mot hoac nhieu) node cu the — cac bien the con lai la "toan cuc". */
function nodeIdsOfError(err: GraphError, edges: readonly RFEdge[]): string[] {
  switch (err.type) {
    case "duplicate_node_id":
    case "unsupported":
      return [err.type === "duplicate_node_id" ? err.id : err.node];
    case "unknown_port":
    case "missing_required_input":
    case "multiple_inputs_on_single_port":
    case "scene":
    case "too_many_refs":
      return [err.node];
    case "cycle":
      return [...err.nodes];
    case "type_mismatch": {
      const edge = edges.find((e) => e.id === err.edge);
      return edge ? [edge.source, edge.target].filter((id): id is string => Boolean(id)) : [];
    }
    case "unknown_node":
    case "empty":
      return [];
    default:
      return [];
  }
}

function groupErrors(
  errors: GraphError[],
  edges: readonly RFEdge[],
): { byNode: Record<string, GraphError[]>; global: GraphError[] } {
  const byNode: Record<string, GraphError[]> = {};
  const global: GraphError[] = [];
  for (const err of errors) {
    const ids = nodeIdsOfError(err, edges);
    if (ids.length === 0) {
      global.push(err);
      continue;
    }
    for (const id of ids) {
      (byNode[id] ??= []).push(err);
    }
  }
  return { byNode, global };
}

export const useGraphStore = create<GraphStoreState>((set, get) => {
  /** Day snapshot HIEN TAI (truoc khi mutate) vao history, gioi han `MAX_HISTORY` (20). */
  function pushHistory(): void {
    const { nodes, edges, history } = get();
    const next = [...history, { nodes, edges }];
    set({ history: next.length > MAX_HISTORY ? next.slice(next.length - MAX_HISTORY) : next });
  }

  return {
    nodes: [],
    edges: [],
    selectedNodeId: null,
    history: [],
    clipboard: null,
    nodeErrors: {},
    globalErrors: [],

    onNodesChange: (changes) => {
      const removedIds = new Set(
        changes
          .filter((c): c is Extract<NodeChange<RFNode>, { type: "remove" }> => c.type === "remove")
          .map((c) => c.id),
      );
      const structural = removedIds.size > 0;
      const dragEnd = changes.some((c) => c.type === "position" && c.dragging === false);
      if (structural || dragEnd) {
        pushHistory();
      }
      const nodes = applyNodeChanges(changes, get().nodes);
      const edges =
        removedIds.size > 0
          ? get().edges.filter((e) => !removedIds.has(e.source) && !removedIds.has(e.target))
          : get().edges;
      set({ nodes, edges, nodeErrors: {}, globalErrors: [] });
    },

    onEdgesChange: (changes) => {
      const structural = changes.some((c) => c.type === "remove");
      if (structural) {
        pushHistory();
      }
      set({ edges: applyEdgeChanges(changes, get().edges), nodeErrors: {}, globalErrors: [] });
    },

    onConnect: (connection) => {
      const { nodes, edges } = get();
      if (!checkValidConnection(connection, nodes, edges)) {
        return;
      }
      pushHistory();
      const id = genId("e");
      set({ edges: addEdge({ ...connection, id }, edges), nodeErrors: {}, globalErrors: [] });
    },

    addNode: (kind, position) => {
      pushHistory();
      const id = genId("n");
      const data = DEFAULT_DATA[kind]();
      const node: RFNode = { id, type: kind, position, data };
      set({ nodes: [...get().nodes, node], nodeErrors: {}, globalErrors: [] });
    },

    addNodeConnected: (kind, position, from) => {
      pushHistory();
      const { nodes, edges } = get();
      const fromNode = nodes.find((n) => n.id === from.nodeId);
      const id = genId("n");
      const node: RFNode = { id, type: kind, position, data: DEFAULT_DATA[kind]() };
      const nextNodes = [...nodes, node];
      let nextEdges = edges;
      if (fromNode) {
        const conn = connectionForNewNode(
          { ...from, kind: fromNode.data.kind },
          id,
          kind,
        );
        if (conn && checkValidConnection(conn, nextNodes, edges)) {
          nextEdges = addEdge({ ...conn, id: genId("e") }, edges);
        }
      }
      set({ nodes: nextNodes, edges: nextEdges, nodeErrors: {}, globalErrors: [] });
    },

    updateNodeData: (id, data) => {
      set({
        nodes: get().nodes.map((n) => (n.id === id ? { ...n, type: data.kind, data } : n)),
        nodeErrors: {},
        globalErrors: [],
      });
    },

    addImageNodes: (items, origin) => {
      if (items.length === 0) {
        return;
      }
      pushHistory();
      const kept = get().nodes.map((n) => (n.selected ? { ...n, selected: false } : n));
      const created: RFNode[] = items.map((it, i) => ({
        id: genId("n"),
        type: "image_upload",
        selected: true,
        position: { x: origin.x + 340, y: origin.y + i * 300 },
        data: { kind: "image_upload", paths: [it.path], label: it.label },
      }));
      set({ nodes: [...kept, ...created], nodeErrors: {}, globalErrors: [] });
    },

    addVideoNode: (path, label, origin) => {
      pushHistory();
      const kept = get().nodes.map((n) => (n.selected ? { ...n, selected: false } : n));
      const node: RFNode = {
        id: genId("n"),
        type: "load_video",
        selected: true,
        position: { x: origin.x + 340, y: origin.y + 40 },
        data: { kind: "load_video", paths: [path], label },
      };
      set({ nodes: [...kept, node], nodeErrors: {}, globalErrors: [] });
    },

    deleteNode: (id) => {
      pushHistory();
      set((s) => ({
        nodes: s.nodes.filter((n) => n.id !== id),
        edges: s.edges.filter((e) => e.source !== id && e.target !== id),
        selectedNodeId: s.selectedNodeId === id ? null : s.selectedNodeId,
        nodeErrors: {},
        globalErrors: [],
      }));
    },

    setSelected: (id) => set({ selectedNodeId: id }),

    selectAll: () => {
      set({ nodes: get().nodes.map((n) => ({ ...n, selected: true })) });
    },

    copySelection: () => {
      const { nodes, edges } = get();
      const picked = nodes.filter((n) => n.selected);
      if (picked.length === 0) {
        return;
      }
      const ids = new Set(picked.map((n) => n.id));
      const inner = edges.filter((e) => ids.has(e.source) && ids.has(e.target));
      set({ clipboard: { nodes: picked, edges: inner } });
    },

    paste: () => {
      const { clipboard, nodes, edges } = get();
      if (!clipboard || clipboard.nodes.length === 0) {
        return;
      }
      pushHistory();
      const OFF = 48;
      const idMap = new Map<string, string>();
      const newNodes: RFNode[] = clipboard.nodes.map((n) => {
        const id = genId("n");
        idMap.set(n.id, id);
        return {
          ...n,
          id,
          selected: true,
          position: { x: n.position.x + OFF, y: n.position.y + OFF },
        };
      });
      const newEdges: RFEdge[] = clipboard.edges.map((e) => ({
        ...e,
        id: genId("e"),
        source: idMap.get(e.source) ?? e.source,
        target: idMap.get(e.target) ?? e.target,
      }));
      // Bỏ chọn node cũ để chỉ còn node vừa dán được chọn.
      const kept = nodes.map((n) => (n.selected ? { ...n, selected: false } : n));
      set({
        nodes: [...kept, ...newNodes],
        edges: [...edges, ...newEdges],
        nodeErrors: {},
        globalErrors: [],
      });
    },

    deleteSelection: () => {
      const { nodes, edges } = get();
      const ids = new Set(nodes.filter((n) => n.selected).map((n) => n.id));
      if (ids.size === 0) {
        return;
      }
      pushHistory();
      set({
        nodes: nodes.filter((n) => !ids.has(n.id)),
        edges: edges.filter((e) => !ids.has(e.source) && !ids.has(e.target)),
        selectedNodeId: null,
        nodeErrors: {},
        globalErrors: [],
      });
    },

    undo: () => {
      const { history } = get();
      const prev = history[history.length - 1];
      if (!prev) {
        return;
      }
      set({ nodes: prev.nodes, edges: prev.edges, history: history.slice(0, -1) });
    },

    loadGraph: (graph) => {
      const { nodes, edges } = fromGraph(graph);
      set({ nodes, edges, selectedNodeId: null, history: [], nodeErrors: {}, globalErrors: [] });
    },

    resetGraph: () => {
      set({ nodes: [], edges: [], selectedNodeId: null, history: [], nodeErrors: {}, globalErrors: [] });
      clearPersisted();
    },

    setValidation: (errors) => {
      if (!errors || errors.length === 0) {
        set({ nodeErrors: {}, globalErrors: [] });
        return;
      }
      const { byNode, global } = groupErrors(errors, get().edges);
      set({ nodeErrors: byNode, globalErrors: global });
    },

    toGraph: () => toGraph(get().nodes, get().edges),
  };
});

function hasLocalStorage(): boolean {
  try {
    return typeof localStorage !== "undefined";
  } catch {
    return false;
  }
}

/** Tai canvas da luu (neu co) luc module nay duoc import lan dau — dung dinh dang `Graph` JSON. */
export function loadPersistedGraph(): Graph | null {
  if (!hasLocalStorage()) {
    return null;
  }
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return null;
    }
    // P4a: canvas luu tu ban cu co the con kind `video`/kind P4 da xoa — migrate TRUOC zod.
    return GraphSchema.parse(migrateGraphJson(JSON.parse(raw)));
  } catch {
    return null;
  }
}

function clearPersisted(): void {
  if (!hasLocalStorage()) {
    return;
  }
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // bo qua — vd storage bi khoa (che do rieng tu)
  }
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

/** Luu debounce (500ms) canvas hien tai vao `localStorage` moi khi store doi. */
useGraphStore.subscribe((state) => {
  if (!hasLocalStorage()) {
    return;
  }
  if (saveTimer) {
    clearTimeout(saveTimer);
  }
  saveTimer = setTimeout(() => {
    try {
      const graph = toGraph(state.nodes, state.edges);
      localStorage.setItem(STORAGE_KEY, JSON.stringify(graph));
    } catch {
      // canh thieu handle giua chung sua tren canvas — bo qua, khong lam vo UI.
    }
  }, 500);
});
