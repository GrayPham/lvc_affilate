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
import { useCallback, useEffect, useRef } from "react";
import type { DragEvent } from "react";
import {
  Background,
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
import RunBar from "../components/RunBar";
import ProgressPanel from "../components/ProgressPanel";
import PromptNode from "../nodes/PromptNode";
import ImageUploadNode from "../nodes/ImageUploadNode";
import VideoNode from "../nodes/VideoNode";
import SaveNode from "../nodes/SaveNode";
import DisabledNode from "../nodes/DisabledNode";
import type { RFEdge, RFNode } from "../graph/adapter";
import { isValidConnection } from "../graph/rules";
import type { AddableNodeKind } from "../graph/ports";
import { loadPersistedGraph, useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
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
  video: VideoNode,
  save: SaveNode,
  image_from_url: DisabledNode,
  image_gen: DisabledNode,
  remove_logo: DisabledNode,
  ensure_h264: DisabledNode,
  concat: DisabledNode,
} as unknown as NodeTypes;

const ADDABLE_KINDS: ReadonlySet<string> = new Set<AddableNodeKind>([
  "prompt",
  "image_upload",
  "video",
  "save",
]);

function isAddableKind(value: string): value is AddableNodeKind {
  return ADDABLE_KINDS.has(value);
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

  const reactFlow = useReactFlow<RFNode, RFEdge>();
  const wrapperRef = useRef<HTMLDivElement>(null);

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
    <div ref={wrapperRef} className="relative flex-1" onDragOver={onDragOver} onDrop={onDrop}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onNodeClick={(_e, node) => setSelected(node.id)}
        onPaneClick={() => setSelected(null)}
        nodeTypes={nodeTypes}
        isValidConnection={handleIsValidConnection}
        fitView
      >
        <Background />
        <Controls />
        <MiniMap pannable zoomable />
      </ReactFlow>
    </div>
  );
}

export default function Workflow() {
  return (
    <div className="flex h-full flex-col">
      <RunBar />
      <div className="flex flex-1 overflow-hidden">
        <Palette />
        <ReactFlowProvider>
          <WorkflowCanvas />
        </ReactFlowProvider>
        <aside className="flex w-80 shrink-0 flex-col overflow-hidden border-l border-slate-200 bg-white">
          <h2 className="border-b border-slate-100 p-2 text-xs font-semibold uppercase tracking-wide text-slate-400">
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
