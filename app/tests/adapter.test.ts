// TDD Task 11 buoc 1: `fromGraph`/`toGraph` (src/graph/adapter.ts) phai doi qua lai voi
// `graph.json` (fixture Task 9, do thi 3 canh Prompt -> Tao video -> Luu) khong mat/lech du lieu,
// canh sinh ra phai tro dung ten tay cam co that trong `PORTS`, va `NodeKind` la khong ro (chua
// dang ky) phai bi zod tu choi tai bien gioi schema — khong lot qua duoc `toGraph`/`fromGraph`.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { fromGraph, migrateGraphJson, toGraph, type RFEdge } from "../src/graph/adapter";
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

describe("migrateGraphJson (P4a): do thi cu -> dinh dang hien tai TRUOC zod", () => {
  const VIDEO_PARAMS = {
    mode: "fast",
    model: "seedance-2.0",
    ratio: "9:16",
    duration: 10,
    logo: "off",
    account: "",
  };

  it("v1: doi kind `video` -> `dola_video`, bo node kind P4 da xoa kem canh cham vao", () => {
    const v1 = {
      version: 1,
      nodes: [
        { id: "n1", kind: { kind: "prompt", text: "canh 1" }, x: 0, y: 0 },
        { id: "n2", kind: { kind: "video", ...VIDEO_PARAMS }, x: 300, y: 0 },
        { id: "n3", kind: { kind: "ensure_h264" }, x: 600, y: 0 },
        { id: "n4", kind: { kind: "image_from_url", url: "http://x" }, x: 0, y: 200 },
        { id: "n5", kind: { kind: "save", out_dir: "downloads" }, x: 900, y: 0 },
      ],
      edges: [
        { id: "e1", from: "n1", from_port: "out", to: "n2", to_port: "prompt" },
        { id: "e2", from: "n2", from_port: "out", to: "n3", to_port: "in" },
        { id: "e3", from: "n3", from_port: "out", to: "n5", to_port: "video" },
        { id: "e4", from: "n4", from_port: "out", to: "n2", to_port: "refs" },
        { id: "e5", from: "n2", from_port: "out", to: "n5", to_port: "video" },
      ],
    };
    const g = GraphSchema.parse(migrateGraphJson(v1));
    expect(g.version).toBe(2);
    expect(g.nodes.map((n) => [n.id, n.kind.kind])).toEqual([
      ["n1", "prompt"],
      ["n2", "dola_video"],
      ["n5", "save"],
    ]);
    expect(g.nodes[1]!.kind).toEqual({ kind: "dola_video", ...VIDEO_PARAMS });
    expect(g.edges.map((e) => e.id)).toEqual(["e1", "e5"]);

    const { nodes } = fromGraph(g);
    expect(nodes.map((n) => n.type)).toEqual(["prompt", "dola_video", "save"]);
  });

  it("v1 JSON khong migrate thi zod tu choi (kind `video` khong con hop le)", () => {
    const v1 = {
      version: 1,
      nodes: [{ id: "n2", kind: { kind: "video", ...VIDEO_PARAMS }, x: 0, y: 0 }],
      edges: [],
    };
    expect(GraphSchema.safeParse(v1).success).toBe(false);
    expect(GraphSchema.safeParse(migrateGraphJson(v1)).success).toBe(true);
  });

  it("graph.json (v2) di qua migrate khong doi gi", () => {
    const original = loadGraphFixture();
    expect(migrateGraphJson(original)).toEqual(original);
  });

  it("dau vao khong dung hinh Graph -> tra nguyen", () => {
    expect(migrateGraphJson(null)).toBeNull();
    expect(migrateGraphJson("abc")).toBe("abc");
    expect(migrateGraphJson({ foo: 1 })).toEqual({ foo: 1 });
  });
});
