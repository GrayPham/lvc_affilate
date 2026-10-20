// P4a: canvas autosave trong localStorage ("lvc.workflow.canvas.v1") tu ban UI cu (Graph v1, kind
// `video`, co the kem kind P4 da xoa) phai khoi phuc duoc qua `loadPersistedCanvas` — migrate
// TRUOC zod, khong mat ca canvas.
//
// Ban cu luu MOT `Graph` tran; ban hien tai luu `{graph, workflowId, workflowName, workflowSaved}`
// de mo lai app khong mat danh tinh workflow dang mo. Ca hai dinh dang deu phai doc duoc.
import { afterEach, describe, expect, it, vi } from "vitest";

import { createGraphStore, loadPersistedCanvas } from "../src/store/graph";

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

describe("loadPersistedCanvas (P4a migrate)", () => {
  it("canvas v1: doi video->dola_video, GIU concat (node that), chi bo kind THAT SU da go", () => {
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
        // `concat` LA node that (crate node-concat) -> phai duoc GIU, khong bi migrate cat.
        { id: "n3", kind: { kind: "concat" }, x: 600, y: 0 },
        // `remove_logo` la kind P4 da go THAT SU -> phai bi bo, kem canh noi vao no.
        { id: "n4", kind: { kind: "remove_logo" }, x: 900, y: 0 },
      ],
      edges: [
        { id: "e1", from: "n1", from_port: "out", to: "n2", to_port: "prompt" },
        { id: "e2", from: "n2", from_port: "out", to: "n3", to_port: "in" },
        { id: "e3", from: "n3", from_port: "out", to: "n4", to_port: "in" },
      ],
    };
    vi.stubGlobal("localStorage", memoryStorage({ "lvc.workflow.canvas.v1": JSON.stringify(v1) }));

    const loaded = loadPersistedCanvas();
    expect(loaded).not.toBeNull();
    const g = loaded!.graph;
    expect(g.version).toBe(2);
    // video->dola_video, concat GIU, remove_logo BO.
    expect(g.nodes.map((n) => n.kind.kind)).toEqual(["prompt", "dola_video", "concat"]);
    // e1 (prompt->video) + e2 (video->concat) giu; e3 (->remove_logo) bi bo theo node da go.
    expect(g.edges.map((e) => e.id)).toEqual(["e1", "e2"]);
    // Dinh dang cu khong co danh tinh -> coi nhu chua dat ten, chua luu.
    expect(loaded!.workflowName).toBe("");
    expect(loaded!.workflowSaved).toBe(false);

    const store = createGraphStore({ workflowId: "test", workflowName: "test" });
    store.getState().loadGraph(g);
    expect(store.getState().nodes.map((n) => n.type)).toEqual(["prompt", "dola_video", "concat"]);
    expect(store.getState().edges.map((e) => e.id)).toEqual(["e1", "e2"]);
  });

  it("dinh dang moi: giu nguyen danh tinh workflow dang mo", () => {
    const payload = {
      graph: { version: 2, nodes: [], edges: [] },
      workflowId: "wf-that",
      workflowName: "Review sản phẩm",
      workflowSaved: true,
    };
    vi.stubGlobal(
      "localStorage",
      memoryStorage({ "lvc.workflow.canvas.v1": JSON.stringify(payload) }),
    );

    const loaded = loadPersistedCanvas();
    expect(loaded!.workflowId).toBe("wf-that");
    expect(loaded!.workflowName).toBe("Review sản phẩm");
    expect(loaded!.workflowSaved).toBe(true);
  });

  it("chua tung luu gi -> null, khong nem loi", () => {
    vi.stubGlobal("localStorage", memoryStorage({}));
    expect(loadPersistedCanvas()).toBeNull();
  });

  it("du lieu hong -> null thay vi lam vo app", () => {
    vi.stubGlobal(
      "localStorage",
      memoryStorage({ "lvc.workflow.canvas.v1": "{khong-phai-json" }),
    );
    expect(loadPersistedCanvas()).toBeNull();
  });
});