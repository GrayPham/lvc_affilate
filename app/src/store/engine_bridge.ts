// Cau noi vong doi UNG DUNG giua kenh Tauri `engine://event` va `store/run.ts` — review
// fix-round 1 (C2): truoc day dang ky trong `Workflow.tsx`'s mount effect, nhung `App.tsx` chi
// render `<ActivePage/>` cho tab dang mo (`src/App.tsx`) — chuyen sang tab khac se UNMOUNT
// `Workflow.tsx` hoan toan, chay cleanup, huy dang ky, va MOI su kien phat trong luc do bi mat
// (ke ca `run_done` ket thuc mot lan chay, khien nut "Chạy" bi khoa vinh vien vi `running` khong
// bao gio tro lai `false` — xem review). `startEngineBridge()` goi MOT LAN duy nhat tu
// `main.tsx` (ngoai React, truoc render — idempotent qua co `started`), song suot vong doi tab
// trinh duyet/webview, khong bao gio unlisten.
import { listenEngine } from "../ipc/events";
import { errorText } from "../ipc/error_text";
import { useRunStore } from "./run";

let started = false;

/** Idempotent — goi lai (vd StrictMode/HMR) khong dang ky them lan hai. */
export function startEngineBridge(): void {
  if (started) {
    return;
  }
  started = true;
  listenEngine((event) => {
    useRunStore.getState().ingest(event);
  }).catch((e: unknown) => {
    // Ngoai cua so Tauri that (vd `pnpm dev` mo trong trinh duyet thuong de xem UI nhanh) —
    // khong co kenh Tauri, bo qua thay vi lam vo app.
    console.warn("khong the lang nghe engine://event:", errorText(e));
  });
}
