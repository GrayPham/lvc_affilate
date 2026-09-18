// TDD P3c Task 2 buoc 1 — `store/scripts.ts`: (1) `dirty` bat len khi sua `draft` va tat sau khi
// `saveScenes()` thanh cong; (2) "tu luu roi chay" (`runScene` tu goi `saveScenes` TRUOC khi
// `script_run` neu dang `dirty`, giai quyet dung `id` MOI cho canh vua them — xem doc
// `store/scripts.ts::runScene`); (3) su kien cham task (mo phong bang `refreshDoc()` — logic
// debounce/loc that nam o `pages/ScriptsPage.tsx`) CHI ghi de `doc`, KHONG dong `draft`/`dirty`.
//
// Mock DUY NHAT lop `src/ipc/tauri.ts` (giong `store_accounts.test.ts`) — `commands.ts` (zod parse
// that) va `format/scene.ts` (`canStop`/`sceneIdsToRun` that) khong bi mock.
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ScriptDoc, ScriptMeta, SceneDto, SceneRow } from "../src/ipc/types";

const mockInvoke = vi.fn(
  (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> =>
    Promise.reject(new Error("mockInvoke: chua cau hinh cho lenh nay")),
);

vi.mock("../src/ipc/tauri", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args),
  listen: vi.fn(),
}));

import { useScriptsStore, type DraftScene } from "../src/store/scripts";

function meta(overrides: Partial<ScriptMeta> = {}): ScriptMeta {
  return {
    id: "scr1",
    name: "Demo",
    dir: "",
    createdAt: 0,
    updatedAt: 0,
    total: 1,
    done: 0,
    failed: 0,
    running: 0,
    ...overrides,
  };
}

function sceneDto(overrides: Partial<SceneDto> = {}): SceneDto {
  return {
    id: "sc1",
    idx: 1,
    prompt: "cảnh 1",
    ratio: null,
    duration: null,
    model: null,
    mode: null,
    logo: null,
    account: "",
    refs: [],
    taskId: null,
    taskStatus: null,
    failureCode: null,
    localPath: null,
    ...overrides,
  };
}

function sceneRow(overrides: Partial<SceneRow> = {}): SceneRow {
  return {
    id: "sc1",
    scriptId: "scr1",
    idx: 1,
    prompt: "cảnh 1",
    ratio: null,
    duration: null,
    model: null,
    mode: null,
    logo: null,
    account: "",
    refs: [],
    taskId: null,
    updatedAt: 0,
    ...overrides,
  };
}

function docOf(scenes: SceneDto[], metaOverrides: Partial<ScriptMeta> = {}): ScriptDoc {
  return { meta: meta({ total: scenes.length, ...metaOverrides }), scenes };
}

function draftOf(dto: SceneDto): DraftScene {
  return {
    key: dto.id,
    id: dto.id,
    prompt: dto.prompt,
    ratio: dto.ratio,
    duration: dto.duration,
    model: dto.model,
    mode: dto.mode,
    logo: dto.logo,
    account: dto.account,
    refs: dto.refs,
  };
}

const EMPTY_STATE = {
  scripts: [],
  loadingList: false,
  listError: null,
  selectedScriptId: null,
  doc: null,
  draft: [],
  dirty: false,
  loadingDoc: false,
  style: "",
  selectedIds: new Set<string>(),
  lastRunId: null,
  lastReport: null,
  busy: false,
  pendingIds: new Set<string>(),
  error: null,
};

beforeEach(() => {
  mockInvoke.mockReset();
  useScriptsStore.setState({ ...EMPTY_STATE });
});

describe("dirty — bat len khi sua draft, tat sau khi saveScenes() thanh cong", () => {
  it("setDraftField dat dirty=true; saveScenes goi script_put_scenes + script_get roi tat dirty", async () => {
    const doc = docOf([sceneDto()]);
    useScriptsStore.setState({
      selectedScriptId: "scr1",
      doc,
      draft: [draftOf(doc.scenes[0]!)],
      dirty: false,
    });

    expect(useScriptsStore.getState().dirty).toBe(false);
    useScriptsStore.getState().setDraftField("sc1", { prompt: "cảnh 1 đã sửa" });
    expect(useScriptsStore.getState().dirty).toBe(true);
    expect(useScriptsStore.getState().draft[0]!.prompt).toBe("cảnh 1 đã sửa");

    const savedRow = sceneRow({ prompt: "cảnh 1 đã sửa" });
    const savedDoc = docOf([sceneDto({ prompt: "cảnh 1 đã sửa" })]);
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "script_put_scenes") {
        return Promise.resolve([savedRow]);
      }
      if (cmd === "script_get") {
        return Promise.resolve(savedDoc);
      }
      return Promise.reject(new Error(`lenh khong mong doi: ${cmd}`));
    });

    const ok = await useScriptsStore.getState().saveScenes();
    expect(ok).toBe(true);
    expect(useScriptsStore.getState().dirty).toBe(false);
    expect(useScriptsStore.getState().doc?.scenes[0]?.prompt).toBe("cảnh 1 đã sửa");
    expect(mockInvoke).toHaveBeenCalledWith(
      "script_put_scenes",
      expect.objectContaining({ id: "scr1" }),
    );
  });

  it("addRow/removeRow cung dat dirty=true", () => {
    const doc = docOf([sceneDto()]);
    useScriptsStore.setState({ selectedScriptId: "scr1", doc, draft: [draftOf(doc.scenes[0]!)], dirty: false });

    useScriptsStore.getState().addRow();
    expect(useScriptsStore.getState().dirty).toBe(true);
    expect(useScriptsStore.getState().draft).toHaveLength(2);

    useScriptsStore.setState({ dirty: false });
    const key = useScriptsStore.getState().draft[0]!.key;
    useScriptsStore.getState().removeRow(key);
    expect(useScriptsStore.getState().dirty).toBe(true);
    expect(useScriptsStore.getState().draft).toHaveLength(1);
  });
});

describe("runScene — tu luu (auto-save-then-run) khi dang dirty", () => {
  it("canh MOI THEM (chua co id): luu TRUOC de co id MOI, roi chay DUNG id do", async () => {
    const doc = docOf([sceneDto()]);
    // Nguoi dung them 1 dong moi (chua luu) SAU dong da co san — dirty=true.
    useScriptsStore.setState({
      selectedScriptId: "scr1",
      doc,
      draft: [draftOf(doc.scenes[0]!)],
      dirty: false,
    });
    useScriptsStore.getState().addRow();
    const newKey = useScriptsStore.getState().draft[1]!.key;
    useScriptsStore.getState().setDraftField(newKey, { prompt: "cảnh mới" });
    expect(useScriptsStore.getState().dirty).toBe(true);

    const calls: string[] = [];
    const savedDoc = docOf([sceneDto(), sceneDto({ id: "sc_new", idx: 2, prompt: "cảnh mới" })]);
    mockInvoke.mockImplementation((cmd, args) => {
      calls.push(cmd);
      if (cmd === "script_put_scenes") {
        return Promise.resolve([sceneRow(), sceneRow({ id: "sc_new", idx: 2, prompt: "cảnh mới" })]);
      }
      if (cmd === "script_get") {
        return Promise.resolve(savedDoc);
      }
      if (cmd === "script_run") {
        expect(args).toEqual(
          expect.objectContaining({ id: "scr1", sceneIds: ["sc_new"], style: "" }),
        );
        return Promise.resolve({ runId: "run1", tasks: [{ sceneId: "sc_new", taskId: "t1" }], errors: [] });
      }
      if (cmd === "script_list") {
        return Promise.resolve([meta({ total: 2 })]);
      }
      return Promise.reject(new Error(`lenh khong mong doi: ${cmd}`));
    });

    await useScriptsStore.getState().runScene(newKey);

    // Thu tu: luu TRUOC (put_scenes + get de biet id moi), CHAY SAU.
    const putIdx = calls.indexOf("script_put_scenes");
    const runIdx = calls.indexOf("script_run");
    expect(putIdx).toBeGreaterThanOrEqual(0);
    expect(runIdx).toBeGreaterThan(putIdx);

    expect(useScriptsStore.getState().dirty).toBe(false);
    expect(useScriptsStore.getState().lastRunId).toBe("run1");
    expect(useScriptsStore.getState().error).toBeNull();
  });

  it("khong dirty -> chay THANG, khong goi script_put_scenes", async () => {
    const doc = docOf([sceneDto()]);
    useScriptsStore.setState({
      selectedScriptId: "scr1",
      doc,
      draft: [draftOf(doc.scenes[0]!)],
      dirty: false,
    });

    const calls: string[] = [];
    mockInvoke.mockImplementation((cmd) => {
      calls.push(cmd);
      if (cmd === "script_run") {
        return Promise.resolve({ runId: "run2", tasks: [{ sceneId: "sc1", taskId: "t1" }], errors: [] });
      }
      if (cmd === "script_get") {
        return Promise.resolve(doc);
      }
      if (cmd === "script_list") {
        return Promise.resolve([meta()]);
      }
      return Promise.reject(new Error(`lenh khong mong doi: ${cmd}`));
    });

    await useScriptsStore.getState().runScene("sc1");
    expect(calls).not.toContain("script_put_scenes");
    expect(calls).toContain("script_run");
  });
});

describe("refreshDoc — dong bo doc tu server, KHONG dong draft/dirty (su kien cham task)", () => {
  it("doc doi taskStatus nhung draft van giu nguyen edit chua luu", async () => {
    const doc = docOf([sceneDto({ taskId: "t1", taskStatus: "queued" })]);
    useScriptsStore.setState({
      selectedScriptId: "scr1",
      doc,
      draft: [{ ...draftOf(doc.scenes[0]!), prompt: "prompt dang go do, chua luu" }],
      dirty: true,
    });

    const updatedDoc = docOf([sceneDto({ taskId: "t1", taskStatus: "processing" })]);
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "script_get") {
        return Promise.resolve(updatedDoc);
      }
      return Promise.reject(new Error(`lenh khong mong doi: ${cmd}`));
    });

    await useScriptsStore.getState().refreshDoc();

    const state = useScriptsStore.getState();
    expect(state.doc?.scenes[0]?.taskStatus).toBe("processing");
    // draft/dirty KHONG bi dong de mat edit dang do cua nguoi dung.
    expect(state.dirty).toBe(true);
    expect(state.draft[0]!.prompt).toBe("prompt dang go do, chua luu");
  });
});
