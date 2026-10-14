// Store zustand cho MOT canvas Workflow (Task 12, P-tabs: tach thanh FACTORY de moi tab workflow co
// MOT instance rieng — nodes/edges/node dang chon, undo (20 buoc), loi kiem hop le theo node (tu
// `workflow_validate`, hien qua badge tren tung node) deu KHONG con dung chung giua cac tab).
// `createGraphStore()` tao MOT instance; component doc/ghi qua hook `useGraphStore` (doc dung
// `GraphStoreContext` — xem `GraphStoreProvider`, mount boi `components/WorkflowTabs.tsx`, MOI tab
// MOT provider boc quanh `<Workflow/>` cua tab do) — giu NGUYEN chu ky goi cu (`useGraphStore((s) =>
// s.nodes)`) o MOI noi khac trong app, chi doi NOI lay store instance.
//
// Tu luu CA PHIEN lam viec (moi tab) vao `localStorage` — nguon ghi DUY NHAT la `WorkflowTabs`
// `WorkflowTabs.tsx` chi bat co dung MOT tab dau tien/mac dinh) de reload trang khong mat viec —
// nhieu tab cung luu chung MOT khoa se de ghi de lan nhau, khong co y nghia.
import { createContext, useContext, type ReactNode } from "react";
import { create, useStore, type StoreApi, type UseBoundStore } from "zustand";
import { addEdge, applyEdgeChanges, applyNodeChanges } from "@xyflow/react";
import type { Connection, EdgeChange, NodeChange } from "@xyflow/react";

import type { RFEdge, RFNode } from "../graph/adapter";
import { fromGraph, migrateGraphJson, toGraph } from "../graph/adapter";
import { autoLayout } from "../graph/autoLayout";
import { isValidConnection as checkValidConnection } from "../graph/rules";
import { connectionForNewNode, type PendingConnect } from "../graph/connect";
import type { AddableNodeKind } from "../graph/ports";
import { GraphSchema, type Graph, type GraphError, type NodeKind } from "../ipc/types";

const MAX_HISTORY = 20;
const STORAGE_KEY = "lvc.workflow.canvas.v1";

/** Du lieu mac dinh khi tha mot loai node moi tu Palette — khop dung cac truong `NodeKind`. */
const DEFAULT_DATA: Record<AddableNodeKind, () => NodeKind> = {
  prompt: () => ({ kind: "prompt", text: "" }),
  note: () => ({ kind: "note", text: "" }),
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
  audio_extract: () => ({ kind: "audio_extract", format: "mp3" }),
  audio_split: () => ({ kind: "audio_split" }),
  script_gen: () => ({
    kind: "script_gen",
    topic: "",
    num_scenes: 3,
    language: "vi",
    style: "",
    system: "",
    credential: "",
    provider: "gemini",
    save_as_script: false,
    script_name: "",
  }),
  script_agent: () => ({
    kind: "script_agent",
    topic: "",
    num_scenes: 4,
    language: "vi",
    style: "",
    extra_instructions: "",
    credential: "",
    provider: "gemini",
    save_as_script: false,
    script_name: "",
  }),
  veo3_image: () => ({
    kind: "veo3_image",
    model: "GEM_PIX_2",
    aspect: "portrait",
    seed: null,
    account: "",
  }),
  veo3_video: () => ({
    kind: "veo3_video",
    mode: "t2v",
    model: "",
    aspect: "portrait",
    seed_count: 1,
    account: "",
  }),
  google_image: () => ({
    kind: "google_image",
    model: "",
    aspect: "portrait",
    credential: "",
  }),
  google_video: () => ({
    kind: "google_video",
    mode: "t2v",
    model: "",
    aspect: "landscape",
    credential: "",
  }),
  seedance_image: () => ({
    kind: "seedance_image",
    model: "",
    aspect: "square",
    credential: "",
  }),
  seedance_video: () => ({
    kind: "seedance_video",
    mode: "t2v",
    model: "",
    aspect: "landscape",
    credential: "",
  }),
  grok_image: () => ({
    kind: "grok_image",
    model: "",
    credential: "",
  }),
  // Khung rong — khong trong ADDABLE_KINDS nen factory nay khong thuc su duoc goi tu Palette,
  // chi can co de thoa man kieu `Record<AddableNodeKind, ...>` (xem graph/ports.ts).
  museai_video: () => ({ kind: "museai_video", account: "", aspect: "", duration: null }),
  museai_image: () => ({ kind: "museai_image", account: "" }),
  tts: () => ({
    kind: "tts",
    provider: "elevenlabs",
    voice_id: "",
    model: "",
    speed: 1,
    credential: "",
  }),
};

function genId(prefix: string): string {
  const hasRandomUuid =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function";
  const rand = hasRandomUuid ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return `${prefix}_${rand}`;
}

/** Id ephemeral cho mot workflow CHUA tung `workflowSave` — dung chung boi `WorkflowTabs.tsx` (tao
 * tab moi) va `RunBar.tsx` (nut "Mới"). Chuyen tu `RunBar.tsx` sang day (P-tabs) vi ca hai noi deu
 * can. */
export function newEphemeralWorkflowId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `wf-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

interface HistoryEntry {
  nodes: RFNode[];
  edges: RFEdge[];
}

export interface GraphStoreState {
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
  /** P-tabs: id/ten workflow cua TAB nay — chuyen tu `useState` cuc bo cua `RunBar.tsx` sang day de
   * `WorkflowTabs.tsx` (chu cua thanh tab) doc duoc PHAN ANH ten tab theo thoi gian thuc (Chrome-
   * style: doi ten/Luu/Mo deu cap nhat nhan tab ngay). */
  workflowId: string;
  workflowName: string;
  /** `true` = workflow nay DA co trong "Workflow của tôi" (bang `workflows`), nen `workflowId` la
   * id THAT chu khong phai id tam cua tab moi. Ctrl+S dung no de biet luu de len ban cu hay phai
   * hoi ten; doi ten dung no de biet co can ghi lai vao DB hay khong.
   *
   * Khong suy ra tu hinh dang `workflowId` duoc: id tam cung la UUID y het id da luu. */
  workflowSaved: boolean;
  setWorkflowMeta: (id: string, name: string, saved?: boolean) => void;
  /** Mẫu CỘNG ĐỒNG mà đồ thị trên canvas này đến từ — `null` khi người dùng tự dựng. Ghi lúc bấm
   * "Dùng mẫu", đọc lúc lưu workflow để gọi `workflowSetOrigin`; nhờ đó về sau app biết mẫu gốc
   * có bản mới hay chưa (xem `crates/lvc-app/src/commands/updates.rs`). Chỉ giữ trong bộ nhớ của
   * tab — nguồn sự thật là bảng `workflow_origin` sau khi đã lưu. */
  templateOrigin: { sourceId: string; sourceVersion: string } | null;
  setTemplateOrigin: (origin: { sourceId: string; sourceVersion: string } | null) => void;

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
    /** Ghi de len tren `DEFAULT_DATA[kind]()` — dung khi can nhet san du lieu ban dau (vd
     * `PromptField` tu tao node Prompt + nhet luon chu nguoi dung vua go, khong de node moi rong). */
    dataOverride?: Record<string, unknown>,
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
  /** Tự sắp xếp node theo tầng trái→phải (xem `graph/autoLayout.ts`). Vào lịch sử nên Ctrl+Z
   * hoàn tác được — xếp lại cả canvas là thay đổi lớn, phải lùi được. */
  autoArrange: () => void;
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

export type GraphStoreApi = UseBoundStore<StoreApi<GraphStoreState>>;

/** Tao MOT instance store canvas — moi tab workflow goi ham nay MOT lan (xem `WorkflowTabs.tsx`).
 *
 * Store nay KHONG tu ghi `localStorage`: `WorkflowTabs` gop moi tab lai roi ghi mot lan (xem
 * [`saveSession`]) — hai nguon ghi vao cung mot khoa se de ghi de len nhau. */
export function createGraphStore(
  initialMeta: { workflowId: string; workflowName: string; workflowSaved?: boolean },
): GraphStoreApi {
  const store = create<GraphStoreState>((set, get) => {
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
      workflowId: initialMeta.workflowId,
      workflowName: initialMeta.workflowName,
      workflowSaved: initialMeta.workflowSaved ?? false,
      setWorkflowMeta: (id, name, saved) =>
        set({
          workflowId: id,
          workflowName: name,
          // Khong truyen `saved` = giu nguyen co hien tai (vd chi doi ten mot workflow da luu).
          ...(saved === undefined ? {} : { workflowSaved: saved }),
        }),
      templateOrigin: null,
      setTemplateOrigin: (templateOrigin) => set({ templateOrigin }),

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

      addNodeConnected: (kind, position, from, dataOverride) => {
        pushHistory();
        const { nodes, edges } = get();
        const fromNode = nodes.find((n) => n.id === from.nodeId);
        const id = genId("n");
        const baseData = DEFAULT_DATA[kind]();
        const node: RFNode = {
          id,
          type: kind,
          position,
          data: dataOverride ? ({ ...baseData, ...dataOverride } as typeof baseData) : baseData,
        };
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

      autoArrange: () => {
        const { nodes, edges } = get();
        if (nodes.length === 0) {
          return;
        }
        pushHistory();
        set({ nodes: autoLayout(nodes, edges) });
      },

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
        // Nạp một đồ thị khác = canvas không còn là mẫu cộng đồng cũ nữa. Người gọi nào biết
        // nguồn gốc (TemplateGallery) tự gọi `setTemplateOrigin` NGAY SAU lời gọi này.
        set({
          nodes,
          edges,
          selectedNodeId: null,
          history: [],
          nodeErrors: {},
          globalErrors: [],
          templateOrigin: null,
        });
      },

      resetGraph: () => {
        set({ nodes: [], edges: [], selectedNodeId: null, history: [], nodeErrors: {}, globalErrors: [], templateOrigin: null });
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

  return store;
}

// ---------------------------------------------------------------------------------------------
// Context — moi tab workflow boc `<Workflow/>` trong MOT `GraphStoreProvider` rieng (xem
// `WorkflowTabs.tsx`). MOI component khac trong app goi `useGraphStore((s) => s.x)`/
// `useGraphStoreApi()` y HET truoc day — chi khac o cho instance thuc su lay tu context thay vi
// mot bien module-level duy nhat.
// ---------------------------------------------------------------------------------------------

const GraphStoreContext = createContext<GraphStoreApi | null>(null);

export function GraphStoreProvider({ store, children }: { store: GraphStoreApi; children: ReactNode }) {
  return <GraphStoreContext.Provider value={store}>{children}</GraphStoreContext.Provider>;
}

/** Instance store (khong qua selector) — dung cho noi can `.getState()`/`.setState()` truc tiep
 * (vd trong mot handler, khong phai luc render) thay vi `useGraphStore.getState()` cu (khong con
 * ton tai mot bien module-level duy nhat de goi kieu do nua). */
export function useGraphStoreApi(): GraphStoreApi {
  const store = useContext(GraphStoreContext);
  if (!store) {
    throw new Error("useGraphStoreApi: thieu GraphStoreProvider bao ngoai (xem WorkflowTabs.tsx)");
  }
  return store;
}

export function useGraphStore<T>(selector: (state: GraphStoreState) => T): T {
  const store = useGraphStoreApi();
  return useStore(store, selector);
}

function hasLocalStorage(): boolean {
  try {
    return typeof localStorage !== "undefined";
  } catch {
    return false;
  }
}

/** Canvas da luu tam: do thi + DANH TINH workflow.
 *
 * Luu ca danh tinh (khong chi do thi) vi neu thieu, mo lai app se ra mot canvas "chua dat ten"
 * mang dung noi dung cu — bam Luu se tao them MOT workflow moi thay vi ghi de ban dang mo. */
export interface PersistedCanvas {
  graph: Graph;
  workflowId: string;
  workflowName: string;
  workflowSaved: boolean;
}

/** Anh chup MOT canvas tu state hien tai cua store. */
export function snapshotCanvas(state: GraphStoreState): PersistedCanvas {
  return {
    graph: toGraph(state.nodes, state.edges),
    workflowId: state.workflowId,
    workflowName: state.workflowName,
    workflowSaved: state.workflowSaved,
  };
}

/** Khoa luu CA PHIEN lam viec (moi tab dang mo + tab dang xem). */
const SESSION_KEY = "lvc.workflow.session.v1";

/** Phien lam viec: danh sach tab + tab dang xem. */
export interface PersistedSession {
  tabs: PersistedCanvas[];
  activeIndex: number;
}

/** Giai ma MOT canvas tu JSON tho; `null` khi hong. Dung chung cho ca khoa cu lan khoa phien. */
function decodeCanvas(parsed: unknown): PersistedCanvas | null {
  try {
    const hasMeta = parsed !== null && typeof parsed === "object" && "graph" in (parsed as object);
    const rawGraph = hasMeta ? (parsed as { graph: unknown }).graph : parsed;
    // P4a: canvas luu tu ban cu co the con kind `video`/kind P4 da xoa — migrate TRUOC zod.
    const graph = GraphSchema.parse(migrateGraphJson(rawGraph));
    if (!hasMeta) {
      return { graph, workflowId: newEphemeralWorkflowId(), workflowName: "", workflowSaved: false };
    }
    const meta = parsed as { workflowId?: unknown; workflowName?: unknown; workflowSaved?: unknown };
    return {
      graph,
      workflowId:
        typeof meta.workflowId === "string" && meta.workflowId !== ""
          ? meta.workflowId
          : newEphemeralWorkflowId(),
      workflowName: typeof meta.workflowName === "string" ? meta.workflowName : "",
      workflowSaved: meta.workflowSaved === true,
    };
  } catch {
    return null;
  }
}

/** Ghi CA phien. Goi tu `WorkflowTabs` (nguon ghi DUY NHAT) sau khi gop thay doi. */
export function saveSession(session: PersistedSession): void {
  if (!hasLocalStorage()) {
    return;
  }
  try {
    localStorage.setItem(SESSION_KEY, JSON.stringify(session));
  } catch {
    // vd storage day/bi khoa — bo qua, khong lam vo UI.
  }
}

/**
 * Doc phien lam viec lan truoc. `null` = chua co phien nao.
 *
 * Tu di cu tu khoa CU (mot canvas duy nhat) de ban da cai tu truoc mo len van thay dung canvas
 * dang lam — ho khong phai biet gi ve viec dinh dang luu da doi.
 */
export function loadSession(): PersistedSession | null {
  if (!hasLocalStorage()) {
    return null;
  }
  try {
    const raw = localStorage.getItem(SESSION_KEY);
    if (raw) {
      const parsed: unknown = JSON.parse(raw);
      const rawTabs = (parsed as { tabs?: unknown })?.tabs;
      if (Array.isArray(rawTabs)) {
        // Tab nao hong thi BO rieng tab do, khong vut ca phien.
        const tabs = rawTabs.map(decodeCanvas).filter((c): c is PersistedCanvas => c !== null);
        if (tabs.length > 0) {
          const rawIndex = (parsed as { activeIndex?: unknown }).activeIndex;
          const activeIndex =
            typeof rawIndex === "number" && rawIndex >= 0 && rawIndex < tabs.length ? rawIndex : 0;
          return { tabs, activeIndex };
        }
      }
    }
  } catch {
    // roi xuong khoa cu
  }
  const old = loadPersistedCanvas();
  return old === null ? null : { tabs: [old], activeIndex: 0 };
}

/** Tai canvas da luu (neu co) — dung LUC TAO tab "chinh" (`persist: true`, xem `WorkflowTabs.tsx`).
 *
 * Chap nhan CA dinh dang cu (chi mot `Graph` tran) de ban da cai tu truoc khong mat canvas khi
 * nang cap: khi do coi nhu workflow chua dat ten, chua luu. */
export function loadPersistedCanvas(): PersistedCanvas | null {
  if (!hasLocalStorage()) {
    return null;
  }
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return null;
    }
    return decodeCanvas(JSON.parse(raw));
  } catch {
    return null;
  }
}

