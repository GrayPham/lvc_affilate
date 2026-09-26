// TDD Task 12 buoc 1: `allowedDurations` mirror `Mode::allowed_durations` (mode.rs) — bang tinh
// khop dung 6 truong hop test Rust `thoi_luong_cho_phep_theo_model_adr_044`. `isValidConnection`
// mirror hai buoc dau cua `validate()` (validate.rs): sai kieu cong bi tu choi, cong vao
// `many=false` nhan canh thu 2 bi tu choi (P4a: chua kind nao co cong nhu vay), canh hop le duoc
// chap nhan.
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
    dola_video: {
      kind: "dola_video",
      mode: "fast",
      model: "seedance-2.0",
      ratio: "9:16",
      duration: 10,
      logo: "off",
      account: "",
    },
    concat: { kind: "concat" },
    save: { kind: "save", out_dir: "" },
  };

  function node(id: string, kind: NodeKindTag): RFNode {
    return { id, type: kind, position: { x: 0, y: 0 }, data: DUMMY_DATA[kind] };
  }

  function edge(id: string, source: string, sourceHandle: string, target: string, targetHandle: string): RFEdge {
    return { id, source, sourceHandle, target, targetHandle };
  }

  it("canh hop le (kieu khop, cong con trong) duoc chap nhan", () => {
    const nodes = [node("n1", "prompt"), node("n2", "dola_video")];
    const ok = isValidConnection(
      { source: "n1", sourceHandle: "out", target: "n2", targetHandle: "prompt" },
      nodes,
      [],
    );
    expect(ok).toBe(true);
  });

  it("sai kieu cong (Image -> Text) bi tu choi", () => {
    const nodes = [node("n1", "image_upload"), node("n2", "dola_video")];
    const ok = isValidConnection(
      { source: "n1", sourceHandle: "out", target: "n2", targetHandle: "prompt" },
      nodes,
      [],
    );
    expect(ok).toBe(false);
  });

  it("cong vao many=true (vd Save.video) nhan nhieu canh tu nhieu node Tao video Dola", () => {
    // P4a: khong con kind nao co cong vao many=false (EnsureH264/RemoveLogo da xoa) — kiem nguoc
    // lai: cong many=true khong bi chan canh thu 2.
    const nodes = [node("v1", "dola_video"), node("v2", "dola_video"), node("n3", "save")];
    const existing = [edge("e1", "v1", "out", "n3", "video")];
    const ok = isValidConnection(
      { source: "v2", sourceHandle: "out", target: "n3", targetHandle: "video" },
      nodes,
      existing,
    );
    expect(ok).toBe(true);
  });

  it("cong vao many=true nhan nhieu canh (vd Video.prompt) van duoc chap nhan", () => {
    const nodes = [node("p1", "prompt"), node("p2", "prompt"), node("n3", "dola_video")];
    const existing = [edge("e1", "p1", "out", "n3", "prompt")];
    const ok = isValidConnection(
      { source: "p2", sourceHandle: "out", target: "n3", targetHandle: "prompt" },
      nodes,
      existing,
    );
    expect(ok).toBe(true);
  });

  it("thieu sourceHandle/targetHandle bi tu choi", () => {
    const nodes = [node("n1", "prompt"), node("n2", "dola_video")];
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

  it("review fix-round 1 I3: tu noi mot node vao chinh no (source === target) bi tu choi", () => {
    // Chua kind nao co cong vao/ra cung kieu — kiem thang nhanh source === target (chan truoc moi
    // buoc khac) bang mot node Tao video Dola.
    const nodes = [node("n1", "dola_video")];
    const ok = isValidConnection(
      { source: "n1", sourceHandle: "out", target: "n1", targetHandle: "refs" },
      nodes,
      [],
    );
    expect(ok).toBe(false);
  });
});
