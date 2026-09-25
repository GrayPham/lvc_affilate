// P4a: canvas autosave trong localStorage ("lvc.workflow.canvas.v1") tu ban UI cu (Graph v1, kind
// `video`, co the kem kind P4 da xoa) phai khoi phuc duoc qua `loadPersistedGraph` — migrate
// TRUOC zod, khong mat ca canvas.
import { afterEach, describe, expect, it, vi } from "vitest";

import { loadPersistedGraph, useGraphStore } from "../src/store/graph";

function memoryStorage(initial: Record<string, string>): Storage {
  const data = new Map(Object.entries(initial));
  return {
    get length() {
      return data.size;
    },
    clear: () => data.clear(),
    getItem: (k) => data.get(k) ?? null,
    key: (i) => [...data.keys()][i] ?? null,
    removeItem: (k) => {
      data.delete(k);
    },
    setItem: (k, v) => {
      data.set(k, v);
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("loadPersistedGraph (P4a migrate)", () => {
  it("canvas v1 (kind video + concat) -> Graph v2 dola_video, bo concat + canh cua no", () => {
    const v1 = {
      version: 1,
      nodes: [
        { id: "n1", kind: { kind: "prompt", text: "canh 1" }, x: 0, y: 0 },
        {
          id: "n2",
          kind: {
            kind: "video",
            mode: "fast",
            model: "seedance-2.0",
            ratio: "9:16",
            duration: 10,
            logo: "off",
            account: "",
          },
          x: 300,
          y: 0,
        },
        { id: "n3", kind: { kind: "concat" }, x: 600, y: 0 },
      ],
      edges: [
        { id: "e1", from: "n1", from_port: "out", to: "n2", to_port: "prompt" },
        { id: "e2", from: "n2", from_port: "out", to: "n3", to_port: "in" },
      ],
    };
    vi.stubGlobal("localStorage", memoryStorage({ "lvc.workflow.canvas.v1": JSON.stringify(v1) }));

    const g = loadPersistedGraph();
    expect(g).not.toBeNull();
    expect(g!.version).toBe(2);
    expect(g!.nodes.map((n) => n.kind.kind)).toEqual(["prompt", "dola_video"]);
    expect(g!.edges.map((e) => e.id)).toEqual(["e1"]);

    useGraphStore.getState().loadGraph(g!);
    expect(useGraphStore.getState().nodes.map((n) => n.type)).toEqual(["prompt", "dola_video"]);
  });
});
