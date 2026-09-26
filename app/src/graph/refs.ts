// Chip tham chieu (P-lumina): cac node NGUON noi vao mot cong VAO cua mot node dich — suy THUAN
// tu nodes+edges hien tai trong store (khong doi round-trip). Dung o `VideoParamsPanel` de liet ke
// cac node noi vao cong `refs` (kieu image) cua node `dola_video` dang chon, moi chip co the xoa
// canh (bo tham chieu) qua `edgeId`. Tach ham THUAN de test doc lap (`tests/refs.test.ts`).
import type { NodeKind } from "../ipc/types";
import { NODE_LABEL_VI } from "./ports";
import type { RFEdge, RFNode } from "./adapter";

/** Mot chip tham chieu — mang `edgeId` (de xoa canh) + `nodeId` nguon + nhan hien thi. */
export interface ReferenceChip {
  edgeId: string;
  nodeId: string;
  label: string;
}

function baseName(p: string): string {
  const parts = p.split(/[/\\]/);
  return parts[parts.length - 1] ?? p;
}

/** Nhan ngan gon mo ta mot node nguon (dung tren chip) — uu tien meta cua node, roi den nhan kind. */
export function nodeChipLabel(data: NodeKind): string {
  switch (data.kind) {
    case "image_upload": {
      // Uu tien TEN TUY CHINH cua node (nguoi dung dat, vd "character"/"hung") lam nhan tham chieu.
      const custom = (data.label ?? "").trim();
      if (custom.length > 0) {
        return custom;
      }
      const n = data.paths.length;
      if (n === 0) {
        return NODE_LABEL_VI.image_upload;
      }
      const first = baseName(data.paths[0]!);
      return n === 1 ? first : `${first} +${n - 1}`;
    }
    case "prompt": {
      const first = data.text.split("\n").map((l) => l.trim()).find((l) => l.length > 0) ?? "";
      if (first.length === 0) {
        return NODE_LABEL_VI.prompt;
      }
      return first.length > 24 ? `${first.slice(0, 24)}…` : first;
    }
    case "dola_video":
      return NODE_LABEL_VI.dola_video;
    case "concat":
      return NODE_LABEL_VI.concat;
    case "save":
      return NODE_LABEL_VI.save;
  }
}

/**
 * Cac chip tham chieu cua `nodeId` tai mot cong VAO (`port`, mac dinh `refs`): moi canh co
 * `target === nodeId` va `targetHandle === port` -> mot chip mo ta node NGUON. Bo qua canh tro toi
 * node nguon khong con ton tai (an toan khi store dang giua chung sua). Giu nguyen thu tu edges.
 */
export function referenceChips(
  nodeId: string,
  nodes: readonly RFNode[],
  edges: readonly RFEdge[],
  port = "refs",
): ReferenceChip[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const chips: ReferenceChip[] = [];
  for (const e of edges) {
    if (e.target !== nodeId || e.targetHandle !== port) {
      continue;
    }
    const src = byId.get(e.source);
    if (!src) {
      continue;
    }
    chips.push({ edgeId: e.id, nodeId: src.id, label: nodeChipLabel(src.data) });
  }
  return chips;
}
