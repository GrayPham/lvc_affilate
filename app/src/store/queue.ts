// Store zustand cho hang cho workflow (man "Tác vụ") — moi hanh dong GHI deu `refresh()` lai sau
// khi thanh cong (cung quy uoc `store/credentials.ts`).
//
// Bo chay o BACKEND (task nen trong Rust) nen UI phai HOI LAI dinh ky de thay tien do: `startPoll`
// bat mot vong `setInterval` nhe (2s) chi khi hang cho dang chay; dung thi tu tat.
import { create } from "zustand";

import {
  queueAdd,
  queueClearFinished,
  queueList,
  queueRemove,
  queueReorder,
  queueStart,
  queueStop,
} from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { Graph, QueueItem } from "../ipc/types";

const POLL_MS = 2000;

interface QueueStoreState {
  items: QueueItem[];
  running: boolean;
  loading: boolean;
  error: string | null;
  /** Handle cua vong hoi dinh ky — `null` = khong hoi. */
  pollTimer: ReturnType<typeof setInterval> | null;

  refresh: () => Promise<void>;
  add: (workflowId: string, name: string, graph: Graph) => Promise<void>;
  remove: (id: string) => Promise<void>;
  reorder: (ids: string[]) => Promise<void>;
  clearFinished: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  /** Bat/tat vong hoi — goi luc mount/unmount trang. */
  startPoll: () => void;
  stopPoll: () => void;
}

export const useQueueStore = create<QueueStoreState>((set, get) => ({
  items: [],
  running: false,
  loading: false,
  error: null,
  pollTimer: null,

  refresh: async () => {
    set({ loading: true });
    try {
      const state = await queueList();
      set({ items: state.items, running: state.running, loading: false, error: null });
      // Hang cho vua dung han -> khong can hoi nua.
      if (!state.running && get().pollTimer !== null) {
        get().stopPoll();
      }
    } catch (e) {
      set({ loading: false, error: errorText(e) });
    }
  },

  add: async (workflowId, name, graph) => {
    try {
      await queueAdd(workflowId, name, graph);
      await get().refresh();
    } catch (e) {
      set({ error: errorText(e) });
      throw e;
    }
  },

  remove: async (id) => {
    await queueRemove(id);
    await get().refresh();
  },

  reorder: async (ids) => {
    // Doi NGAY tren man cho muot tay keo; `refresh` sau do se chot lai theo backend.
    set((s) => {
      const byId = new Map(s.items.map((i) => [i.id, i]));
      const next = ids.map((id) => byId.get(id)).filter((i): i is QueueItem => i !== undefined);
      return { items: next };
    });
    await queueReorder(ids);
    await get().refresh();
  },

  clearFinished: async () => {
    await queueClearFinished();
    await get().refresh();
  },

  start: async () => {
    await queueStart();
    await get().refresh();
    get().startPoll();
  },

  stop: async () => {
    await queueStop();
    await get().refresh();
  },

  startPoll: () => {
    if (get().pollTimer !== null) {
      return;
    }
    const t = setInterval(() => void get().refresh(), POLL_MS);
    set({ pollTimer: t });
  },

  stopPoll: () => {
    const t = get().pollTimer;
    if (t !== null) {
      clearInterval(t);
      set({ pollTimer: null });
    }
  },
}));
