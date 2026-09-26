// Store zustand cho MOT run dang chay (Task 12) — nghe `engine://event` qua
// `src/store/engine_bridge.ts` (dang ky MOT LAN cho ca vong doi app, xem doc file do — KHONG con
// dang ky tu Workflow.tsx, review fix-round 1 C2: tab nao cung unmount Workflow.tsx khi doi tab,
// dang ky trong do se mat su kien, ke ca `run_done` ket thuc mot lan chay). Tach rieng ham
// `applyEvent` THUAN (khong phu thuoc zustand) de test duoc doc lap (`tests/run_reducer.test.ts`)
// — day la logic dung nhat de kiem: thu tu su kien, dem done/ok/failed, trang thai ket thuc.
import { create } from "zustand";
import type { EngineEvent, RunRow, TaskDto } from "../ipc/types";

/** Trang thai mot task (canh) suy ra tu chuoi `task_stage`/`task_done`/`task_failed`. */
export interface TaskRunState {
  taskId: string;
  status: "running" | "done" | "failed";
  stage: string | null;
  detail: string | null;
  elapsedMs: number;
  account: string | null;
  failureCode: string | null;
  error: string | null;
}

/** Trang thai mot node tren canvas, suy tu su kien `node_state` gan nhat cua node do. */
export interface NodeRunState {
  nodeId: string;
  /** Chuoi tho tu Rust: `queued`/`running`/`done`/`failed`/`cancelled` (dag.rs NODE_*). */
  state: string;
  done: number;
  total: number;
  ok: number;
  failed: number;
}

export interface RunState {
  runId: string | null;
  /** Chuoi tho tu `RunDone.status` (`completed`/`failed`/`cancelled`); `null` = chua ket thuc. */
  status: string | null;
  tasks: Record<string, TaskRunState>;
  nodes: Record<string, NodeRunState>;
  /** P-lumina: duong dan video HOAN TAT theo node (nodeId -> local_path) de node video preview. */
  nodeVideos: Record<string, string>;
  events: EngineEvent[];
}

export const EMPTY_RUN_STATE: RunState = {
  runId: null,
  status: null,
  tasks: {},
  nodes: {},
  nodeVideos: {},
  events: [],
};

/**
 * Gioi han `RunState.events` (review fix-round 1 M1: nhat ky chi phuc vu test/debug, khong doc
 * boi UI nao trong diff nay — nhung van cap thay vi bo han de con dung duoc neu sau nay can) —
 * giu 500 su kien GAN NHAT, bo cu nhat.
 */
const MAX_EVENTS = 500;

function appendEvent(events: readonly EngineEvent[], event: EngineEvent): EngineEvent[] {
  const next = [...events, event];
  return next.length > MAX_EVENTS ? next.slice(next.length - MAX_EVENTS) : next;
}

/**
 * Reducer thuan: `state` sau khi ap dung `event`.
 *
 * Fix-round 1 (review C1): `node_state`/`run_done` mang truong `runId` — neu store DANG theo doi
 * mot run khac (`state.runId` khac null VA khac `event.runId`), su kien nay thuoc mot run KHAC
 * (vd mot run mo coi tu "Mới"/"Mở" cu, van con phat tren CUNG kenh broadcast dung chung — xem
 * `crates/lvc-engine/src/events.rs`) — BO QUA hoan toan (tra nguyen `state`, khong hijack
 * `runId`/`status`). Khi `state.runId` con `null` (truoc khi `beginRun` kip goi, vd su kien
 * "queued" dau tien phat trong luc `run_start` con dang await), van cho nhan (adopt) nhu cu — do
 * la truong hop hop le, khong phai run la.
 *
 * `task_stage`/`task_done`/`task_failed` KHONG mang `runId` (chi `taskId`) nen khong loc duoc
 * theo run o day; ranh gioi run cho nhom su kien nay dua vao UI khong con giu `runId` cua run cu
 * (RunBar fix-round 1: "Mới"/"Mở" khi dang co run active se hoi truoc, KHONG tu clearRun ngam).
 */
export function applyEvent(state: RunState, event: EngineEvent): RunState {
  switch (event.type) {
    case "task_stage": {
      const prev = state.tasks[event.taskId];
      // Fix-round 1 (review I2): buoc "launching" mang ten tai khoan trong `detail`
      // (crates/lvc-engine/src/events.rs, generate.rs ~1655) — day la nguon DUY NHAT co account
      // TRONG LUC dang chay (task_done moi co field `account` rieng, nhung chi toi khi xong).
      const account =
        event.stage === "launching" && event.detail ? event.detail : (prev?.account ?? null);
      const next: TaskRunState = {
        taskId: event.taskId,
        status: "running",
        stage: event.stage,
        detail: event.detail,
        elapsedMs: event.elapsedMs,
        account,
        failureCode: null,
        error: null,
      };
      return {
        ...state,
        events: appendEvent(state.events, event),
        tasks: { ...state.tasks, [event.taskId]: next },
      };
    }
    case "task_done": {
      const prev = state.tasks[event.taskId];
      const next: TaskRunState = {
        taskId: event.taskId,
        status: "done",
        stage: prev?.stage ?? "done",
        detail: prev?.detail ?? null,
        elapsedMs: prev?.elapsedMs ?? 0,
        account: event.account,
        failureCode: null,
        error: null,
      };
      return {
        ...state,
        events: appendEvent(state.events, event),
        tasks: { ...state.tasks, [event.taskId]: next },
      };
    }
    case "task_failed": {
      const prev = state.tasks[event.taskId];
      const next: TaskRunState = {
        taskId: event.taskId,
        status: "failed",
        stage: prev?.stage ?? null,
        detail: prev?.detail ?? null,
        elapsedMs: prev?.elapsedMs ?? 0,
        account: prev?.account ?? null,
        failureCode: event.code,
        error: event.message,
      };
      return {
        ...state,
        events: appendEvent(state.events, event),
        tasks: { ...state.tasks, [event.taskId]: next },
      };
    }
    case "node_state": {
      if (state.runId !== null && state.runId !== event.runId) {
        return state;
      }
      const next: NodeRunState = {
        nodeId: event.nodeId,
        state: event.state,
        done: event.done,
        total: event.total,
        ok: event.ok,
        failed: event.failed,
      };
      return {
        ...state,
        events: appendEvent(state.events, event),
        runId: state.runId ?? event.runId,
        nodes: { ...state.nodes, [event.nodeId]: next },
      };
    }
    case "run_done": {
      if (state.runId !== null && state.runId !== event.runId) {
        return state;
      }
      return { ...state, events: appendEvent(state.events, event), runId: event.runId, status: event.status };
    }
  }
}

/**
 * Dong bo `status` cuc bo tu mot `RunRow` (bang `runs`, doc qua `run_list`) — dung khi nghi ngo
 * da bo lo `run_done` (review fix-round 1 C2: tab Workflow bi unmount dung luc engine phat su
 * kien). Chi ap dung khi `row.id` KHOP `state.runId` HIEN TAI (khong duoc doi run dang theo doi)
 * va `row.status` da la trang thai KET THUC (khac "running") — neu khong thi tra nguyen `state`.
 */
export function reconcileFromRunRow(state: RunState, row: RunRow): RunState {
  if (state.runId !== row.id) {
    return state;
  }
  if (row.status === "running") {
    return state;
  }
  if (state.status === row.status) {
    return state;
  }
  return { ...state, status: row.status };
}

/**
 * Dong bo tung dong `tasks` (`TaskDto`, doc qua `task_list`) vao `state.tasks` — bo sung/nang cap
 * nhung task CHUA co trang thai KET THUC cuc bo (khong ghi de mot su kien song "done"/"failed" da
 * toi truoc do bang mot ban chup DB co the cu hon do do tre round-trip). Chi xet dto co
 * `runId === state.runId` (an toan them, phong khi gian tiep truyen nham dto cua run khac).
 */
export function reconcileTasksFromDb(state: RunState, dtos: readonly TaskDto[]): RunState {
  let tasks = state.tasks;
  let nodeVideos = state.nodeVideos;
  for (const dto of dtos) {
    if (dto.runId !== state.runId) {
      continue;
    }
    // Video hoan tat -> nho path theo node de node video preview (dto co ca nodeId lan localPath).
    if (dto.status === "completed" && dto.localPath && dto.nodeId) {
      if (nodeVideos === state.nodeVideos) {
        nodeVideos = { ...state.nodeVideos };
      }
      nodeVideos[dto.nodeId] = dto.localPath;
    }
    const prev = tasks[dto.id];
    if (prev && (prev.status === "done" || prev.status === "failed")) {
      continue;
    }
    const status: TaskRunState["status"] =
      dto.status === "completed" ? "done" : dto.status === "failed" ? "failed" : "running";
    const next: TaskRunState = {
      taskId: dto.id,
      status,
      stage: dto.stage,
      detail: prev?.detail ?? null,
      elapsedMs: prev?.elapsedMs ?? 0,
      account: dto.account ?? prev?.account ?? null,
      failureCode: dto.failureCode,
      error: dto.error,
    };
    if (tasks === state.tasks) {
      tasks = { ...state.tasks };
    }
    tasks[dto.id] = next;
  }
  if (tasks === state.tasks && nodeVideos === state.nodeVideos) {
    return state;
  }
  return { ...state, tasks, nodeVideos };
}

interface RunStore extends RunState {
  /**
   * Ghi nhan `runId` cua run vua `run_start` tra ve — KHONG ghi de neu da co `runId` (mot su kien
   * `node_state` "queued" phat trong luc `run_start` con dang await co the da toi va tu dien
   * `runId` truoc — xem `applyEvent` nhanh `node_state`). Goi SAU `clearRun()` + `runStart()`.
   */
  beginRun: (runId: string) => void;
  /** Nhan mot `EngineEvent` (goi trong `engine_bridge.ts`). */
  ingest: (event: EngineEvent) => void;
  /**
   * Ve trang thai rong — CHI goi khi KHONG con run nao dang active can theo doi (xem RunBar
   * fix-round 1: "Mới"/"Mở" trong luc co run active se hoi Huỷ truoc, KHONG tu clearRun ngam de
   * run mo coi van con "cancellable" — review C1).
   */
  clearRun: () => void;
  /** Dong bo lai tu DB (`run_list`/`task_list`) — xem `reconcileFromRunRow`/`reconcileTasksFromDb`. */
  reconcile: (row: RunRow | null, tasks: readonly TaskDto[]) => void;
}

export const useRunStore = create<RunStore>((set) => ({
  ...EMPTY_RUN_STATE,
  beginRun: (runId) => set((s) => ({ runId: s.runId ?? runId })),
  ingest: (event) => set((state) => applyEvent(state, event)),
  clearRun: () => set({ ...EMPTY_RUN_STATE }),
  reconcile: (row, tasks) =>
    set((s) => {
      const afterRow = row ? reconcileFromRunRow(s, row) : s;
      return reconcileTasksFromDb(afterRow, tasks);
    }),
}));
