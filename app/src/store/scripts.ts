// Store zustand cho tab "Kịch bản" (P3c Task 2) — danh sach kich ban (trai), tai lieu (`doc`) +
// ban nhap co the sua (`draft`) cua kich ban dang mo (phai), cac hanh dong CRUD/chay/dung/dat lai.
//
// Hai luong DU LIEU tach biet co CHU DICH cho kich ban dang mo:
// - `doc` (kieu `ScriptDoc`, tu `scriptGet`) la SU THAT server, GOM CA trang thai task song
//   (`taskStatus`/`failureCode`/`localPath`) — moi lan `refreshDoc()` (tu bridge su kien, xem
//   `pages/ScriptsPage.tsx`) GHI DE `doc` hoan toan.
// - `draft` (kieu `DraftScene[]`, noi bo) la ban dang SUA cua nguoi dung (prompt/tham so tung
//   canh) — CHI duoc dong bo lai tu `doc` luc `selectScript()`/`saveScenes()` thanh cong (KHONG
//   bao gio bi `refreshDoc()` ghi de), de mot su kien task_* toi giua luc nguoi dung dang go
//   khong lam mat edit dang do. `dirty` bao hieu `draft` da lech `doc` hien tai, dung de:
//   1. Khoa nut "Lưu cảnh" khi khong co gi de luu.
//   2. Bat "tu luu roi chay" (`runScene`/`runAllPending`) — `run_script` (Rust) doc canh TU DB,
//      khong tu `draft`, nen phai luu truoc khi chay neu dang co edit chua luu (xem
//      task-2-brief.md: "auto-save-then-run", khong hoi lai nguoi dung).
//
// Quy uoc loi: MOI hanh dong ghi (`createScript`/`renameScript`/`deleteScript`/`importTxt`/
// `saveScenes`/`runScene`/`runAllPending`/`stopScene`/`stopAll`/`openSceneFile`/
// `resetSelectedOrFailed`) TU BAT loi va ghi vao `error` (mot chuoi CHUNG cho ca trang, giong
// `store/settings.ts::save` — KHONG rethrow) roi tra ve `true`/`false`/gia tri de UI biet co thanh
// cong hay khong; day la lua chon co chu dich khac voi `store/accounts.ts` (rethrow, loi hien
// INLINE tung dong) vi bang canh co RAT NHIEU nut hanh dong moi dong (Chay/Chay lai/Dung/Mo thu
// muc) — mirror `store/tasks.ts` (mot `error` chung phia tren bang + `pendingIds` khoa tung nut).
import { create } from "zustand";

import {
  runCancel,
  scriptCreate,
  scriptDelete,
  scriptGet,
  scriptImportTxt,
  scriptList,
  scriptPutScenes,
  scriptResetScenes,
  scriptRun,
  scriptUpdate,
  taskCancel,
  taskOpenFile,
} from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { canStop, sceneIdsToRun } from "../format/scene";
import type { ScriptDoc, ScriptMeta, ScriptRunReport, SceneDto, SceneIn } from "../ipc/types";

/** Ban nhap MOT canh trong bang dang sua — `key` la khoa React/noi bo ON DINH: bang `id` cho canh
 * DA co server (moi id server la duy nhat), hoac `new_<n>` cho canh MOI THEM (chua luu, `id:
 * null`) — tach khoi `id` vi `id` co the la `null`. */
export interface DraftScene {
  key: string;
  id: string | null;
  prompt: string;
  ratio: string | null;
  duration: number | null;
  model: string | null;
  mode: string | null;
  logo: string | null;
  account: string;
  refs: string[];
}

/** `SceneDto` (tu `doc`) -> `DraftScene` (ban nhap) — bo cac truong CHI DOC (`idx`/`taskId`/
 * `taskStatus`/`failureCode`/`localPath`, bang doc thang tu `doc.scenes` khi can hien thi). */
export function docToDraft(doc: ScriptDoc): DraftScene[] {
  return doc.scenes.map((s) => ({
    key: s.id,
    id: s.id,
    prompt: s.prompt,
    ratio: s.ratio,
    duration: s.duration,
    model: s.model,
    mode: s.mode,
    logo: s.logo,
    account: s.account,
    refs: s.refs,
  }));
}

/** `true` khi noi dung `row` (ban nhap) con khop CHINH XAC voi `scene` (su that server) — dung boi
 * `saveScenes()` (fix-round 1, review C1 "belt and braces") de phat hien canh dang chay ma nguoi
 * dung da tung sua truoc khi UI kip khoa o nhap (`SceneRow.tsx`, dua vao `canStop`). */
function draftMatchesScene(row: DraftScene, scene: SceneDto): boolean {
  return (
    row.prompt === scene.prompt &&
    row.ratio === scene.ratio &&
    row.duration === scene.duration &&
    row.model === scene.model &&
    row.mode === scene.mode &&
    row.logo === scene.logo &&
    row.account === scene.account &&
    row.refs.length === scene.refs.length &&
    row.refs.every((r, i) => r === scene.refs[i])
  );
}

export function toSceneIn(d: DraftScene): SceneIn {
  return {
    id: d.id,
    prompt: d.prompt,
    ratio: d.ratio,
    duration: d.duration,
    model: d.model,
    mode: d.mode,
    logo: d.logo,
    account: d.account,
    refs: d.refs,
  };
}

let newRowSeq = 0;

function emptyDraftRow(): DraftScene {
  newRowSeq += 1;
  return {
    key: `new_${newRowSeq}`,
    id: null,
    prompt: "",
    ratio: null,
    duration: null,
    model: null,
    mode: null,
    logo: null,
    account: "",
    refs: [],
  };
}

function withAdded(ids: ReadonlySet<string>, id: string): Set<string> {
  const next = new Set(ids);
  next.add(id);
  return next;
}

function withRemoved(ids: ReadonlySet<string>, id: string): Set<string> {
  const next = new Set(ids);
  next.delete(id);
  return next;
}

interface ScriptsState {
  // -- Danh sach (trai) --
  scripts: ScriptMeta[];
  loadingList: boolean;
  listError: string | null;
  refreshList: () => Promise<void>;

  createScript: (name: string, dir: string) => Promise<boolean>;
  renameScript: (id: string, name: string, dir: string) => Promise<boolean>;
  /** `false` khi that bai (vd `SCENE_RUNNING`) — chi tiet nam trong `error`. */
  deleteScript: (id: string) => Promise<boolean>;
  /** Tra `null` khi that bai; nguoc lai so canh MOI (thay TOAN BO, R4). */
  importTxt: (id: string, text: string) => Promise<number | null>;

  // -- Kich ban dang mo (phai) --
  selectedScriptId: string | null;
  doc: ScriptDoc | null;
  draft: DraftScene[];
  dirty: boolean;
  loadingDoc: boolean;
  selectScript: (id: string | null) => Promise<void>;
  /** Doc lai `doc` tu server (KHONG dong `draft`/`dirty`) — goi tu bridge su kien (debounce, xem
   * `ScriptsPage`) khi mot task cua kich ban dang mo thay doi trang thai. */
  refreshDoc: () => Promise<void>;

  style: string;
  setStyle: (style: string) => void;

  selectedIds: Set<string>;
  toggleSelected: (id: string) => void;
  clearSelected: () => void;

  setDraftField: (key: string, patch: Partial<Omit<DraftScene, "key" | "id">>) => void;
  addRow: () => void;
  /** Khong lam gi neu canh (`key` = id server) DANG CHAY (`queued`/`processing`) — an toan phong
   * khi UI goi nham (nut tuong ung da bi vo hieu qua `canStop`). */
  removeRow: (key: string) => void;
  saveScenes: () => Promise<boolean>;

  lastRunId: string | null;
  lastReport: ScriptRunReport | null;
  /**
   * Nhan `key` cua MOT dong trong `draft` (khong phai `id` — canh MOI THEM chua luu co `key !=
   * id`, xem `DraftScene`). Neu `dirty` -> tu luu (`saveScenes`) TRUOC, khong hoi lai nguoi dung
   * (xem doc dau file); sau khi luu, `replace_scenes` (Rust) giu NGUYEN THU TU mang truyen vao
   * (ke ca canh MOI) nen VI TRI cua dong trong `draft` truoc/sau luu la ON DINH — dung vi tri do
   * de tim `id` THAT (server vua gan) cho canh MOI thay vi dua vao `key` cu (da doi sau khi luu).
   */
  runScene: (key: string) => Promise<void>;
  /** `scriptRun(id, [], style)` — "Chạy tất cả (chưa xong)". */
  runAllPending: () => Promise<void>;
  stopScene: (taskId: string) => Promise<void>;
  /** `run_cancel(lastRunId)` — khong lam gi khi chua co lan chay nao (`lastRunId === null`). */
  stopAll: () => Promise<void>;
  /**
   * Fix-round 1 (review I1): xoa `lastRunId` NEU no KHOP `runId` — goi tu `pages/ScriptsPage.tsx`
   * ngay khi mot su kien `run_done` toi (KHONG debounce, chi doi trang thai cuc bo, khong goi
   * IPC nao ca). "Dừng tất cả" het y nghia SAU KHI chinh lan chay ma no tro toi da ket thuc — neu
   * khong xoa, nut van bat sang mai mai (giong loi da tung sua o `store/run.ts`/
   * `engine_bridge.ts` cho tab Workflow).
   */
  clearRunIdIfMatches: (runId: string) => void;
  openSceneFile: (taskId: string) => Promise<void>;
  /** `scriptResetScenes` cho `sceneIdsToRun(doc, selectedIds)` (uu tien da chon, roi ve moi canh
   * `failed`) — khong lam gi neu danh sach do rong. */
  resetSelectedOrFailed: () => Promise<void>;

  busy: boolean;
  pendingIds: Set<string>;
  error: string | null;
}

export const useScriptsStore = create<ScriptsState>((set, get) => {
  /** Goi `script_run` roi cap nhat `lastRunId`/`lastReport`/`doc`/`scripts` — dung chung boi
   * `runScene`/`runAllPending` (dong bao dong tren `set`/`get` cua chinh store nay, tranh phai tu
   * khai kieu tay cho tham so `set`/`get` — kieu that cua `zustand`'s `setState` la overload theo
   * kieu bivariant-hack, de vo tinh lech kieu neu khai lai thu cong o ngoai). */
  async function runAndRefresh(scriptId: string, sceneIds: readonly string[]): Promise<void> {
    set({ busy: true, error: null });
    try {
      const report = await scriptRun(scriptId, [...sceneIds], get().style);
      set({ lastReport: report, lastRunId: report.runId ?? get().lastRunId });
      await get().refreshDoc();
      await get().refreshList();
    } catch (e) {
      set({ error: errorText(e) });
    } finally {
      set({ busy: false });
    }
  }

  return {
    scripts: [],
    loadingList: false,
    listError: null,

    selectedScriptId: null,
    doc: null,
    draft: [],
    dirty: false,
    loadingDoc: false,

    style: "",
    selectedIds: new Set(),

    lastRunId: null,
    lastReport: null,

    busy: false,
    pendingIds: new Set(),
    error: null,

    refreshList: async () => {
      set({ loadingList: true, listError: null });
      try {
        const scripts = await scriptList();
        set({ scripts, loadingList: false });
      } catch (e) {
        set({ loadingList: false, listError: errorText(e) });
      }
    },

    createScript: async (name, dir) => {
      set({ busy: true, error: null });
      try {
        const row = await scriptCreate(name, dir);
        await get().refreshList();
        await get().selectScript(row.id);
        set({ busy: false });
        return true;
      } catch (e) {
        set({ busy: false, error: errorText(e) });
        return false;
      }
    },

    renameScript: async (id, name, dir) => {
      set({ busy: true, error: null });
      try {
        await scriptUpdate(id, name, dir);
        await get().refreshList();
        if (get().selectedScriptId === id) {
          await get().refreshDoc();
        }
        set({ busy: false });
        return true;
      } catch (e) {
        set({ busy: false, error: errorText(e) });
        return false;
      }
    },

    deleteScript: async (id) => {
      set({ busy: true, error: null });
      try {
        await scriptDelete(id);
        if (get().selectedScriptId === id) {
          set({
            selectedScriptId: null,
            doc: null,
            draft: [],
            dirty: false,
            lastRunId: null,
            lastReport: null,
            selectedIds: new Set(),
          });
        }
        await get().refreshList();
        set({ busy: false });
        return true;
      } catch (e) {
        set({ busy: false, error: errorText(e) });
        return false;
      }
    },

    importTxt: async (id, text) => {
      set({ busy: true, error: null });
      try {
        const n = await scriptImportTxt(id, text);
        await get().refreshList();
        if (get().selectedScriptId === id) {
          const doc = await scriptGet(id);
          set({ doc, draft: docToDraft(doc), dirty: false });
        }
        set({ busy: false });
        return n;
      } catch (e) {
        set({ busy: false, error: errorText(e) });
        return null;
      }
    },

    selectScript: async (id) => {
      set({
        selectedScriptId: id,
        doc: null,
        draft: [],
        dirty: false,
        style: "",
        selectedIds: new Set(),
        lastRunId: null,
        lastReport: null,
        error: null,
      });
      if (id === null) {
        return;
      }
      set({ loadingDoc: true });
      try {
        const doc = await scriptGet(id);
        set({ doc, draft: docToDraft(doc), loadingDoc: false });
      } catch (e) {
        set({ loadingDoc: false, error: errorText(e) });
      }
    },

    refreshDoc: async () => {
      const id = get().selectedScriptId;
      if (id === null) {
        return;
      }
      try {
        const doc = await scriptGet(id);
        set({ doc });
        // Fix-round 1 (review I1, dieu kien thu hai): khong con canh nao dang chay trong kich
        // ban nay -> "Dừng tất cả" khong con gi de huy nua, DU su kien `run_done` co the chua toi
        // kip (tab bi nen lau) hoac bi mat — dua truc tiep vao trang thai THAT cua doc thay vi chi
        // dua vao su kien.
        if (get().lastRunId !== null && !doc.scenes.some((s) => canStop(s))) {
          set({ lastRunId: null });
        }
      } catch (e) {
        set({ error: errorText(e) });
      }
    },

    setStyle: (style) => set({ style }),

    toggleSelected: (id) => {
      set((s) => {
        const next = new Set(s.selectedIds);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
        }
        return { selectedIds: next };
      });
    },
    clearSelected: () => set({ selectedIds: new Set() }),

    setDraftField: (key, patch) => {
      set((s) => ({
        draft: s.draft.map((d) => (d.key === key ? { ...d, ...patch } : d)),
        dirty: true,
      }));
    },

    addRow: () => {
      set((s) => ({ draft: [...s.draft, emptyDraftRow()], dirty: true }));
    },

    removeRow: (key) => {
      const docScene = get().doc?.scenes.find((s) => s.id === key);
      if (docScene && canStop(docScene)) {
        return;
      }
      set((s) => ({ draft: s.draft.filter((d) => d.key !== key), dirty: true }));
    },

    saveScenes: async () => {
      const { selectedScriptId, draft, doc } = get();
      if (selectedScriptId === null) {
        return false;
      }
      // Fix-round 1 (review C1 "belt and braces"): `SceneRow.tsx` khoa o nhap khi `canStop`, nhung
      // co the co MOT canh vua CHUYEN sang chay NGAY SAU khi nguoi dung bat dau go (truoc khi khoa
      // do kip ap dung lai) — kiem lai O DAY, TRUOC khi goi IPC: neu canh dang chay VA noi dung
      // draft da lech server, TU CHOI CA LUOT (khong goi `script_put_scenes`, khong dong `draft`/
      // `dirty`) va bao loi ro rang, thay vi de Rust am tham GIU NGUYEN noi dung server (dung R2)
      // roi `saveScenes()` lai bao "thanh cong" sai lech (dung y review — mat edit khong bao loi).
      if (doc) {
        for (let i = 0; i < draft.length; i += 1) {
          const row = draft[i]!;
          const docScene = doc.scenes.find((s) => s.id === row.id);
          if (docScene && canStop(docScene) && !draftMatchesScene(row, docScene)) {
            set({ error: `Cảnh #${i + 1} đang chạy — không lưu được thay đổi của cảnh đó` });
            return false;
          }
        }
      }
      set({ busy: true, error: null });
      try {
        await scriptPutScenes(selectedScriptId, draft.map(toSceneIn));
        const doc = await scriptGet(selectedScriptId);
        set({ doc, draft: docToDraft(doc), dirty: false, busy: false });
        return true;
      } catch (e) {
        set({ busy: false, error: errorText(e) });
        return false;
      }
    },

    runScene: async (key) => {
      const { selectedScriptId, dirty, draft, saveScenes: save } = get();
      if (selectedScriptId === null) {
        return;
      }
      const idx = draft.findIndex((d) => d.key === key);
      if (idx === -1) {
        return;
      }
      if (dirty) {
        const ok = await save();
        if (!ok) {
          return;
        }
      }
      const targetId = get().draft[idx]?.id ?? null;
      if (targetId === null) {
        // Khong nen xay ra (moi dong trong draft SAU khi luu deu co id) — phong thu.
        return;
      }
      await runAndRefresh(selectedScriptId, [targetId]);
    },

    runAllPending: async () => {
      const { selectedScriptId, dirty, saveScenes: save } = get();
      if (selectedScriptId === null) {
        return;
      }
      if (dirty) {
        const ok = await save();
        if (!ok) {
          return;
        }
      }
      await runAndRefresh(selectedScriptId, []);
    },

    stopScene: async (taskId) => {
      set((s) => ({ pendingIds: withAdded(s.pendingIds, taskId), error: null }));
      try {
        await taskCancel(taskId);
        await get().refreshDoc();
      } catch (e) {
        set({ error: errorText(e) });
      } finally {
        set((s) => ({ pendingIds: withRemoved(s.pendingIds, taskId) }));
      }
    },

    stopAll: async () => {
      const runId = get().lastRunId;
      if (runId === null) {
        return;
      }
      set({ busy: true, error: null });
      try {
        await runCancel(runId);
        await get().refreshDoc();
        await get().refreshList();
      } catch (e) {
        set({ error: errorText(e) });
      } finally {
        set({ busy: false });
      }
    },

    clearRunIdIfMatches: (runId) => {
      if (get().lastRunId === runId) {
        set({ lastRunId: null });
      }
    },

    openSceneFile: async (taskId) => {
      set((s) => ({ pendingIds: withAdded(s.pendingIds, taskId), error: null }));
      try {
        await taskOpenFile(taskId);
      } catch (e) {
        set({ error: errorText(e) });
      } finally {
        set((s) => ({ pendingIds: withRemoved(s.pendingIds, taskId) }));
      }
    },

    resetSelectedOrFailed: async () => {
      const { selectedScriptId, doc, selectedIds } = get();
      if (selectedScriptId === null || doc === null) {
        return;
      }
      const ids = sceneIdsToRun(doc, selectedIds);
      if (ids.length === 0) {
        return;
      }
      set({ busy: true, error: null });
      try {
        await scriptResetScenes(selectedScriptId, ids);
        await get().refreshDoc();
        await get().refreshList();
        set({ selectedIds: new Set() });
      } catch (e) {
        set({ error: errorText(e) });
      } finally {
        set({ busy: false });
      }
    },
  };
});

// re-export cho tests/store_scripts.test.ts (dung `SceneDto` khi dung fixture-like du lieu).
export type { SceneDto };
