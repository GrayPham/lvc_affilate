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
import { useCallback, useEffect, useRef, useState } from "react";
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
  type NodeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import Palette, { DRAG_KIND_MIME } from "../components/Palette";
import CanvasToolbar from "../components/CanvasToolbar";
import VideoParamsPanel from "../components/VideoParamsPanel";
import RunBar from "../components/RunBar";
import ProgressPanel from "../components/ProgressPanel";
import PromptNode from "../nodes/PromptNode";
import ImageUploadNode from "../nodes/ImageUploadNode";
import VideoNode from "../nodes/VideoNode";
import SaveNode from "../nodes/SaveNode";
import type { RFEdge, RFNode } from "../graph/adapter";
import { isValidConnection } from "../graph/rules";
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
  dola_video: VideoNode,
  save: SaveNode,
} as unknown as NodeTypes;

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

  // Mo menu them node tai vi tri chuot phai; chan menu mac dinh cua WebView2 (Back/Refresh/Inspect).
  const openMenuAt = useCallback(
    (clientX: number, clientY: number) => {
      const rect = wrapperRef.current?.getBoundingClientRect();
      const screen = { x: clientX - (rect?.left ?? 0), y: clientY - (rect?.top ?? 0) };
      const flow = reactFlow.screenToFlowPosition({ x: clientX, y: clientY });
      setMenuAt({ screen, flow });
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

  // Menu dong (nut "+" tat, hoac bam ra ngoai) -> quen vi tri chuot phai de lan mo sau bat dau sach.
  useEffect(() => {
    if (!addMenuOpen) {
      setMenuAt(null);
    }
  }, [addMenuOpen]);

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

  return (
    <div
      ref={wrapperRef}
      className="relative flex-1 bg-[#0a0a0b]"
      onDragOver={onDragOver}
      onDrop={onDrop}
      // Chan menu chuot phai mac dinh cua WebView2 tren toan canvas (Back/Refresh/Inspect).
      onContextMenu={(e) => e.preventDefault()}
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onNodeClick={(_e, node) => setSelected(node.id)}
        onPaneClick={() => {
          setSelected(null);
          setMenuAt(null);
          closeAddMenu();
        }}
        onPaneContextMenu={onPaneContextMenu}
        nodeTypes={nodeTypes}
        isValidConnection={handleIsValidConnection}
        colorMode="dark"
        defaultEdgeOptions={{ type: "default", style: { stroke: "#8b8b93", strokeWidth: 1.5 } }}
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
            <Palette at={menuAt.flow} />
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
    </div>
  );
}

export default function Workflow() {
  return (
    <div className="flex h-full flex-col bg-[#0a0a0b]">
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
