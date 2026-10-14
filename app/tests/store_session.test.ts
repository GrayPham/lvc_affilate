// `loadSession`/`saveSession` (src/store/graph.tsx) — tat app bat lai phai thay DUNG cac tab dang
// mo, kem DANH TINH workflow (id/ten/da-luu-chua). Thieu danh tinh thi mo lai se ra canvas "chua
// dat ten" mang dung noi dung cu, bam Luu se tao THEM mot workflow moi thay vi ghi de ban dang mo.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { loadSession, saveSession } from "../src/store/graph";
import type { Graph } from "../src/ipc/types";

function memoryStorage(initial: Record<string, string> = {}): Storage {
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

const EMPTY: Graph = { version: 2, nodes: [], edges: [] };

function canvas(name: string, saved = true) {
  return { graph: EMPTY, workflowId: `id-${name}`, workflowName: name, workflowSaved: saved };
}

beforeEach(() => {
  vi.stubGlobal("localStorage", memoryStorage());
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("saveSession + loadSession", () => {
  it("giu DU so tab, dung thu tu", () => {
    saveSession({ tabs: [canvas("A"), canvas("B"), canvas("C")], activeIndex: 1 });
    const got = loadSession();
    expect(got!.tabs.map((t) => t.workflowName)).toEqual(["A", "B", "C"]);
  });

  it("giu tab DANG XEM", () => {
    saveSession({ tabs: [canvas("A"), canvas("B")], activeIndex: 1 });
    expect(loadSession()!.activeIndex).toBe(1);
  });

  it("giu danh tinh workflow (id + da luu chua)", () => {
    saveSession({ tabs: [canvas("Review", true)], activeIndex: 0 });
    const t = loadSession()!.tabs[0]!;
    expect(t.workflowId).toBe("id-Review");
    expect(t.workflowSaved).toBe(true);
  });

  it("chua co phien nao -> null", () => {
    expect(loadSession()).toBeNull();
  });

  it("activeIndex ngoai pham vi -> ve tab dau, khong tra undefined", () => {
    saveSession({ tabs: [canvas("A")], activeIndex: 99 });
    expect(loadSession()!.activeIndex).toBe(0);
  });

  it("MOT tab hong chi bo tab do, khong vut ca phien", () => {
    vi.stubGlobal(
      "localStorage",
      memoryStorage({
        "lvc.workflow.session.v1": JSON.stringify({
          tabs: [canvas("A"), { graph: "khong-phai-do-thi" }, canvas("C")],
          activeIndex: 0,
        }),
      }),
    );
    const got = loadSession();
    expect(got!.tabs.map((t) => t.workflowName)).toEqual(["A", "C"]);
  });

  it("phien hong hoan toan -> roi ve khoa CU (mot canvas) de ban cu khong mat canvas", () => {
    vi.stubGlobal(
      "localStorage",
      memoryStorage({
        "lvc.workflow.session.v1": "{khong-phai-json",
        "lvc.workflow.canvas.v1": JSON.stringify(EMPTY),
      }),
    );
    const got = loadSession();
    expect(got!.tabs.length).toBe(1);
    expect(got!.tabs[0]!.workflowSaved).toBe(false);
  });

  it("chi co khoa CU (ban da cai tu truoc) -> van mo lai duoc canvas do", () => {
    vi.stubGlobal(
      "localStorage",
      memoryStorage({ "lvc.workflow.canvas.v1": JSON.stringify(EMPTY) }),
    );
    expect(loadSession()!.tabs.length).toBe(1);
  });
});
