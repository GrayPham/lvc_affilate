// Ham THUAN (khong I/O) cho kho template (P4d) — ma loi TEMPLATE_* -> thong diep tieng Viet.
// Theo dung quy uoc `format/account.ts`/`format/credential.ts`:
// - `TEMPLATE_ERROR_VI` duoc EXPORT de test ghim (pin) day du 5 ma loi cua backend; mot ma MOI
//   (hoac go sai) se fail build thay vi am tham thieu nhan.
// - `describeTemplateError` luon kiem `isIpcError` TRUOC (invoke() reject bang IpcError THUAN,
//   khong phai Error JS that — xem ghi chu o `format/account.ts::describeAccountError`).
import { isIpcError } from "../ipc/commands";
import type { IpcError } from "../ipc/types";

// ---------------------------------------------------------------------------------------------
// Ma loi IPC -> thong diep tieng Viet (P4d contract). Exported de test pin duoc — 5 ma phai khop
// CHINH XAC cac ma backend tra ve cho mien template, khong hon khong kem.
// ---------------------------------------------------------------------------------------------

export const TEMPLATE_ERROR_VI: Record<string, string> = {
  TEMPLATE_NOT_FOUND: "Không tìm thấy mẫu.",
  TEMPLATE_CORRUPT: "Mẫu bị hỏng (không đọc được đồ thị).",
  TEMPLATE_READONLY: "Mẫu công khai chỉ đọc — không thể lưu đè hoặc xoá.",
  GRAPH_ENCODE: "Lỗi mã hoá đồ thị của mẫu.",
  STORE: "Lỗi lưu dữ liệu.",
  TEMPLATE_REMOTE_UNAVAILABLE: "Không tải được kho mẫu miễn phí — kiểm tra kết nối mạng rồi thử lại.",
};

/** Thong diep tieng Viet cho `IpcError` cua kho template — roi ve `err.message` neu ma la. */
export function templateErrorMessage(err: IpcError): string {
  return TEMPLATE_ERROR_VI[err.code] ?? err.message;
}

/** Thong diep hien thi cho MOT `catch (e: unknown)` bat ky — xem `describeAccountError`. */
export function describeTemplateError(e: unknown): string {
  if (isIpcError(e)) {
    return templateErrorMessage(e);
  }
  return e instanceof Error ? e.message : String(e);
}
