// Doi qua lai giua do thi React Flow (canvas Task 12) va `Graph` cua Rust
// (crates/lvc-core/src/graph/mod.rs — dinh dang file workflow JSON, cung la tham so/ket qua cac
// lenh IPC `workflow_validate`/`workflow_save`/`run_start`).
//
// Vi tri ve (`Node.x`/`Node.y` — hai truong PHANG, KHONG long trong `position`, xem node.rs) map
// thang sang `RFNode.position.{x,y}` cua React Flow — Graph cua Rust KHONG co truong nao khac
// ngoai `id, kind, x, y` nen KHONG can them mot map UI-only rieng (`ui: Record<id,{x,y}>`) nhu
// brief co luu y phong khi Rust thieu vi tri.
//
// `RFNode.data` mang THANG `NodeKind` (bao gom ca tham so rieng cua node, vd `dola_video.mode`) — mot
// kieu duy nhat cho ca hai chieu, tranh lech field khi Task 12 doc/ghi `data` tren canvas.
import type { Edge as RFEdgeBase, Node as RFNodeBase } from "@xyflow/react";
import type { Edge, Graph, Node as GraphNode, NodeKind, NodeKindTag } from "../ipc/types";

export type RFNodeData = NodeKind;
export type RFNode = RFNodeBase<RFNodeData, NodeKindTag>;
export type RFEdge = RFEdgeBase;

/** `Graph.version` hien tai — khop `GRAPH_VERSION` (crates/lvc-core/src/graph/mod.rs, P4a = 2). */
export const GRAPH_VERSION = 2;

/** Kind P4 cu da bi XOA khoi backend o P4a — node loai nay bi bo khi migrate (kem canh noi vao). */
const REMOVED_KINDS: ReadonlySet<string> = new Set([
  "image_from_url",
  "image_gen",
  "remove_logo",
  "ensure_h264",
  "concat",
]);

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function kindOf(node: unknown): string | undefined {
  if (!isRecord(node) || !isRecord(node.kind)) {
    return undefined;
  }
  const k = node.kind.kind;
  return typeof k === "string" ? k : undefined;
}

/**
 * Migrate JSON `Graph` THO (chua qua zod) tu dinh dang cu sang hien tai — goi TRUOC
 * `GraphSchema.parse` o moi cua vao (localStorage, `workflow_get`). Mirror `migrate()` cua Rust
 * (mod.rs): `version < 2` -> doi kind `video` thanh `dola_video`. Them rieng phia UI: bo node co
 * kind P4 da xoa (+ moi canh cham vao node do), bat ke version — neu khong zod se tu choi CA do
 * thi. Dau vao khong dung hinh `Graph` -> tra nguyen de zod bao loi nhu cu.
 */
export function migrateGraphJson(raw: unknown): unknown {
  if (!isRecord(raw) || !Array.isArray(raw.nodes)) {
    return raw;
  }
  const oldVersion = typeof raw.version === "number" && raw.version < GRAPH_VERSION;
  const removedIds = new Set<string>();
  const nodes: unknown[] = [];
  for (const n of raw.nodes) {
    const kind = kindOf(n);
    if (kind !== undefined && REMOVED_KINDS.has(kind)) {
      if (isRecord(n) && typeof n.id === "string") {
        removedIds.add(n.id);
      }
      continue;
    }
    if (oldVersion && kind === "video" && isRecord(n) && isRecord(n.kind)) {
      nodes.push({ ...n, kind: { ...n.kind, kind: "dola_video" } });
      continue;
    }
    nodes.push(n);
  }
  const edges = Array.isArray(raw.edges)
    ? raw.edges.filter(
        (e) =>
          !isRecord(e) ||
          !(
            (typeof e.from === "string" && removedIds.has(e.from)) ||
            (typeof e.to === "string" && removedIds.has(e.to))
          ),
      )
    : raw.edges;
  return {
    ...raw,
    version: typeof raw.version === "number" ? Math.max(raw.version, GRAPH_VERSION) : raw.version,
    nodes,
    edges,
  };
}

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
