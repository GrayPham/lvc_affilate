// Cau noi vong doi UNG DUNG giua kenh Tauri `engine://event` va cac store run CUA TUNG TAB
// (P-tabs: truoc day chi co MOT `store/run.ts` toan cuc, gio moi tab workflow co MOT instance
// rieng — xem `store/run.tsx`/`store/runRegistry.ts`) — review fix-round 1 (C2): truoc day dang
// ky trong `Workflow.tsx`'s mount effect, nhung `App.tsx` chi render `<ActivePage/>` cho tab dang
// mo (`src/App.tsx`) — chuyen sang tab khac se UNMOUNT `Workflow.tsx` hoan toan, chay cleanup, huy
// dang ky, va MOI su kien phat trong luc do bi mat. `startEngineBridge()` goi MOT LAN duy nhat tu
// `main.tsx` (ngoai React, truoc render — idempotent qua co `started`), song suot vong doi tab
// trinh duyet/webview, khong bao gio unlisten.
//
// MOI su kien di qua HAI duong SONG SONG:
// 1. `useEngineActivity().ingest(event)` — nhat ky TOAN CUC (khong gan voi tab/run nao), dung boi
//    `TasksPage`/`ScriptsPage` (danh sach theo DB, khong thuoc mot canvas cu the — xem doc
//    `store/engineActivity.ts`).
// 2. Dinh tuyen toi DUNG store run cua tab so huu run do (`store/runRegistry.ts`) — `node_state`/
//    `run_done` theo `runId` chinh xac; `task_stage`/`task_done`/`task_failed` (khong mang `runId`)
//    phat toi MOI tab dang theo doi mot run con hoat dong (xem doc danh doi trong `runRegistry.ts`).
import { listenEngine } from "../ipc/events";
import { errorText } from "../ipc/error_text";
import { useEngineActivity } from "./engineActivity";
import { activeStores, routeByRunId } from "./runRegistry";
import type { EngineEvent } from "../ipc/types";

let started = false;

function dispatchToTabs(event: EngineEvent): void {
  switch (event.type) {
    case "node_state":
    case "run_done":
    case "node_outputs":
      routeByRunId(event);
      return;
    case "task_stage":
    case "task_done":
    case "task_failed":
      for (const store of activeStores()) {
        store.getState().ingest(event);
      }
      return;
  }
}

/** Idempotent — goi lai (vd StrictMode/HMR) khong dang ky them lan hai. */
export function startEngineBridge(): void {
  if (started) {
    return;
  }
  started = true;
  listenEngine((event) => {
    useEngineActivity.getState().ingest(event);
    dispatchToTabs(event);
  }).catch((e: unknown) => {
    // Ngoai cua so Tauri that (vd `pnpm dev` mo trong trinh duyet thuong de xem UI nhanh) —
    // khong co kenh Tauri, bo qua thay vi lam vo app.
    console.warn("khong the lang nghe engine://event:", errorText(e));
  });
}
