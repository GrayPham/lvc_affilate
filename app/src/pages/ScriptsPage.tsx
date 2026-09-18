// Man "Kịch bản" (P3c Task 2): `ScriptList` (trai) + `SceneTable` (phai). Tu cap nhat `doc` (trang
// thai tung canh) qua `useRunStore().events` khi mot su kien `task_stage/task_done/task_failed`
// CHAM toi mot `taskId` thuoc kich ban DANG MO (debounce 300ms) — cung mot mo hinh voi
// `pages/TasksPage.tsx` (KHONG tu `listenEngine` rieng o day: `src/store/engine_bridge.ts`, goi tu
// `main.tsx`, la subscription DUY NHAT cho kenh `engine://event` suot vong doi app; trang nay chi
// DOC `useRunStore().events`, xem doc file do).
import { useEffect, useRef } from "react";

import { useRunStore } from "../store/run";
import { useScriptsStore } from "../store/scripts";
import ScriptList from "../components/ScriptList";
import SceneTable from "../components/SceneTable";
import type { EngineEvent } from "../ipc/types";

const REFRESH_DEBOUNCE_MS = 300;

/** `taskId` cua `event` neu no la MOT trong 3 kieu su kien cham toi mot task cu the
 * (`node_state`/`run_done` khong mang `taskId` rieng — xem `store/run.ts`); `null` neu khong. */
function taskIdOf(event: EngineEvent): string | null {
  if (event.type === "task_stage" || event.type === "task_done" || event.type === "task_failed") {
    return event.taskId;
  }
  return null;
}

export default function ScriptsPage() {
  const refreshList = useScriptsStore((s) => s.refreshList);
  const refreshDoc = useScriptsStore((s) => s.refreshDoc);
  const doc = useScriptsStore((s) => s.doc);
  const events = useRunStore((s) => s.events);
  const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    void refreshList();
    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
    // `refreshList` la action zustand (tham chieu on dinh) — chi can chay dung mot lan luc mount.
  }, []);

  // Moi lan `events` doi tham chieu VA su kien MOI NHAT la mot su kien cham task CUA MOT taskId
  // thuoc kich ban dang mo (`doc.scenes[].taskId`) -> dat lai debounce 300ms roi doc lai `doc` +
  // lam moi so dem danh sach kich ban (yeu cau brief Task 2).
  useEffect(() => {
    const last = events[events.length - 1];
    if (!last || !doc) {
      return;
    }
    const taskId = taskIdOf(last);
    if (taskId === null) {
      return;
    }
    const belongsToDoc = doc.scenes.some((s) => s.taskId === taskId);
    if (!belongsToDoc) {
      return;
    }
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }
    timerRef.current = setTimeout(() => {
      void refreshDoc();
      void refreshList();
    }, REFRESH_DEBOUNCE_MS);
  }, [events, doc, refreshDoc, refreshList]);

  return (
    <div className="flex h-full">
      <ScriptList />
      <SceneTable />
    </div>
  );
}
