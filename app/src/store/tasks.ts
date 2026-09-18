// Store zustand cho man "Tác vụ" (Task 13): danh sach task cua BO LOC hien tai (theo run, server
// side qua `taskList(runId, limit)`), loc theo trang thai (client side, `TaskDto.status` khong co
// tham so rieng o `task_list` — xem `crates/lvc-app/src/commands/tasks.rs`), va hai thao tac
// `taskCancel`/`taskResume`. `refresh()` la ham DUY NHAT goi IPC doc danh sach — TasksPage.tsx goi
// no khi mount VA khi `filter.runId` doi VA (debounce 300ms) moi khi co `EngineEvent` cham toi mot
// task, thay vi tu ap du lieu su kien vao tung dong rieng (danh doi don gian lay dong bo — server
// (SQLite) la nguon su that duy nhat cho bang nay, khac voi `store/run.ts` von PHAI tu ap vi
// ProgressPanel can cap nhat tuc thi tren canvas dang mo).
import { create } from "zustand";

import { taskCancel, taskList, taskOpenFile, taskResume } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { TaskDto } from "../ipc/types";

export type StatusFilter = "all" | "queued" | "processing" | "completed" | "failed";

export interface TaskFilter {
  /** `null` = tất cả run (`task_list(run_id=None)` — tab "mới nhất gần đây"). */
  runId: string | null;
  status: StatusFilter;
}

export const DEFAULT_TASK_FILTER: TaskFilter = { runId: null, status: "all" };

/** So dong toi da moi lan doc — khop tran server (`TASK_LIST_LIMIT_MAX`, tasks.rs) la 200. */
const TASK_LIST_LIMIT = 200;

/** Loc CLIENT SIDE theo trang thai — tach rieng ham thuan de test/tai su dung khong can store. */
export function filterByStatus(rows: readonly TaskDto[], status: StatusFilter): TaskDto[] {
  if (status === "all") {
    return [...rows];
  }
  return rows.filter((r) => r.status === status);
}

interface TasksState {
  rows: TaskDto[];
  filter: TaskFilter;
  loading: boolean;
  error: string | null;
  /** id cac task dang co thao tac (huỷ/tiếp tục/mở) dang cho IPC tra loi — vo hieu nut khi dang cho. */
  pendingIds: Set<string>;

  setRunId: (runId: string | null) => void;
  setStatus: (status: StatusFilter) => void;
  refresh: () => Promise<void>;
  cancelTask: (id: string) => Promise<void>;
  resumeTask: (id: string) => Promise<void>;
  openTaskFile: (id: string) => Promise<void>;
}

export const useTasksStore = create<TasksState>((set, get) => ({
  rows: [],
  filter: DEFAULT_TASK_FILTER,
  loading: false,
  error: null,
  pendingIds: new Set(),

  setRunId: (runId) => {
    set((s) => ({ filter: { ...s.filter, runId } }));
    void get().refresh();
  },

  setStatus: (status) => {
    set((s) => ({ filter: { ...s.filter, status } }));
  },

  refresh: async () => {
    set({ loading: true, error: null });
    try {
      const rows = await taskList(get().filter.runId, TASK_LIST_LIMIT);
      set({ rows, loading: false });
    } catch (e) {
      set({ loading: false, error: errorText(e) });
    }
  },

  cancelTask: async (id) => {
    set((s) => ({ pendingIds: new Set(s.pendingIds).add(id) }));
    try {
      await taskCancel(id);
    } catch (e) {
      set({ error: errorText(e) });
    } finally {
      set((s) => {
        const next = new Set(s.pendingIds);
        next.delete(id);
        return { pendingIds: next };
      });
      await get().refresh();
    }
  },

  resumeTask: async (id) => {
    set((s) => ({ pendingIds: new Set(s.pendingIds).add(id) }));
    try {
      await taskResume(id);
    } catch (e) {
      set({ error: errorText(e) });
    } finally {
      set((s) => {
        const next = new Set(s.pendingIds);
        next.delete(id);
        return { pendingIds: next };
      });
      await get().refresh();
    }
  },

  openTaskFile: async (id) => {
    set((s) => ({ pendingIds: new Set(s.pendingIds).add(id) }));
    try {
      await taskOpenFile(id);
    } catch (e) {
      set({ error: errorText(e) });
    } finally {
      set((s) => {
        const next = new Set(s.pendingIds);
        next.delete(id);
        return { pendingIds: next };
      });
    }
  },
}));
