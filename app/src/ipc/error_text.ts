// Chuan hoa loi ve MOT dong de hien thi cho nguoi dung — review fix-round 1 (I1): Tauri
// `invoke()` (`@tauri-apps/api/core`) reject bang gia tri Rust `Err` da giai ma THANG, mot object
// thuong `{code,message,details}` (`IpcError`) — KHONG phai instance `Error` cua JS. Moi noi tung
// dung `e instanceof Error ? e.message : String(e)` deu roi ve chuoi "[object Object]" cho dung
// truong hop hay gap nhat (lenh IPC tra `IpcError`, vd `run_start` that bai) — ham nay la diem
// DUY NHAT xu ly ca ba dang loi co the gap trong app: `IpcError`, `Error` (JS thuan), va bat ky
// gia tri nao khac.
import { isIpcError } from "./commands";

/**
 * `IpcError` -> `"<code>: <message>"` (them so luong loi neu la `GRAPH_INVALID`, `details` la
 * `GraphError[]`); `Error` -> `.message`; con lai -> `String(e)`.
 */
export function errorText(e: unknown): string {
  if (isIpcError(e)) {
    const detailsCount =
      e.code === "GRAPH_INVALID" && Array.isArray(e.details) ? ` (${e.details.length} lỗi)` : "";
    return `${e.code}: ${e.message}${detailsCount}`;
  }
  if (e instanceof Error) {
    return e.message;
  }
  return String(e);
}
