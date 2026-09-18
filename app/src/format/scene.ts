// Ham THUAN (khong I/O) cho tab "Kịch bản" (P3c Task 2): nhan trang thai mot canh (tai su dung
// `statusLabel`/`failureLabel` cua `format/task.ts` — CUNG mot bang tra cuu `TaskDto.status`/
// `FailureCode`, chi khac nguon la `SceneDto` thay vi `TaskDto`), dieu kien hien nut Chay/Chay
// lai/Dung (mirror `SceneTable`/`SceneRow` can), nhan ma loi rieng cua kich ban
// (`ScriptRunReport.errors[].code`/IpcError cua cac lenh `script_*`), va ham chon canh muc tieu
// cho "Đặt lại trạng thái" (uu tien danh sach da chon, roi lai ve moi canh dang `failed`).
import type { ScriptDoc, SceneDto } from "../ipc/types";
import { failureLabel, statusLabel } from "./task";

/**
 * Nhan hien thi cho MOT canh: "Chưa chạy" khi `taskStatus` con `null` (chua tung chay HOAC
 * `taskId` da mo coi — xem doc `SceneDto`), nguoc lai dung `statusLabel` (Task 13) + gop them
 * `failureLabel` khi la `failed` VA co `failureCode` (giong cach `TaskTable` hien mã lỗi duoi
 * badge trang thai).
 */
export function sceneStatusLabel(scene: Pick<SceneDto, "taskStatus" | "failureCode">): string {
  if (scene.taskStatus === null) {
    return "Chưa chạy";
  }
  const base = statusLabel(scene.taskStatus);
  if (scene.taskStatus === "failed" && scene.failureCode) {
    return `${base} (${failureLabel(scene.failureCode)})`;
  }
  return base;
}

/** `true` khi canh co the bam "Chạy" — chua tung chay (`null`) hoac lan truoc that bai. */
export function canRun(scene: Pick<SceneDto, "taskStatus">): boolean {
  return scene.taskStatus === null || scene.taskStatus === "failed";
}

/** `true` khi canh co the bam "Chạy lại" (da xong — nut nay can xac nhan vi TON CREDIT, xem
 * `SceneRow`). */
export function canRerun(scene: Pick<SceneDto, "taskStatus">): boolean {
  return scene.taskStatus === "completed";
}

/** `true` khi canh co the bam "Dừng" (`task_cancel`) — chi con y nghia khi chua ket thuc, giong
 * `format/task.ts::isCancellable`. */
export function canStop(scene: Pick<SceneDto, "taskStatus">): boolean {
  return scene.taskStatus === "queued" || scene.taskStatus === "processing";
}

/**
 * Nhan tieng Viet cho ma loi RIENG cua mien kich ban — hai nguon: `ScriptRunReport.errors[].code`
 * (tung canh khong duoc chay trong mot lan `script_run`) VA `IpcError.code` cua cac lenh
 * `script_*` khac (`script_delete`/`script_put_scenes`/`script_import_txt`...). Ma khong nam
 * trong bang -> tra nguyen ma tho (server la nguon su that, cung quy uoc voi
 * `format/task.ts::failureLabel`).
 */
const SCRIPT_ERROR_VI: Record<string, string> = {
  SCENE_RUNNING: "Cảnh đang chạy",
  SCENE_EMPTY: "Prompt trống",
  SCENE_INVALID: "Cảnh không hợp lệ",
  TOO_MANY_SCENES: "Quá nhiều cảnh",
  SCRIPT_NOT_FOUND: "Không tìm thấy kịch bản",
};

export function scriptErrorLabel(code: string): string {
  return SCRIPT_ERROR_VI[code] ?? code;
}

/**
 * Danh sach id canh muc tieu cho "Đặt lại trạng thái": uu tien cac id NGUOI DUNG da chon
 * (`selectedIds`, loc lai theo cac canh THAT SU con trong `doc` — tranh gui id "ma" cua mot ban
 * chup cu); khi khong chon gi ca (`selectedIds` rong) -> roi ve MOI canh dang `failed` cua kich
 * ban (dung y "for selected/failed" — nguoi dung khong can tu chon tung canh loi de dat lai).
 * Thu tu ket qua theo dung thu tu `doc.scenes` (da sap `idx` boi server).
 */
export function sceneIdsToRun(
  doc: Pick<ScriptDoc, "scenes">,
  selectedIds: ReadonlySet<string>,
): string[] {
  if (selectedIds.size > 0) {
    return doc.scenes.filter((s) => selectedIds.has(s.id)).map((s) => s.id);
  }
  return doc.scenes.filter((s) => s.taskStatus === "failed").map((s) => s.id);
}
