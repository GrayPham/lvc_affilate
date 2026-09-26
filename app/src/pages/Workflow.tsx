// Trang Workflow (Task 12) — canvas React Flow that thay placeholder cua Task 11
// (`WorkflowPage.tsx`, da xoa). Bo cuc: Palette (trai) + canvas (giua) + ProgressPanel (phai),
// RunBar (tren cung).
//
// Fix-round 1 (review C2): kenh `engine://event` KHONG con dang ky o day — `App.tsx` chi render
// `<ActivePage/>` cho tab dang mo nen doi sang tab khac se unmount trang nay hoan toan, huy dang
// ky va lam mat su kien (ke ca `run_done`); dang ky that su nam o `store/engine_bridge.ts`, goi
// mot lan tu `main.tsx`, song suot vong doi app. Trang nay chi con lo dong bo lai (reconcile)
// trang thai run TU DB moi khi MO trang (xem effect ben duoi) — phong truong hop da bo lo
// `run_done`/cac su kien `task_*` trong luc trang bi unmount.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DragEvent, MouseEvent as ReactMouseEvent } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  useReactFlow,
  type Connection,
  type EdgeTypes,
  type NodeTypes,
  type OnConnectStartParams,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import Palette, { DRAG_KIND_MIME } from "../components/Palette";
import CanvasToolbar from "../components/CanvasToolbar";
import VideoParamsPanel from "../components/VideoParamsPanel";
import NodeLogPanel from "../components/NodeLogPanel";
import CaptureFramesPanel from "../components/CaptureFramesPanel";
import TrimPanel from "../components/TrimPanel";
import RunBar from "../components/RunBar";
import ProgressPanel from "../components/ProgressPanel";
import PromptNode from "../nodes/PromptNode";
import ImageUploadNode from "../nodes/ImageUploadNode";
import LoadVideoNode from "../nodes/LoadVideoNode";
import VideoNode from "../nodes/VideoNode";
import SaveNode from "../nodes/SaveNode";
import ConcatNode from "../nodes/ConcatNode";
import TrimNode from "../nodes/TrimNode";
import FrameCaptureNode from "../nodes/FrameCaptureNode";
import type { RFEdge, RFNode } from "../graph/adapter";
import FlowEdge from "../graph/FlowEdge";
import { isValidConnection } from "../graph/rules";
import { canConnectToNode, type PendingConnect } from "../graph/connect";
import { ADDABLE_KINDS, type AddableNodeKind } from "../graph/ports";
import { loadPersistedGraph, useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { useWorkflowUi } from "../store/workflowUi";
import { runList, taskList } from "../ipc/commands";
import { errorText } from "../ipc/error_text";

// `NodeTypes` (`@xyflow/react`) la `Record<string, ComponentType<NodeProps & {data:any,
// type:any}>>` — mot kieu DONG NHAT (`any` o hai truong) de chua nhieu component KHAC NHAU. Cac
// component o day moi cai lai duoc go kieu HEP theo dung `NodeKind` cua no (vd
// `NodeProps<PromptRFNode>`, xem `nodes/PromptNode.tsx`), nen TS khong tu suy duoc mot object
// chua nhieu kieu he specific nhu vay la khop `NodeTypes` (bien phuong sai giua cac
// `ComponentType` cu the va kieu `any` dong nhat) — ep kieu la cach lam chuan cua React Flow
// trong chinh vi du/README cua ho khi dung custom node duoc go kieu rieng; giu `as unknown as
// NodeTypes` (khong phai `any` bare) chu khong tach le tung entry vi khong lam gon hon.
const nodeTypes: NodeTypes = {
  prompt: PromptNode,
  image_upload: ImageUploadNode,
  load_video: LoadVideoNode,
  dola_video: VideoNode,
  concat: ConcatNode,
  save: SaveNode,
  trim: TrimNode,
  frame_capture: FrameCaptureNode,
} as unknown as NodeTypes;

// Dây "dòng chảy" (đốm sáng chạy theo hướng) — dùng cho mọi cạnh.
const edgeTypes = { flow: FlowEdge } as unknown as EdgeTypes;

function isAddableKind(value: string): value is AddableNodeKind {
  return (ADDABLE_KINDS as ReadonlySet<string>).has(value);
}

function WorkflowCanvas() {
  const nodes = useGraphStore((s) => s.nodes);
  const edges = useGraphStore((s) => s.edges);
  const onNodesChange = useGraphStore((s) => s.onNodesChange);
  const onEdgesChange = useGraphStore((s) => s.onEdgesChange);
  const onConnect = useGraphStore((s) => s.onConnect);
  const addNode = useGraphStore((s) => s.addNode);
  const setSelected = useGraphStore((s) => s.setSelected);
  const loadGraph = useGraphStore((s) => s.loadGraph);
  const addMenuOpen = useWorkflowUi((s) => s.addMenuOpen);
  const closeAddMenu = useWorkflowUi((s) => s.closeAddMenu);
  const openAddMenu = useWorkflowUi((s) => s.openAddMenu);

  const reactFlow = useReactFlow<RFNode, RFEdge>();
  const wrapperRef = useRef<HTMLDivElement>(null);
  // Chuot phai tren canvas: mo menu "Them node" tai con tro. `screen` = vi tri hien popup (px trong
  // wrapper); `flow` = toa do do thi de dat node moi dung cho. `null` = menu mo kieu cu (nut "+").
  const [menuAt, setMenuAt] = useState<{ screen: { x: number; y: number }; flow: { x: number; y: number } } | null>(
    null,
  );
  // Handle dang keo (kieu Lumina): dung de LAM MO node khong noi duoc trong luc keo, va de mo menu
  // "Noi toi node" khi tha ra cho trong. `connectMenuFrom` = nguon giu lai cho menu sau khi tha.
  const [connecting, setConnecting] = useState<PendingConnect | null>(null);
  const [connectMenuFrom, setConnectMenuFrom] = useState<Omit<PendingConnect, "kind"> | null>(null);

  // Mo menu them node tai con tro; `from` != null -> menu "Noi toi node" (node moi noi san voi handle
  // dang keo). Chan menu mac dinh cua WebView2 (Back/Refresh/Inspect) o cho goi.
  const openMenuAt = useCallback(
    (clientX: number, clientY: number, from?: Omit<PendingConnect, "kind"> | null) => {
      const rect = wrapperRef.current?.getBoundingClientRect();
      const screen = { x: clientX - (rect?.left ?? 0), y: clientY - (rect?.top ?? 0) };
      const flow = reactFlow.screenToFlowPosition({ x: clientX, y: clientY });
      setMenuAt({ screen, flow });
      setConnectMenuFrom(from ?? null);
      openAddMenu();
    },
    [reactFlow, openAddMenu],
  );

  const onPaneContextMenu = useCallback(
    (e: ReactMouseEvent | globalThis.MouseEvent) => {
      e.preventDefault();
      openMenuAt(e.clientX, e.clientY);
    },
    [openMenuAt],
  );

  // Nhap dup len NEN canvas (khong phai node/handle/control) -> mo menu "Them node" tai con tro
  // (kieu Lumina). Chi nhan khi target la `.react-flow__pane` de khong dinh toi nhap dup doi ten
  // nhan tren the node. `zoomOnDoubleClick` da tat de khong vua phong to vua mo menu.
  const onPaneDoubleClick = useCallback(
    (e: ReactMouseEvent<HTMLDivElement>) => {
      if ((e.target as HTMLElement).classList.contains("react-flow__pane")) {
        e.preventDefault();
        openMenuAt(e.clientX, e.clientY);
      }
    },
    [openMenuAt],
  );

  // Tai canvas da luu tam (localStorage, xem store/graph.ts) MOT LAN luc trang mo — reload trang
  // khong mat viec dang sua.
  useEffect(() => {
    const persisted = loadPersistedGraph();
    if (persisted) {
      loadGraph(persisted);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Fix-round 1 (review C2, phan 2): moi lan trang nay MO (mount) — kenh su kien that su nam o
  // `store/engine_bridge.ts` (song suot app, xem doc dau file) — dong bo lai run dang theo doi
  // (neu co) tu DB, phong truong hop da bo lo `run_done`/cac su kien `task_*` trong luc trang bi
  // unmount (doi tab). Chi can khi con `runId` nhung `status` van `null` (chua biet ket thuc chua).
  useEffect(() => {
    const { runId, status } = useRunStore.getState();
    if (!runId || status !== null) {
      return;
    }
    Promise.all([runList(20), taskList(runId, 200)])
      .then(([rows, tasks]) => {
        const row = rows.find((r) => r.id === runId) ?? null;
        useRunStore.getState().reconcile(row, tasks);
      })
      .catch((e: unknown) => {
        console.warn("khong the dong bo lai trang thai run tu DB:", errorText(e));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Menu dong (nut "+" tat, hoac bam ra ngoai) -> quen vi tri + nguon noi de lan mo sau bat dau sach.
  useEffect(() => {
    if (!addMenuOpen) {
      setMenuAt(null);
      setConnectMenuFrom(null);
    }
  }, [addMenuOpen]);

  // Phim tat tien thao tac: Ctrl/Cmd + Z (hoan tac) / A (chon tat) / C (chep) / X (cat) / V (dan) /
  // S (luu). Bo qua khi con tro dang trong o nhap (INPUT/TEXTAREA/contenteditable) de khong pha
  // copy/paste/undo cua van ban. Delete/Backspace do React Flow xu ly (deleteKeyCode).
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey) {
        return;
      }
      const t = e.target as HTMLElement | null;
      const inField =
        !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable);
      const k = e.key.toLowerCase();
      if (inField && ["a", "c", "x", "v", "z"].includes(k)) {
        return; // de o nhap tu xu ly
      }
      const g = useGraphStore.getState();
      switch (k) {
        case "z":
          e.preventDefault();
          g.undo();
          break;
        case "a":
          e.preventDefault();
          g.selectAll();
          break;
        case "c":
          g.copySelection();
          break;
        case "x":
          e.preventDefault();
          g.copySelection();
          g.deleteSelection();
          break;
        case "v":
          e.preventDefault();
          g.paste();
          break;
        case "s":
          e.preventDefault();
          window.dispatchEvent(new CustomEvent("lvc:save"));
          break;
        default:
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const onDragOver = useCallback((e: DragEvent<HTMLDivElement>) => {
    if (e.dataTransfer.types.includes(DRAG_KIND_MIME)) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
    }
  }, []);

  const onDrop = useCallback(
    (e: DragEvent<HTMLDivElement>) => {
      const kind = e.dataTransfer.getData(DRAG_KIND_MIME);
      if (!kind || !isAddableKind(kind)) {
        return;
      }
      e.preventDefault();
      const position = reactFlow.screenToFlowPosition({ x: e.clientX, y: e.clientY });
      addNode(kind, position);
    },
    [reactFlow, addNode],
  );

  const handleIsValidConnection = useCallback(
    (c: RFEdge | Connection) => isValidConnection(c, nodes, edges),
    [nodes, edges],
  );

  // Nhap dup len mot DAY NOI -> xoa day do (cach truc quan de "xoa day noi da co san"; ngoai ra
  // chon day roi Delete/Backspace cung xoa — xem `deleteKeyCode` ben duoi).
  const onEdgeDoubleClick = useCallback(
    (_e: ReactMouseEvent, edge: RFEdge) => {
      onEdgesChange([{ type: "remove", id: edge.id }]);
    },
    [onEdgesChange],
  );

  // Bat dau keo tu mot handle -> nho nguon (node/cong/loai) de lam mo node khong noi duoc.
  // Bao ve bang try/catch: mot loi o day KHONG duoc lam ket trang thai keo cua React Flow.
  const onConnectStart = useCallback(
    (_e: unknown, params: OnConnectStartParams) => {
      try {
        const node = params.nodeId ? nodes.find((n) => n.id === params.nodeId) : undefined;
        if (!node || !params.nodeId || !params.handleId || !params.handleType) {
          return;
        }
        setConnecting({
          nodeId: params.nodeId,
          kind: node.data.kind,
          handleId: params.handleId,
          handleType: params.handleType,
        });
      } catch {
        setConnecting(null);
      }
    },
    [nodes],
  );

  // Tha day: neu roi vao CHO TRONG (nen canvas) -> mo menu "Noi toi node" tai do (node moi noi san,
  // kieu Lumina); neu roi vao handle hop le thi `onConnect` da tao canh, khong lam gi them. LUON
  // xoa `connecting` (bo lam mo) va bao ve bang try/catch de khong bao gio ket UI khi tha chuot.
  const onConnectEnd = useCallback(
    (event: MouseEvent | TouchEvent) => {
      const from = connecting;
      setConnecting(null);
      try {
        if (!from) {
          return;
        }
        const target = event.target as HTMLElement | null;
        if (!target || typeof target.className !== "string" || !target.classList.contains("react-flow__pane")) {
          return;
        }
        const point = "clientX" in event ? event : event.changedTouches[0];
        if (!point) {
          return;
        }
        openMenuAt(point.clientX, point.clientY, {
          nodeId: from.nodeId,
          handleId: from.handleId,
          handleType: from.handleType,
        });
      } catch {
        // Nuot loi — mien la khong ket trang thai keo.
      }
    },
    [connecting, openMenuAt],
  );

  // Trong luc keo: id cac node KHONG noi duoc (sai kieu / tu noi / cong da day) — de lam mo bang CSS
  // theo `data-id` (KHONG doi object node → khong lam ket ket noi cua React Flow, review bug "tha
  // chuot loi").
  const dimIds = useMemo(() => {
    if (!connecting) {
      return [] as string[];
    }
    return nodes
      .filter((n) => n.id !== connecting.nodeId && !canConnectToNode(connecting, n, nodes, edges))
      .map((n) => n.id);
  }, [nodes, edges, connecting]);

  const dimCss = useMemo(() => {
    if (dimIds.length === 0) {
      return "";
    }
    const sel = dimIds.map((id) => `.react-flow__node[data-id="${id.replace(/["\\]/g, "\\$&")}"]`).join(",");
    return `${sel}{opacity:.25;filter:grayscale(.5);transition:opacity .15s}`;
  }, [dimIds]);

  return (
    <div
      ref={wrapperRef}
      className="relative flex-1 bg-[#0a0a0b]"
      onDragOver={onDragOver}
      onDrop={onDrop}
      onDoubleClick={onPaneDoubleClick}
      // Chan menu chuot phai mac dinh cua WebView2 tren toan canvas (Back/Refresh/Inspect).
      onContextMenu={(e) => e.preventDefault()}
    >
      {/* Lam mo node khong noi duoc trong luc keo — CSS theo data-id, khong dung vao object node. */}
      {dimCss ? <style>{dimCss}</style> : null}
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onConnectStart={onConnectStart}
        onConnectEnd={onConnectEnd}
        onNodeClick={(_e, node) => setSelected(node.id)}
        onPaneClick={() => {
          setSelected(null);
          setMenuAt(null);
          closeAddMenu();
        }}
        onPaneContextMenu={onPaneContextMenu}
        onEdgeDoubleClick={onEdgeDoubleClick}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        isValidConnection={handleIsValidConnection}
        zoomOnDoubleClick={false}
        deleteKeyCode={["Delete", "Backspace"]}
        multiSelectionKeyCode={["Shift"]}
        selectionOnDrag
        panOnDrag={[1, 2]}
        panActivationKeyCode={["Space", "Control", "Meta"]}
        selectionKeyCode={null}
        colorMode="dark"
        defaultEdgeOptions={{ type: "flow", style: { stroke: "#6b6b74", strokeWidth: 1.5 } }}
        fitView
      >
        <Background variant={BackgroundVariant.Dots} gap={22} size={1.4} color="#2b2b32" bgColor="#0a0a0b" />
        <Controls />
        <MiniMap pannable zoomable maskColor="rgba(10,10,11,0.6)" style={{ background: "#17171a" }} />
      </ReactFlow>

      {/* Thanh cong cu doc trai (P-lumina) noi tren canvas */}
      <div className="absolute left-3 top-1/2 z-10 -translate-y-1/2">
        <CanvasToolbar />
      </div>

      {/* Popup them node (Palette): chuot phai -> tai con tro (menuAt); nut "+" -> canh thanh trai */}
      {addMenuOpen ? (
        menuAt ? (
          <div
            className="absolute z-20"
            style={{ left: menuAt.screen.x, top: menuAt.screen.y }}
          >
            <Palette at={menuAt.flow} connectFrom={connectMenuFrom} />
          </div>
        ) : (
          <div className="absolute left-16 top-1/2 z-20 -translate-y-1/2">
            <Palette />
          </div>
        )
      ) : null}

      {/* Bang tham so duoi — chi hien khi node dola_video duoc chon (component tu an) */}
      <div className="pointer-events-none absolute inset-x-0 bottom-4 z-10 flex justify-center px-4">
        <VideoParamsPanel />
      </div>

      {/* Bang nhat ky chay cua node dang chon (goc duoi-trai) — hien khi node da co trang thai chay */}
      <div className="absolute bottom-4 left-4 z-10">
        <NodeLogPanel />
      </div>
    </div>
  );
}

export default function Workflow() {
  return (
    <div className="flex h-full flex-col bg-[#0a0a0b]">
      <CaptureFramesPanel />
      <TrimPanel />
      <RunBar />
      <div className="flex flex-1 overflow-hidden">
        <ReactFlowProvider>
          <WorkflowCanvas />
        </ReactFlowProvider>
        <aside className="flex w-80 shrink-0 flex-col overflow-hidden border-l border-white/10 bg-[#121216] text-slate-200">
          <h2 className="border-b border-white/10 p-2 text-xs font-semibold uppercase tracking-wide text-slate-400">
            Tiến độ
          </h2>
          <div className="flex-1 overflow-y-auto">
            <ProgressPanel />
          </div>
        </aside>
      </div>
    </div>
  );
}
