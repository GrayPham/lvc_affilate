// Gom `TaskDto[]` cua MOT lan chay thanh dong thoi gian THEO NODE cho `RunLogDialog` — THUAN
// (khong I/O, khong React) nen test duoc truc tiep.
//
// Vi sao phai gom: backend ghi mot dong `tasks` cho MOI CANH (mot node "Tạo video" voi 3 cảnh =
// 3 task). Ban thiet ke lai ve mot dong cho moi NODE. Gom o day thay vi trong component de logic
// "node nay coi la thanh cong/that bai/chua chay" co the kiem bang test, khong phai bam thu.
import type { TaskDto } from "../ipc/types";

/** Ba trang thai cua mot node tren dong thoi gian — khop dung ban thiet ke. */
export type StageState = "ok" | "err" | "running" | "skip";

export interface TimelineRow {
  nodeId: string;
  /** So thu tu hien thi, bat dau tu 1. */
  index: number;
  state: StageState;
  /** So canh thuoc node nay. */
  taskCount: number;
  /** So canh da xong. */
  doneCount: number;
  /** Tai khoan da dung (khac nhau giua cac canh thi lay cai dau tien co gia tri). */
  account: string;
  /** Buoc hien tai — chi co nghia khi `state === "running"`. */
  stage: string;
  /** Giay tu luc canh dau bat dau den luc canh cuoi xong; `null` khi chua chay xong. */
  durationSec: number | null;
  /** Ma loi + thong diep cua canh THAT BAI dau tien; rong khi khong loi. */
  failureCode: string;
  error: string;
}

/**
 * Mot node coi la:
 * - `err` neu CO it nhat mot canh that bai (mot canh hong la ca node khong ra du ket qua);
 * - `running` neu con canh dang xu ly;
 * - `ok` neu MOI canh da hoan tat;
 * - `skip` neu moi canh con dang cho (chua tung bat dau).
 *
 * Thu tu uu tien nay co y: bao loi TRUOC bao dang chay, vi nguoi dung mo log la de tim cho hong.
 */
function stateOf(tasks: readonly TaskDto[]): StageState {
  if (tasks.some((t) => t.status === "failed")) {
    return "err";
  }
  if (tasks.some((t) => t.status === "processing")) {
    return "running";
  }
  if (tasks.every((t) => t.status === "completed")) {
    return "ok";
  }
  return "skip";
}

/** Giay tu mốc bat dau SOM NHAT den moc xong MUON NHAT; `null` khi thieu moc. */
function durationOf(tasks: readonly TaskDto[]): number | null {
  const starts = tasks.map((t) => t.startedAt).filter((v): v is number => v !== null && v > 0);
  const ends = tasks.map((t) => t.finishedAt).filter((v): v is number => v !== null && v > 0);
  if (starts.length === 0 || ends.length === 0) {
    return null;
  }
  const d = Math.max(...ends) - Math.min(...starts);
  // Dong ho he thong co the nhay lui -> khong bao gio tra so am.
  return d < 0 ? null : Math.round(d);
}

/**
 * Gom theo `nodeId`, giu THU TU XUAT HIEN dau tien trong `tasks` (backend tra theo thu tu tao, tuc
 * la theo thu tu engine chay) — khong sap lai theo ten/id, vi thu tu chay moi la thong tin nguoi
 * dung can khi doc log.
 */
export function buildTimeline(tasks: readonly TaskDto[]): TimelineRow[] {
  const groups = new Map<string, TaskDto[]>();
  for (const t of tasks) {
    const list = groups.get(t.nodeId);
    if (list) {
      list.push(t);
    } else {
      groups.set(t.nodeId, [t]);
    }
  }

  const out: TimelineRow[] = [];
  let index = 0;
  for (const [nodeId, group] of groups) {
    index += 1;
    const failed = group.find((t) => t.status === "failed");
    const running = group.find((t) => t.status === "processing");
    out.push({
      nodeId,
      index,
      state: stateOf(group),
      taskCount: group.length,
      doneCount: group.filter((t) => t.status === "completed").length,
      account: group.find((t) => t.account !== null && t.account !== "")?.account ?? "",
      stage: running?.stage ?? "",
      durationSec: durationOf(group),
      failureCode: failed?.failureCode ?? "",
      error: failed?.error ?? "",
    });
  }
  return out;
}

/** `3:21` / `0:04` / `1:02:03`. `null` -> chuoi rong (dung cho node chua chay). */
export function formatDuration(sec: number | null): string {
  if (sec === null) {
    return "";
  }
  const s = Math.max(0, Math.round(sec));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(ss)}` : `${m}:${pad(ss)}`;
}

/** `07:58:02` tu moc giay Unix; `0`/`null` -> `"--:--:--"`. */
export function formatClock(unixSeconds: number | null): string {
  if (unixSeconds === null || unixSeconds <= 0) {
    return "--:--:--";
  }
  const d = new Date(unixSeconds * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/**
 * Log dang text de nguoi dung sao chep di bao loi. Dung tu CHINH cac dong `tasks` da luu — khong
 * phai log server (app khong giu log van ban cho tung lan chay), nen moi dong o day deu la du lieu
 * that, khong co gi duoc bia them.
 */
export function buildLogText(tasks: readonly TaskDto[]): string {
  const lines: string[] = [];
  for (const t of tasks) {
    const when = formatClock(t.startedAt ?? t.createdAt);
    const scene = t.sceneIndex === null ? "" : ` canh=${t.sceneIndex + 1}`;
    const acc = t.account === null || t.account === "" ? "" : ` tk=${t.account}`;
    const head = `${when} node=${t.nodeId}${scene}${acc} ${t.status}`;
    if (t.status === "failed") {
      const code = t.failureCode === null || t.failureCode === "" ? "" : ` ${t.failureCode}`;
      const msg = t.error === null || t.error === "" ? "" : ` — ${t.error}`;
      lines.push(`${head}${code}${msg}`);
    } else {
      lines.push(`${head} buoc=${t.stage}`);
    }
  }
  return lines.join("\n");
}
