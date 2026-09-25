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
import type { AddableNodeKind } from "../graph/ports";
import { GraphSchema, type Graph, type GraphError, type NodeKind } from "../ipc/types";

const MAX_HISTORY = 20;
const STORAGE_KEY = "lvc.workflow.canvas.v1";

/** Du lieu mac dinh khi tha mot loai node moi tu Palette — khop dung cac truong `NodeKind`. */
const DEFAULT_DATA: Record<AddableNodeKind, () => NodeKind> = {
  prompt: () => ({ kind: "prompt", text: "" }),
  image_upload: () => ({ kind: "image_upload", paths: [] }),
  dola_video: () => ({
    kind: "dola_video",
    mode: "fast",
    model: "seedance-2.0",
    ratio: "9:16",
    duration: 10,
    logo: "off",
    account: "",
  }),
  save: () => ({ kind: "save", out_dir: "" }),
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
  /** Loi `workflow_validate` gan nhat, gom theo nodeId (xem `Workflow.tsx`/`RunBar.tsx`). */
  nodeErrors: Record<string, GraphError[]>;
  /** Loi khong gan duoc voi node nao (vd `empty`) — hien nguyen dang trong RunBar. */
  globalErrors: GraphError[];

  onNodesChange: (changes: NodeChange<RFNode>[]) => void;
  onEdgesChange: (changes: EdgeChange<RFEdge>[]) => void;
  onConnect: (connection: Connection) => void;
  addNode: (kind: AddableNodeKind, position: { x: number; y: number }) => void;
  updateNodeData: (id: string, data: NodeKind) => void;
  setSelected: (id: string | null) => void;
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

    updateNodeData: (id, data) => {
      set({
        nodes: get().nodes.map((n) => (n.id === id ? { ...n, type: data.kind, data } : n)),
        nodeErrors: {},
        globalErrors: [],
      });
    },

    setSelected: (id) => set({ selectedNodeId: id }),

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
