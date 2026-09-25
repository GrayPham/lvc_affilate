// P-lumina: `referenceChips` (src/graph/refs.ts) suy chip tham chieu tu nodes+edges — chi lay canh
// noi vao cong `refs` cua node dich, mo ta node nguon, mang `edgeId` de xoa canh. `nodeChipLabel`
// chon nhan gon theo tung kind (ten file anh, dong prompt dau, hoac nhan kind).
import { describe, expect, it } from "vitest";

import { nodeChipLabel, referenceChips } from "../src/graph/refs";
import type { RFEdge, RFNode } from "../src/graph/adapter";

function imageNode(id: string, paths: string[]): RFNode {
  return { id, type: "image_upload", position: { x: 0, y: 0 }, data: { kind: "image_upload", paths } };
}

function videoNode(id: string): RFNode {
  return {
    id,
    type: "dola_video",
    position: { x: 0, y: 0 },
    data: { kind: "dola_video", mode: "fast", model: "seedance-2.0", ratio: "9:16", duration: 10, logo: "off", account: "" },
  };
}

function edge(id: string, source: string, target: string, targetHandle: string): RFEdge {
  return { id, source, target, sourceHandle: "out", targetHandle };
}

describe("referenceChips — cong refs cua node dola_video", () => {
  it("chi lay canh noi vao cong `refs` cua node dich, dung edgeId + nodeId + nhan", () => {
    const nodes: RFNode[] = [imageNode("img1", ["/a/character.png"]), imageNode("img2", ["/b/cloths.jpg"]), videoNode("vid")];
    const edges: RFEdge[] = [
      edge("e1", "img1", "vid", "refs"),
      edge("e2", "img2", "vid", "refs"),
      edge("e3", "img1", "vid", "prompt"), // cong khac -> bo qua
    ];
    const chips = referenceChips("vid", nodes, edges);
    expect(chips).toEqual([
      { edgeId: "e1", nodeId: "img1", label: "character.png" },
      { edgeId: "e2", nodeId: "img2", label: "cloths.jpg" },
    ]);
  });

  it("bo qua canh tro toi node nguon khong con ton tai", () => {
    const nodes: RFNode[] = [videoNode("vid")];
    const edges: RFEdge[] = [edge("e1", "ghost", "vid", "refs")];
    expect(referenceChips("vid", nodes, edges)).toEqual([]);
  });

  it("node dich khong co canh refs -> mang rong", () => {
    const nodes: RFNode[] = [imageNode("img1", ["/a/x.png"]), videoNode("vid")];
    expect(referenceChips("vid", nodes, [])).toEqual([]);
  });
});

describe("nodeChipLabel — nhan gon theo kind", () => {
  it("image_upload: 1 anh -> ten file; nhieu anh -> ten file dau + so con lai", () => {
    expect(nodeChipLabel({ kind: "image_upload", paths: ["/a/one.png"] })).toBe("one.png");
    expect(nodeChipLabel({ kind: "image_upload", paths: ["/a/one.png", "/b/two.png", "/c/3.png"] })).toBe("one.png +2");
  });

  it("image_upload rong -> nhan kind", () => {
    expect(nodeChipLabel({ kind: "image_upload", paths: [] })).toBe("Tải ảnh lên");
  });

  it("prompt: lay dong khong rong dau tien, cat neu qua dai", () => {
    expect(nodeChipLabel({ kind: "prompt", text: "\n  canh dau tien\ncanh 2" })).toBe("canh dau tien");
    const longText = "x".repeat(40);
    expect(nodeChipLabel({ kind: "prompt", text: longText })).toBe(`${"x".repeat(24)}…`);
  });

  it("prompt rong -> nhan kind", () => {
    expect(nodeChipLabel({ kind: "prompt", text: "   " })).toBe("Prompt");
  });

  it("dola_video/save -> nhan kind", () => {
    expect(nodeChipLabel({ kind: "dola_video", mode: "fast", model: "seedance-2.0", ratio: "9:16", duration: 10, logo: "off", account: "" })).toBe("Tạo video Dola");
    expect(nodeChipLabel({ kind: "save", out_dir: "" })).toBe("Lưu");
  });
});
