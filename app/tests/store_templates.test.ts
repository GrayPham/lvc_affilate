// TDD P4d — store `store/templates.ts` (kho template). Mock DUY NHAT lop `src/ipc/tauri.ts` (seam
// da tai lieu hoa) — `commands.ts` (zod parse that) va `format/template.ts` (`describeTemplateError`/
// `isIpcError` that) khong bi mock, nen test di qua toan bo duong That tru `invoke()` cuoi cung.
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Graph, TemplateMeta } from "../src/ipc/types";

const mockInvoke = vi.fn(
  (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> =>
    Promise.reject(new Error("mockInvoke: chua cau hinh cho lenh nay")),
);

vi.mock("../src/ipc/tauri", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args),
  listen: vi.fn(),
}));

import { useTemplatesStore } from "../src/store/templates";

function makeMeta(overrides: Partial<TemplateMeta> = {}): TemplateMeta {
  return {
    id: "tpl_1",
    name: "Mau cua toi",
    description: "",
    scope: "private",
    category: "Cơ bản",
    thumbnail: "",
    updatedAt: 1700000000,
    ...overrides,
  };
}

const SAMPLE_GRAPH: Graph = { version: 2, nodes: [], edges: [] };

beforeEach(() => {
  mockInvoke.mockReset();
  useTemplatesStore.setState({ items: [], loading: false, error: null });
});

describe("load(scope)", () => {
  it("truyen scope xuong IPC va parse danh sach qua zod that", async () => {
    let listArgs: Record<string, unknown> | undefined;
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === "template_list") {
        listArgs = args;
        return Promise.resolve([
          makeMeta({ id: "public:x", scope: "public" }),
          makeMeta({ id: "tpl_2" }),
        ]);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });

    await useTemplatesStore.getState().load("public");
    expect(listArgs?.scope).toBe("public");
    expect(useTemplatesStore.getState().items).toHaveLength(2);
    expect(useTemplatesStore.getState().error).toBeNull();
  });

  it("khong dat scope -> gui null", async () => {
    let listArgs: Record<string, unknown> | undefined;
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === "template_list") {
        listArgs = args;
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });
    await useTemplatesStore.getState().load();
    expect(listArgs?.scope).toBeNull();
  });

  it("loi IPC -> `error` la thong diep tieng Viet (khong nem)", async () => {
    mockInvoke.mockImplementation(() =>
      Promise.reject({ code: "STORE", message: "raw", details: null }),
    );
    await useTemplatesStore.getState().load("private");
    expect(useTemplatesStore.getState().error).toBe("Lỗi lưu dữ liệu.");
    expect(useTemplatesStore.getState().loading).toBe(false);
  });
});

describe("getDoc", () => {
  it("lay TemplateDoc + parse qua zod that", async () => {
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === "template_get") {
        expect(args?.id).toBe("tpl_1");
        return Promise.resolve({ meta: makeMeta(), graph: SAMPLE_GRAPH });
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });
    const doc = await useTemplatesStore.getState().getDoc("tpl_1");
    expect(doc.meta.id).toBe("tpl_1");
    expect(doc.graph.version).toBe(2);
  });
});

describe("save", () => {
  it("gui dung tham so (id null=tao moi) va tra ve meta", async () => {
    let saveArgs: Record<string, unknown> | undefined;
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === "template_save_private") {
        saveArgs = args;
        return Promise.resolve(makeMeta({ id: "tpl_new", name: "moi", description: "mo ta" }));
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });

    const meta = await useTemplatesStore.getState().save({
      id: null,
      name: "moi",
      graph: SAMPLE_GRAPH,
      description: "mo ta",
      thumbnail: "",
    });

    expect(saveArgs?.id).toBeNull();
    expect(saveArgs?.name).toBe("moi");
    expect(saveArgs?.description).toBe("mo ta");
    expect(saveArgs?.thumbnail).toBe("");
    expect(saveArgs?.graph).toEqual(SAMPLE_GRAPH);
    expect(meta.id).toBe("tpl_new");
  });
});

describe("remove", () => {
  it("xoa thanh cong -> bo khoi items, tra ve true", async () => {
    useTemplatesStore.setState({ items: [makeMeta(), makeMeta({ id: "tpl_2" })] });
    mockInvoke.mockImplementation((cmd, args) => {
      if (cmd === "template_delete_private") {
        expect(args?.id).toBe("tpl_1");
        return Promise.resolve(true);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });

    const deleted = await useTemplatesStore.getState().remove("tpl_1");
    expect(deleted).toBe(true);
    expect(useTemplatesStore.getState().items.map((t) => t.id)).toEqual(["tpl_2"]);
  });

  it("khong co gi de xoa -> false, items giu nguyen", async () => {
    useTemplatesStore.setState({ items: [makeMeta()] });
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "template_delete_private") {
        return Promise.resolve(false);
      }
      return Promise.reject(new Error(`lenh la: ${cmd}`));
    });
    const deleted = await useTemplatesStore.getState().remove("tpl_1");
    expect(deleted).toBe(false);
    expect(useTemplatesStore.getState().items).toHaveLength(1);
  });
});
