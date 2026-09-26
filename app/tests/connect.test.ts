// Test THUAN cho `graph/connect.ts` — khop cong khi keo "+" ra cho trong roi them node moi
// (`connectionForNewNode`) va lam mo node khong noi duoc trong luc keo (`canConnectToNode`).
import { describe, expect, it } from "vitest";

import { canConnectToNode, connectionForNewNode, type PendingConnect } from "../src/graph/connect";
import type { RFEdge, RFNode } from "../src/graph/adapter";
import type { NodeKind, NodeKindTag } from "../src/ipc/types";

const DATA: Record<NodeKindTag, NodeKind> = {
  prompt: { kind: "prompt", text: "" },
  image_upload: { kind: "image_upload", paths: [] },
  load_video: { kind: "load_video", paths: [] },
  dola_video: { kind: "dola_video", mode: "fast", model: "seedance-2.0", ratio: "9:16", duration: 10, logo: "off", account: "" },
  concat: { kind: "concat" },
  save: { kind: "save", out_dir: "" },
  trim: { kind: "trim", start: 0, end: 0 },
  frame_capture: { kind: "frame_capture", time: 0 },
};

function node(id: string, kind: NodeKindTag): RFNode {
  return { id, type: kind, position: { x: 0, y: 0 }, data: DATA[kind] };
}

describe("connectionForNewNode — khop cong theo kieu du lieu", () => {
  it("keo tu cong RA (prompt.out=text) -> node moi dola_video: noi vao cong prompt", () => {
    const from: PendingConnect = { nodeId: "p1", kind: "prompt", handleId: "out", handleType: "source" };
    expect(connectionForNewNode(from, "v1", "dola_video")).toEqual({
      source: "p1",
      sourceHandle: "out",
      target: "v1",
      targetHandle: "prompt",
    });
  });

  it("keo tu cong VAO (save.video) -> node moi dola_video la NGUON (out -> save.video)", () => {
    const from: PendingConnect = { nodeId: "s1", kind: "save", handleId: "video", handleType: "target" };
    expect(connectionForNewNode(from, "v1", "dola_video")).toEqual({
      source: "v1",
      sourceHandle: "out",
      target: "s1",
      targetHandle: "video",
    });
  });

  it("khong co cong khop kieu -> null (prompt.out=text vao node save khong co input text)", () => {
    const from: PendingConnect = { nodeId: "p1", kind: "prompt", handleId: "out", handleType: "source" };
    expect(connectionForNewNode(from, "s1", "save")).toBeNull();
  });
});

describe("canConnectToNode — lam mo node khong noi duoc", () => {
  const from: PendingConnect = { nodeId: "p1", kind: "prompt", handleId: "out", handleType: "source" };

  it("prompt.out -> dola_video (co input text): noi duoc", () => {
    const nodes = [node("p1", "prompt"), node("v1", "dola_video")];
    expect(canConnectToNode(from, node("v1", "dola_video"), nodes, [])).toBe(true);
  });

  it("prompt.out -> save (khong co input text): KHONG noi duoc", () => {
    const nodes = [node("p1", "prompt"), node("s1", "save")];
    expect(canConnectToNode(from, node("s1", "save"), nodes, [])).toBe(false);
  });

  it("chinh node nguon -> KHONG noi duoc (khong tu noi vao chinh minh)", () => {
    const nodes = [node("p1", "prompt")];
    expect(canConnectToNode(from, node("p1", "prompt"), nodes, [])).toBe(false);
  });

  it("prompt.out -> node prompt khac (khong co cong vao): KHONG noi duoc", () => {
    const nodes = [node("p1", "prompt"), node("p2", "prompt")];
    const edges: RFEdge[] = [];
    expect(canConnectToNode(from, node("p2", "prompt"), nodes, edges)).toBe(false);
  });
});
