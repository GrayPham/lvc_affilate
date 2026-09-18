// Cau su kien engine (crates/lvc-app/src/events.rs, Task 8): moi `EngineEvent` duoc bridge phat
// qua kenh Tauri `engine://event`. `listenEngine` boc `listen` (qua shim `./tauri`) + parse zod —
// payload sai hop dong nem loi NGAY (fail-fast) thay vi UI nhan du lieu khong dung kieu.
import { listen, type UnlistenFn } from "./tauri";
import { EngineEventSchema, type EngineEvent } from "./types";

export const ENGINE_EVENT_NAME = "engine://event";

export async function listenEngine(
  handler: (event: EngineEvent) => void,
): Promise<UnlistenFn> {
  return listen<unknown>(ENGINE_EVENT_NAME, (raw) => {
    handler(EngineEventSchema.parse(raw.payload));
  });
}
