// Doi qua lai giua do thi React Flow (canvas Task 12) va `Graph` cua Rust
// (crates/lvc-core/src/graph/mod.rs — dinh dang file workflow JSON, cung la tham so/ket qua cac
// lenh IPC `workflow_validate`/`workflow_save`/`run_start`).
//
// Vi tri ve (`Node.x`/`Node.y` — hai truong PHANG, KHONG long trong `position`, xem node.rs) map
// thang sang `RFNode.position.{x,y}` cua React Flow — Graph cua Rust KHONG co truong nao khac
// ngoai `id, kind, x, y` nen KHONG can them mot map UI-only rieng (`ui: Record<id,{x,y}>`) nhu
// brief co luu y phong khi Rust thieu vi tri.
//
// `RFNode.data` mang THANG `NodeKind` (bao gom ca tham so rieng cua node, vd `Video.mode`) — mot
// kieu duy nhat cho ca hai chieu, tranh lech field khi Task 12 doc/ghi `data` tren canvas.
import type { Edge as RFEdgeBase, Node as RFNodeBase } from "@xyflow/react";
import type { Edge, Graph, Node as GraphNode, NodeKind, NodeKindTag } from "../ipc/types";

export type RFNodeData = NodeKind;
export type RFNode = RFNodeBase<RFNodeData, NodeKindTag>;
export type RFEdge = RFEdgeBase;

/** `Graph.version` hien tai (P3) — xem doc `Graph` (mod.rs): "danh cho migrate ve sau". */
const GRAPH_VERSION = 1;

/** React Flow (canvas) -> `Graph` (Rust, gui qua IPC). */
export function toGraph(nodes: readonly RFNode[], edges: readonly RFEdge[]): Graph {
  return {
    version: GRAPH_VERSION,
    nodes: nodes.map(rfNodeToGraphNode),
    edges: edges.map(rfEdgeToGraphEdge),
  };
}

function rfNodeToGraphNode(n: RFNode): GraphNode {
  return {
    id: n.id,
    kind: n.data,
    x: n.position.x,
    y: n.position.y,
  };
}

function rfEdgeToGraphEdge(e: RFEdge): Edge {
  const fromPort = e.sourceHandle;
  const toPort = e.targetHandle;
  if (!fromPort || !toPort) {
    throw new Error(
      `canh "${e.id}" thieu sourceHandle/targetHandle — khong the doi sang Edge cua Graph ` +
        "(moi cong React Flow phai co ten tay cam khop PortSpec.name)",
    );
  }
  return {
    id: e.id,
    from: e.source,
    from_port: fromPort,
    to: e.target,
    to_port: toPort,
  };
}

/** `Graph` (Rust, doc tu `workflow_get`) -> React Flow (canvas). */
export function fromGraph(g: Graph): { nodes: RFNode[]; edges: RFEdge[] } {
  return {
    nodes: g.nodes.map(graphNodeToRfNode),
    edges: g.edges.map(graphEdgeToRfEdge),
  };
}

function graphNodeToRfNode(n: GraphNode): RFNode {
  return {
    id: n.id,
    type: n.kind.kind,
    position: { x: n.x, y: n.y },
    data: n.kind,
  };
}

function graphEdgeToRfEdge(e: Edge): RFEdge {
  return {
    id: e.id,
    source: e.from,
    sourceHandle: e.from_port,
    target: e.to,
    targetHandle: e.to_port,
  };
}
