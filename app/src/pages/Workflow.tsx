// Trang Workflow (Task 12) — canvas React Flow that thay placeholder cua Task 11
// (`WorkflowPage.tsx`, da xoa). Bo cuc: Palette (trai) + canvas (giua) + ProgressPanel (phai),
// RunBar (tren cung). Nghe `engine://event` MOT lan duy nhat o day (khong o RunBar/ProgressPanel)
// vi day la trang duy nhat can no dang mo — tranh dang ky nhieu listener trung nhau.
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
import { listenEngine } from "../ipc/events";

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
  const ingest = useRunStore((s) => s.ingest);

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

  // Dang ky DUY NHAT mot lan cho ca vong doi trang — engine phat su kien cho MOI run dang chay
  // (khong can dang ky lai theo runId), `ingest` tu route theo `type` cua tung EngineEvent.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    listenEngine((event) => ingest(event))
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch((e: unknown) => {
        // Ngoai cua so Tauri that (vd `pnpm dev` mo trong trinh duyet thuong de xem UI nhanh) —
        // khong co kenh Tauri, bo qua thay vi lam vo trang.
        console.warn("khong the lang nghe engine://event:", e);
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [ingest]);

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
