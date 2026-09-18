// TDD Task 12 buoc 1: `allowedDurations` mirror `Mode::allowed_durations` (mode.rs) — bang tinh
// khop dung 6 truong hop test Rust `thoi_luong_cho_phep_theo_model_adr_044`. `isValidConnection`
// mirror hai buoc dau cua `validate()` (validate.rs): sai kieu cong bi tu choi, cong vao
// `many=false` nhan canh thu 2 bi tu choi, canh hop le duoc chap nhan.
import { describe, expect, it } from "vitest";

import { allowedDurations, isValidConnection } from "../src/graph/rules";
import type { RFEdge, RFNode } from "../src/graph/adapter";
import type { Mode, Model, NodeKind, NodeKindTag } from "../src/ipc/types";

describe("allowedDurations — mirror Mode::allowed_durations (mode.rs:53, ADR-044)", () => {
  const cases: ReadonlyArray<readonly [Mode, Model, readonly number[]]> = [
    ["fast", "seedance-2.5", [10, 15, 30]],
    ["fast", "seedance-2.0", [10, 15]],
    ["fast", "seedance-1.0", [10, 15]],
    ["pro", "seedance-2.5", [10, 15]],
    ["pro_max", "seedance-2.5", [10, 15]],
    ["agent", "seedance-2.5", [10, 15]],
  ];

  it.each(cases)("%s + %s -> %j", (mode, model, want) => {
    expect(allowedDurations(mode, model)).toEqual(want);
  });
});

describe("isValidConnection — mirror validate() buoc kieu cong + cong don (validate.rs)", () => {
  const DUMMY_DATA: Record<NodeKindTag, NodeKind> = {
    prompt: { kind: "prompt", text: "canh 1" },
    image_upload: { kind: "image_upload", paths: [] },
    image_from_url: { kind: "image_from_url", url: "" },
    image_gen: { kind: "image_gen", prompt: "", backend: "" },
    video: {
      kind: "video",
      mode: "fast",
      model: "seedance-2.0",
      ratio: "9:16",
      duration: 10,
      logo: "off",
      account: "",
    },
    remove_logo: { kind: "remove_logo" },
    ensure_h264: { kind: "ensure_h264" },
    concat: { kind: "concat" },
    save: { kind: "save", out_dir: "downloads" },
  };

  function node(id: string, kind: NodeKindTag): RFNode {
    return { id, type: kind, position: { x: 0, y: 0 }, data: DUMMY_DATA[kind] };
  }

  function edge(id: string, source: string, sourceHandle: string, target: string, targetHandle: string): RFEdge {
    return { id, source, sourceHandle, target, targetHandle };
  }

  it("canh hop le (kieu khop, cong con trong) duoc chap nhan", () => {
    const nodes = [node("n1", "prompt"), node("n2", "video")];
    const ok = isValidConnection(
      { source: "n1", sourceHandle: "out", target: "n2", targetHandle: "prompt" },
      nodes,
      [],
    );
    expect(ok).toBe(true);
  });

  it("sai kieu cong (Image -> Text) bi tu choi", () => {
    const nodes = [node("n1", "image_upload"), node("n2", "video")];
    const ok = isValidConnection(
      { source: "n1", sourceHandle: "out", target: "n2", targetHandle: "prompt" },
      nodes,
      [],
    );
    expect(ok).toBe(false);
  });

  it("cong vao many=false da co 1 canh -> canh thu 2 bi tu choi", () => {
    // EnsureH264."in" many=false (VIDEO_POST_IN, node.rs) — hai node Video khac nhau cung noi vao.
    const nodes = [node("v1", "video"), node("v2", "video"), node("n3", "ensure_h264")];
    const existing = [edge("e1", "v1", "out", "n3", "in")];
    const ok = isValidConnection(
      { source: "v2", sourceHandle: "out", target: "n3", targetHandle: "in" },
      nodes,
      existing,
    );
    expect(ok).toBe(false);
  });

  it("cong vao many=false CHUA co canh nao -> canh dau tien duoc chap nhan", () => {
    const nodes = [node("v1", "video"), node("n3", "ensure_h264")];
    const ok = isValidConnection(
      { source: "v1", sourceHandle: "out", target: "n3", targetHandle: "in" },
      nodes,
      [],
    );
    expect(ok).toBe(true);
  });

  it("cong vao many=true nhan nhieu canh (vd Video.prompt) van duoc chap nhan", () => {
    const nodes = [node("p1", "prompt"), node("p2", "prompt"), node("n3", "video")];
    const existing = [edge("e1", "p1", "out", "n3", "prompt")];
    const ok = isValidConnection(
      { source: "p2", sourceHandle: "out", target: "n3", targetHandle: "prompt" },
      nodes,
      existing,
    );
    expect(ok).toBe(true);
  });

  it("thieu sourceHandle/targetHandle bi tu choi", () => {
    const nodes = [node("n1", "prompt"), node("n2", "video")];
    expect(
      isValidConnection({ source: "n1", sourceHandle: null, target: "n2", targetHandle: "prompt" }, nodes, []),
    ).toBe(false);
    expect(
      isValidConnection({ source: "n1", sourceHandle: "out", target: "n2", targetHandle: null }, nodes, []),
    ).toBe(false);
  });

  it("noi toi node/cong khong ton tai bi tu choi", () => {
    const nodes = [node("n1", "prompt")];
    expect(
      isValidConnection({ source: "n1", sourceHandle: "out", target: "khong-ton-tai", targetHandle: "prompt" }, nodes, []),
    ).toBe(false);
    expect(
      isValidConnection({ source: "n1", sourceHandle: "bogus", target: "n1", targetHandle: "out" }, nodes, []),
    ).toBe(false);
  });
});
