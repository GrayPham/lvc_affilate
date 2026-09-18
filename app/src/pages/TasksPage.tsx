// Man "Tác vụ" (Task 13): bang task cua bo loc hien tai (`TaskFilters` + `TaskTable`), tu cap nhat
// qua `task_list` khi mount/doi run VA khi co `EngineEvent` cham toi mot task (debounce 300ms —
// tranh doc DB lien tuc luc dang chay nhieu task song song).
//
// KHONG tu dang ky `listenEngine` rieng o day: `src/store/engine_bridge.ts` (goi tu `main.tsx`,
// review fix-round 1 C2 cua Task 12b) da la subscription DUY NHAT cho ca vong doi app, đo` moi
// `EngineEvent` vao `useRunStore`. Trang nay chi DOC `useRunStore().events` (mang su kien gan day,
// cap 500 — xem `store/run.ts`) de biet KHI NAO can re-fetch, tranh tao listener thu hai tren cung
// kenh `engine://event` (App.tsx chi render mot tab tai mot thoi diem — tu dang ky lai o day nhu
// Task 12 ban dau se mat su kien moi khi doi tab, dung loi C2 da sua).
import { useEffect, useRef } from "react";

import { useRunStore } from "../store/run";
import { useTasksStore } from "../store/tasks";
import TaskFilters from "../components/TaskFilters";
import TaskTable from "../components/TaskTable";
import type { EngineEvent } from "../ipc/types";

/** Debounce re-fetch sau khi co su kien cham toi task — nhieu su kien den lien tiep (nhieu luong
 * chay song song) chi doc DB MOT lan sau khi im lang 300ms, khop yeu cau brief Task 13. */
const REFRESH_DEBOUNCE_MS = 300;

/** Cac kieu `EngineEvent` THAT SU cham toi mot task (hoac mot run vua ket thuc, co the keo theo
 * task bi huy dong loat) — `node_state` la tong hop cap do node, khong mang thong tin task rieng
 * le nao moi nen khong dua vao day. */
const TASK_TOUCHING_TYPES: ReadonlySet<EngineEvent["type"]> = new Set([
  "task_stage",
  "task_done",
  "task_failed",
  "run_done",
]);

export default function TasksPage() {
  const refresh = useTasksStore((s) => s.refresh);
  const events = useRunStore((s) => s.events);
  const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Doc luc mount + don dep timer con treo luc unmount trang.
  useEffect(() => {
    void refresh();
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
    // `refresh` la action zustand (tham chieu on dinh) — chi can chay dung mot lan luc mount.
  }, []);

  // Moi lan `events` doi tham chieu (co su kien MOI vua duoc `engine_bridge.ts` ingest) va su
  // kien MOI NHAT thuoc nhom cham task -> dat lai debounce 300ms.
  useEffect(() => {
    const last = events[events.length - 1];
    if (!last || !TASK_TOUCHING_TYPES.has(last.type)) {
      return;
    }
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }
    timerRef.current = setTimeout(() => {
      void refresh();
    }, REFRESH_DEBOUNCE_MS);
  }, [events, refresh]);

  return (
    <div className="flex h-full flex-col">
      <TaskFilters />
      <TaskTable />
    </div>
  );
}
