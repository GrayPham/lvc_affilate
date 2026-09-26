// Suy ra CANH nen tao khi keo mot day tu handle cua node NGUON ra cho trong roi chon them mot node
// MOI (kieu Lumina: keo "+" -> menu Add Node -> node moi da noi san). THUAN, test doc lap
// (`tests/connect.test.ts`). Khop cong theo KIEU du lieu (PORTS): output cua nguon phai cung kieu
// voi mot input cua node moi (va nguoc lai khi keo tu cong VAO).
import type { Connection } from "@xyflow/react";

import { PORTS } from "./ports";
import { isValidConnection } from "./rules";
import type { RFEdge, RFNode } from "./adapter";
import type { NodeKindTag } from "../ipc/types";

/** Handle bat dau keo: node nao, cong nao, la cong RA (`source`) hay cong VAO (`target`). */
export interface PendingConnect {
  nodeId: string;
  kind: NodeKindTag;
  handleId: string;
  handleType: "source" | "target";
}

/**
 * `Connection` (React Flow) noi node NGUON (dang keo) voi node MOI (`newNodeId`/`newKind`), hoac
 * `null` neu khong co cong nao khop kieu.
 * - Keo tu cong RA (`source`): node moi la DICH — tim input cua node moi cung kieu voi output dang keo.
 * - Keo tu cong VAO (`target`): node moi la NGUON — tim output cua node moi cung kieu voi input dang keo.
 * Chon cong ĐẦU TIÊN khop kieu (đủ cho tập node hiện tại; mỗi kiểu vào chỉ có một cổng hợp).
 */
export function connectionForNewNode(
  from: PendingConnect,
  newNodeId: string,
  newKind: NodeKindTag,
): Connection | null {
  if (from.handleType === "source") {
    const outType = PORTS[from.kind].outputs.find((o) => o.name === from.handleId)?.type;
    if (!outType) {
      return null;
    }
    const inPort = PORTS[newKind].inputs.find((i) => i.type === outType);
    if (!inPort) {
      return null;
    }
    return { source: from.nodeId, sourceHandle: from.handleId, target: newNodeId, targetHandle: inPort.name };
  }
  const inType = PORTS[from.kind].inputs.find((i) => i.name === from.handleId)?.type;
  if (!inType) {
    return null;
  }
  const outPort = PORTS[newKind].outputs.find((o) => o.type === inType);
  if (!outPort) {
    return null;
  }
  return { source: newNodeId, sourceHandle: outPort.name, target: from.nodeId, targetHandle: from.handleId };
}

/**
 * `node` co the la ĐÍCH (khi keo tu cong RA) hoac NGUON (khi keo tu cong VAO) hop le cho day dang
 * keo tu `from` khong? — dung de LAM MO node khong noi duoc trong luc keo. Bo chinh node nguon
 * (`from.nodeId`). Thu MOI cong cung kieu roi kiem `isValidConnection` (chan sai kieu / trung / cong
 * `many=false` da day / tu noi vao chinh no).
 */
export function canConnectToNode(
  from: PendingConnect,
  node: RFNode,
  nodes: readonly RFNode[],
  edges: readonly RFEdge[],
): boolean {
  if (node.id === from.nodeId) {
    return false;
  }
  const nodeKind = node.data.kind as NodeKindTag;
  if (from.handleType === "source") {
    const outType = PORTS[from.kind].outputs.find((o) => o.name === from.handleId)?.type;
    if (!outType) {
      return false;
    }
    return PORTS[nodeKind].inputs.some(
      (i) =>
        i.type === outType &&
        isValidConnection(
          { source: from.nodeId, sourceHandle: from.handleId, target: node.id, targetHandle: i.name },
          nodes,
          edges,
        ),
    );
  }
  const inType = PORTS[from.kind].inputs.find((i) => i.name === from.handleId)?.type;
  if (!inType) {
    return false;
  }
  return PORTS[nodeKind].outputs.some(
    (o) =>
      o.type === inType &&
      isValidConnection(
        { source: node.id, sourceHandle: o.name, target: from.nodeId, targetHandle: from.handleId },
        nodes,
        edges,
      ),
  );
}
