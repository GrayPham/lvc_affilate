// Hộp thoại "Thêm workflow vào hàng chờ" (src/components/AddWorkflowDialog.tsx) — hai phép ghi đè
// áp cho RIÊNG lần thêm này.
//
// Cả hai đều sửa ẢNH CHỤP đồ thị đưa vào hàng chờ, KHÔNG sửa workflow gốc: sửa gốc sẽ đổi cả
// những lần chạy sau, không phải ý người dùng.
import { describe, expect, it } from "vitest";

import {
  countImageNodes,
  countPromptNodes,
  countSaveNodes,
  withImages,
  withOutDir,
  withPromptText,
} from "../src/components/AddWorkflowDialog";
import { countScenes } from "../src/graph/sceneCount";
import type { Graph } from "../src/ipc/types";

function graph(): Graph {
  return {
    version: 2,
    nodes: [
      { id: "p", kind: { kind: "prompt", text: "canh 1" }, x: 0, y: 0 },
      { id: "s1", kind: { kind: "save", out_dir: "cu" }, x: 0, y: 0 },
      { id: "s2", kind: { kind: "save", out_dir: "" }, x: 0, y: 0 },
      // Them o CUOI de chi so cac node khac giu nguyen cho test ghim theo vi tri.
      { id: "img", kind: { kind: "image_upload", paths: ["cu.png"] }, x: 0, y: 0 },
    ],
    edges: [],
  } as unknown as Graph;
}

describe("withOutDir", () => {
  it("doi out_dir cua MOI node save", () => {
    const out = withOutDir(graph(), "D:/moi");
    const saves = out.nodes.filter((n) => n.kind.kind === "save");
    expect(saves.length).toBe(2);
    for (const s of saves) {
      expect((s.kind as { out_dir: string }).out_dir).toBe("D:/moi");
    }
  });

  it("KHONG sua do thi goc (tra ban sao)", () => {
    const g = graph();
    withOutDir(g, "D:/moi");
    expect((g.nodes[1]!.kind as { out_dir: string }).out_dir).toBe("cu");
  });

  it("khong cham vao node khac save", () => {
    const out = withOutDir(graph(), "D:/moi");
    expect(out.nodes[0]!.kind).toEqual({ kind: "prompt", text: "canh 1" });
  });

  it("duong dan rong/toan khoang trang -> giu nguyen do thi", () => {
    expect(withOutDir(graph(), "")).toEqual(graph());
    expect(withOutDir(graph(), "   ")).toEqual(graph());
  });

  it("trim hai dau truoc khi ghi", () => {
    const out = withOutDir(graph(), "  D:/moi  ");
    expect((out.nodes[1]!.kind as { out_dir: string }).out_dir).toBe("D:/moi");
  });
});

// "Tác vụ là nơi nhập đầu vào để chạy workflow" (quyết định 2026-10-14).
describe("withPromptText", () => {
  it("thay text cua MOI node prompt", () => {
    const out = withPromptText(graph(), "canh A\ncanh B");
    const p = out.nodes.find((n) => n.kind.kind === "prompt")!;
    expect((p.kind as { text: string }).text).toBe("canh A\ncanh B");
  });

  it("KHONG sua do thi goc (tra ban sao)", () => {
    const g = graph();
    withPromptText(g, "canh moi");
    expect((g.nodes[0]!.kind as { text: string }).text).toBe("canh 1");
  });

  it("bo dong trong — dong trong khong phai mot canh", () => {
    const out = withPromptText(graph(), "A\n\n   \nB\n");
    const p = out.nodes.find((n) => n.kind.kind === "prompt")!;
    expect((p.kind as { text: string }).text).toBe("A\nB");
  });

  it("text rong -> giu nguyen do thi (dung prompt da luu trong workflow)", () => {
    expect(withPromptText(graph(), "")).toEqual(graph());
    expect(withPromptText(graph(), "   ")).toEqual(graph());
  });

  it("khong cham node khac prompt", () => {
    const out = withPromptText(graph(), "x");
    expect(out.nodes[1]!.kind).toEqual({ kind: "save", out_dir: "cu" });
  });
});

describe("dem", () => {
  it("countScenes dem dong khac rong", () => {
    expect(countScenes("A\nB\nC")).toBe(3);
    expect(countScenes("A\n\n  \nB")).toBe(2);
    expect(countScenes("")).toBe(0);
    expect(countScenes("   ")).toBe(0);
  });

  it("countSaveNodes / countPromptNodes", () => {
    expect(countSaveNodes(graph())).toBe(2);
    expect(countPromptNodes(graph())).toBe(1);
    const trong = { version: 2, nodes: [], edges: [] } as unknown as Graph;
    expect(countSaveNodes(trong)).toBe(0);
    expect(countPromptNodes(trong)).toBe(0);
  });
});

// "input dau vao chi viec nhap text, chon anh" — anh tham chieu tiem vao anh chup, workflow goc
// khong doi (cung ly do voi withPromptText/withOutDir).
describe("withImages", () => {
  it("thay paths cua MOI node image_upload", () => {
    const out = withImages(graph(), ["a.png", "b.jpg"]);
    const img = out.nodes.find((n) => n.kind.kind === "image_upload")!;
    expect((img.kind as { paths: string[] }).paths).toEqual(["a.png", "b.jpg"]);
  });

  it("KHONG sua do thi goc", () => {
    const g = graph();
    withImages(g, ["moi.png"]);
    const img = g.nodes.find((n) => n.kind.kind === "image_upload")!;
    expect((img.kind as { paths: string[] }).paths).toEqual(["cu.png"]);
  });

  it("danh sach rong -> giu nguyen (dung anh da co trong workflow)", () => {
    expect(withImages(graph(), [])).toEqual(graph());
  });

  it("khong cham node khac image_upload", () => {
    const out = withImages(graph(), ["x.png"]);
    expect(out.nodes.find((n) => n.id === "p")!.kind).toEqual({ kind: "prompt", text: "canh 1" });
  });

  it("countImageNodes dem dung", () => {
    expect(countImageNodes(graph())).toBe(1);
    const trong = { version: 2, nodes: [], edges: [] } as unknown as Graph;
    expect(countImageNodes(trong)).toBe(0);
  });
});