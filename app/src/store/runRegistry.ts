// So dang ky "runId -> store cua tab nao dang theo doi run do" (P-tabs) — dung boi
// `engine_bridge.ts` de dinh tuyen `EngineEvent` toi DUNG store run cua tung tab thay vi mot store
// toan cuc duy nhat nhu truoc. `RunBar.tsx` (handleRun) goi `registerRunOwner` ngay sau khi
// `run_start` tra ve `runId`; `unregisterRunOwner` goi khi tab do dong hoac bat dau mot run KHAC
// (RAII don gian qua `useEffect` cleanup, xem `components/RunBar.tsx`).
//
// Hai nhom su kien dinh tuyen KHAC NHAU (xem `engine_bridge.ts`):
// - `node_state`/`run_done` MANG `runId` -> tra thang qua `owners`; neu CHUA co chu (dua tren
//   `run_start` con dang await, su kien "queued" dau tien co the toi TRUOC khi `registerRunOwner`
//   kip goi) -> DEM lai trong `pending`, `registerRunOwner` tu "xa" (flush) ngay khi dang ky.
// - `task_stage`/`task_done`/`task_failed` KHONG mang `runId` (chi `taskId`, xem doc
//   `crates/lvc-engine/src/events.rs`) — KHONG the dinh tuyen chinh xac tuyet doi o day ma khong
//   doi schema Rust (da khao sat, xem doc lich su trong tin nhan cho nguoi dung); `activeStores()`
//   tra danh sach store DANG theo doi mot run con hoat dong (`runId !== null`) de
//   `engine_bridge.ts` phat (broadcast) rieng nhom nay toi TAT CA — truong hop HIEM hai tab cung
//   chay dong thoi co the lan task lac sang tab khac trong chop nhoang, nhung tu sua ngay o lan
//   `reconcile` ke tiep (`RunBar`/`Workflow.tsx` doc lai tu DB qua `task_list`, loc dung `runId`,
//   xem `store/run.tsx::reconcileTasksFromDb`) — danh doi CHAP NHAN DUOC de tranh phai doi hop
//   dong `EngineEvent` (rui ro cao hon, dung toi duong chay task nhay cam).
import type { EngineEvent } from "../ipc/types";
import type { RunStoreApi } from "./run";

type RoutableByRunId = Extract<EngineEvent, { runId: string }>;

const owners = new Map<string, RunStoreApi>();
const pending = new Map<string, RoutableByRunId[]>();

/** Dang ky `store` la chu cua `runId` — xa (flush) ngay moi su kien da DEM lai truoc do cho dung
 * `runId` nay (xem doc dau file). */
export function registerRunOwner(runId: string, store: RunStoreApi): void {
  owners.set(runId, store);
  const buffered = pending.get(runId);
  if (buffered) {
    pending.delete(runId);
    for (const ev of buffered) {
      store.getState().ingest(ev);
    }
  }
}

export function unregisterRunOwner(runId: string): void {
  owners.delete(runId);
  pending.delete(runId);
}

/** `node_state`/`run_done` — tra thang qua so dang ky; chua co chu thi dem lai cho `registerRunOwner`
 * flush (xem doc dau file). */
export function routeByRunId(event: RoutableByRunId): void {
  const store = owners.get(event.runId);
  if (store) {
    store.getState().ingest(event);
    return;
  }
  const buffered = pending.get(event.runId) ?? [];
  buffered.push(event);
  pending.set(event.runId, buffered);
}

/** Danh sach KHONG TRUNG store dang theo doi mot run con hoat dong (`runId !== null`) — dung de
 * phat (broadcast) `task_stage`/`task_done`/`task_failed` (xem doc dau file). */
export function activeStores(): RunStoreApi[] {
  const seen = new Set<RunStoreApi>();
  for (const store of owners.values()) {
    if (store.getState().runId !== null) {
      seen.add(store);
    }
  }
  return [...seen];
}
