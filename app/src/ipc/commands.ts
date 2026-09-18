// Mot ham boc typed cho MOI lenh Tauri cua `crates/lvc-app` (16 lenh, xem
// `commands::mod.rs`/`lib.rs::invoke_handler!`) — ten lenh dung HET SUC dung chuoi Rust
// (snake_case), tham so JS dung camelCase (Tauri v2 tu doi snake_case -> camelCase cho tham so
// lenh, xac nhan trong task-9-report.md). Ket qua luon di qua `.parse()` zod (khong `safeParse`)
// — lech hop dong phai nem loi ro rang NGAY tai diem goi, khong am tham lot qua kieu `any`.
import { invoke } from "./tauri";
import {
  AppSettingsSchema,
  PlanSummarySchema,
  RunInfoSchema,
  RunRowSchema,
  TaskDtoSchema,
  TaskResumeInfoSchema,
  WorkflowDocSchema,
  WorkflowMetaSchema,
  IpcErrorSchema,
  type AppSettings,
  type Graph,
  type IpcError,
  type PlanSummary,
  type RunInfo,
  type RunRow,
  type TaskDto,
  type TaskResumeInfo,
  type WorkflowDoc,
  type WorkflowMeta,
} from "./types";
import { z } from "zod";

/** Type guard cho loi tra ve tu bat ky lenh IPC nao (`invoke()` reject voi gia tri nay). */
export function isIpcError(e: unknown): e is IpcError {
  return IpcErrorSchema.safeParse(e).success;
}

// ---- workflow.rs -------------------------------------------------------------------------

export async function workflowValidate(graph: Graph): Promise<PlanSummary> {
  const res = await invoke<unknown>("workflow_validate", { graph });
  return PlanSummarySchema.parse(res);
}

export async function workflowSave(
  id: string | null,
  name: string,
  graph: Graph,
): Promise<WorkflowMeta> {
  const res = await invoke<unknown>("workflow_save", { id, name, graph });
  return WorkflowMetaSchema.parse(res);
}

export async function workflowList(): Promise<WorkflowMeta[]> {
  const res = await invoke<unknown>("workflow_list");
  return z.array(WorkflowMetaSchema).parse(res);
}

export async function workflowGet(id: string): Promise<WorkflowDoc> {
  const res = await invoke<unknown>("workflow_get", { id });
  return WorkflowDocSchema.parse(res);
}

export async function workflowDelete(id: string): Promise<boolean> {
  const res = await invoke<unknown>("workflow_delete", { id });
  return z.boolean().parse(res);
}

export async function runStart(workflowId: string, graph: Graph): Promise<RunInfo> {
  const res = await invoke<unknown>("run_start", { workflowId, graph });
  return RunInfoSchema.parse(res);
}

export async function runCancel(runId: string): Promise<boolean> {
  const res = await invoke<unknown>("run_cancel", { runId });
  return z.boolean().parse(res);
}

export async function runList(limit: number): Promise<RunRow[]> {
  const res = await invoke<unknown>("run_list", { limit });
  return z.array(RunRowSchema).parse(res);
}

// ---- tasks.rs -----------------------------------------------------------------------------

export async function taskList(runId: string | null, limit: number): Promise<TaskDto[]> {
  const res = await invoke<unknown>("task_list", { runId, limit });
  return z.array(TaskDtoSchema).parse(res);
}

export async function taskCancel(id: string): Promise<boolean> {
  const res = await invoke<unknown>("task_cancel", { id });
  return z.boolean().parse(res);
}

export async function taskResume(id: string): Promise<TaskResumeInfo> {
  const res = await invoke<unknown>("task_resume", { id });
  return TaskResumeInfoSchema.parse(res);
}

export async function taskOpenFile(id: string): Promise<null> {
  const res = await invoke<unknown>("task_open_file", { id });
  return z.null().parse(res);
}

// ---- settings.rs --------------------------------------------------------------------------

export async function settingsGet(): Promise<AppSettings> {
  const res = await invoke<unknown>("settings_get");
  return AppSettingsSchema.parse(res);
}

/** Tra ve danh sach canh bao chuan hoa (rong = khong co gi phai sua). */
export async function settingsSet(s: AppSettings): Promise<string[]> {
  const res = await invoke<unknown>("settings_set", { s });
  return z.array(z.string()).parse(res);
}

/** `null` = nguoi dung bam Huy trong hop thoai. */
export async function pickDirectory(): Promise<string | null> {
  const res = await invoke<unknown>("pick_directory");
  return z.string().nullable().parse(res);
}

/** `images = true` loc dinh dang anh (png/jpg/jpeg/webp); huy hop thoai -> mang rong. */
export async function pickFiles(images: boolean): Promise<string[]> {
  const res = await invoke<unknown>("pick_files", { images });
  return z.array(z.string()).parse(res);
}
