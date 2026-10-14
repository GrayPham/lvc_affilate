// `autoLayout` (src/graph/autoLayout.ts) — xep node theo TANG tu trai sang phai dung chieu du lieu
// chay. THUAN nen test duoc truc tiep, khong can canvas.
import { describe, expect, it } from "vitest";

import { autoLayout, layerOf } from "../src/graph/autoLayout";
import type { RFEdge, RFNode } from "../src/graph/adapter";

function node(id: string): RFNode {
  return {
    id,
    type: "prompt",
    position: { x: 999, y: 999 },
    data: { kind: "prompt", text: "" },
  } as unknown as RFNode;
}

function edge(from: string, to: string): RFEdge {
  return { id: `${from}-${to}`, source: from, target: to } as RFEdge;
}

describe("layerOf", () => {
  it("node khong co dau vao nam tang 0", () => {
    const l = layerOf([node("a"), node("b")], []);
    expect(l.get("a")).toBe(0);
    expect(l.get("b")).toBe(0);
  });

  it("chuoi a -> b -> c ra ba tang lien tiep", () => {
    const l = layerOf([node("a"), node("b"), node("c")], [edge("a", "b"), edge("b", "c")]);
    expect([l.get("a"), l.get("b"), l.get("c")]).toEqual([0, 1, 2]);
  });

  it("node nhieu nguon nam SAU nguon sau nhat", () => {
    // a -> b -> d, a -> d : d phai o tang 2 (sau b), khong phai tang 1.
    const l = layerOf(
      [node("a"), node("b"), node("d")],
      [edge("a", "b"), edge("b", "d"), edge("a", "d")],
    );
    expect(l.get("d")).toBe(2);
  });

  it("canh mo coi (mot dau da bi xoa) khong lam lech bo cuc", () => {
    const l = layerOf([node("a"), node("b")], [edge("a", "b"), edge("da-xoa", "b")]);
    expect(l.get("b")).toBe(1);
  });

  it("do thi co CHU TRINH van ra bo cuc, khong treo", () => {
    const l = layerOf([node("a"), node("b")], [edge("a", "b"), edge("b", "a")]);
    expect(l.size).toBe(2);
    for (const v of l.values()) {
      expect(Number.isFinite(v)).toBe(true);
    }
  });
});

describe("autoLayout", () => {
  it("tang -> cot ngang, cung tang -> xep doc", () => {
    const out = autoLayout([node("a"), node("b"), node("c")], [edge("a", "c"), edge("b", "c")]);
    const by = new Map(out.map((n) => [n.id, n.position]));
    // a va b cung tang 0: cung x, khac y.
    expect(by.get("a")!.x).toBe(by.get("b")!.x);
    expect(by.get("a")!.y).not.toBe(by.get("b")!.y);
    // c o tang 1: x lon hon.
    expect(by.get("c")!.x).toBeGreaterThan(by.get("a")!.x);
  });

  it("KHONG sua mang goc", () => {
    const nodes = [node("a")];
    autoLayout(nodes, []);
    expect(nodes[0]!.position).toEqual({ x: 999, y: 999 });
  });

  it("giu nguyen moi thu khac cua node, chi doi vi tri", () => {
    const n = { ...node("a"), selected: true, width: 224 } as unknown as RFNode;
    const out = autoLayout([n], [])[0]!;
    expect(out.id).toBe("a");
    expect(out.data).toEqual(n.data);
    expect((out as unknown as { selected: boolean }).selected).toBe(true);
  });

  it("bam hai lan lien tiep ra DUNG mot ket qua", () => {
    const nodes = [node("a"), node("b"), node("c")];
    const edges = [edge("a", "b")];
    const first = autoLayout(nodes, edges);
    const second = autoLayout(first, edges);
    expect(second.map((n) => n.position)).toEqual(first.map((n) => n.position));
  });

  it("canvas trong -> mang rong, khong nem", () => {
    expect(autoLayout([], [])).toEqual([]);
  });
});
