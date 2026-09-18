// TDD Task 11 buoc 1: `fromGraph`/`toGraph` (src/graph/adapter.ts) phai doi qua lai voi
// `graph.json` (fixture Task 9, do thi 3 canh Prompt -> Tao video -> Luu) khong mat/lech du lieu,
// canh sinh ra phai tro dung ten tay cam co that trong `PORTS`, va `NodeKind` la khong ro (chua
// dang ky) phai bi zod tu choi tai bien gioi schema — khong lot qua duoc `toGraph`/`fromGraph`.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { fromGraph, toGraph, type RFEdge } from "../src/graph/adapter";
import { GraphSchema, type Graph } from "../src/ipc/types";
import { PORTS } from "../src/graph/ports";

const FIXTURES_DIR = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "src",
  "ipc",
  "fixtures",
);

function loadGraphFixture(): Graph {
  const raw = readFileSync(path.join(FIXTURES_DIR, "graph.json"), "utf-8");
  return GraphSchema.parse(JSON.parse(raw));
}

describe("adapter: Graph (Rust) <-> React Flow", () => {
  it("fromGraph(graph.json) roi toGraph(...) deep-equal dung JSON ban dau", () => {
    const original = loadGraphFixture();
    const { nodes, edges } = fromGraph(original);
    const back = toGraph(nodes, edges);
    expect(back).toEqual(original);
  });

  it("moi canh sinh ra tham chieu dung ten tay cam co that trong PORTS cua node hai dau", () => {
    const original = loadGraphFixture();
    const { edges } = fromGraph(original);
    const kindByNodeId = new Map(original.nodes.map((n) => [n.id, n.kind.kind]));

    expect(edges.length).toBeGreaterThan(0);
    for (const edge of edges) {
      const fromKind = kindByNodeId.get(edge.source);
      const toKind = kindByNodeId.get(edge.target);
      expect(fromKind, `canh ${edge.id}: khong tim thay node nguon ${edge.source}`).toBeDefined();
      expect(toKind, `canh ${edge.id}: khong tim thay node dich ${edge.target}`).toBeDefined();

      const outputNames = PORTS[fromKind as keyof typeof PORTS].outputs.map((p) => p.name);
      const inputNames = PORTS[toKind as keyof typeof PORTS].inputs.map((p) => p.name);
      expect(outputNames, `canh ${edge.id}: cong ra "${edge.sourceHandle}"`).toContain(
        edge.sourceHandle,
      );
      expect(inputNames, `canh ${edge.id}: cong vao "${edge.targetHandle}"`).toContain(
        edge.targetHandle,
      );
    }
  });

  it("toGraph nem loi ro rang khi mot canh thieu sourceHandle/targetHandle", () => {
    const { nodes } = fromGraph(loadGraphFixture());
    const first = nodes[0];
    const second = nodes[1];
    expect(first).toBeDefined();
    expect(second).toBeDefined();
    const badEdge: RFEdge = { id: "bad", source: first!.id, target: second!.id };
    expect(() => toGraph(nodes, [badEdge])).toThrow(/sourceHandle|targetHandle/);
  });

  it("GraphSchema tu choi Node co kind chua dang ky (khong lot qua duoc bien gioi IPC)", () => {
    const original = loadGraphFixture();
    const bogus = {
      ...original,
      nodes: [{ id: "bogus", kind: { kind: "khong_ton_tai" }, x: 0, y: 0 }, ...original.nodes],
    };
    expect(GraphSchema.safeParse(bogus).success).toBe(false);
  });
});
