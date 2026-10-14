// Nhat ky SU KIEN ENGINE TOAN CUC (P-tabs) — tach khoi `store/run.ts` (gio la MOT instance RIENG
// cho tung tab workflow, xem doc o do) vi `TasksPage`/`ScriptsPage` can theo doi MOI `EngineEvent`
// BAT KE no thuoc tab/run nao (danh sach Tac vu/Kich ban la theo DB, khong gan voi mot canvas cu
// the — kich ban con co duong chay RIENG qua `store/scripts.ts`, khong di qua RunBar/tab nao ca).
// `engine_bridge.ts` (dang ky kenh `engine://event` DUY NHAT cho ca vong doi app) goi `ingest` o
// day cho MOI su kien, SONG SONG voi viec dinh tuyen su kien toi dung store run cua tung tab.
import { create } from "zustand";
import type { EngineEvent } from "../ipc/types";

/** Gioi han nhat ky — giong `MAX_EVENTS` cua `store/run.ts` truoc day (xem doc o do). */
const MAX_EVENTS = 500;

interface EngineActivityState {
  events: EngineEvent[];
  ingest: (event: EngineEvent) => void;
}

export const useEngineActivity = create<EngineActivityState>((set, get) => ({
  events: [],
  ingest: (event) => {
    const next = [...get().events, event];
    set({ events: next.length > MAX_EVENTS ? next.slice(next.length - MAX_EVENTS) : next });
  },
}));
