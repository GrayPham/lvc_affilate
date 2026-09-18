// Store zustand cho MOT run dang chay tren canvas (Task 12) — nghe `engine://event` qua
// `src/ipc/events.ts::listenEngine` (Workflow.tsx goi `ingest` trong handler). Tach rieng ham
// `applyEvent` THUAN (khong phu thuoc zustand) de test duoc doc lap (`tests/run_reducer.test.ts`)
// — day la logic dung nhat de kiem: thu tu su kien, dem done/ok/failed, trang thai ket thuc.
import { create } from "zustand";
import type { EngineEvent } from "../ipc/types";

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
  events: EngineEvent[];
}

export const EMPTY_RUN_STATE: RunState = {
  runId: null,
  status: null,
  tasks: {},
  nodes: {},
  events: [],
};

/**
 * Reducer thuan: `state` sau khi ap dung `event`. KHONG loc theo `runId` — Workflow.tsx chi
 * `ingest` khi da co run dang mo (goi `beginRun` truoc), va engine (Task 4-7) chi phat su kien
 * cua run/task thuoc phien hien tai qua kenh broadcast dung chung; loc them o day la thua so voi
 * nguon that (lvc-engine).
 */
export function applyEvent(state: RunState, event: EngineEvent): RunState {
  const events = [...state.events, event];

  switch (event.type) {
    case "task_stage": {
      const prev = state.tasks[event.taskId];
      const next: TaskRunState = {
        taskId: event.taskId,
        status: "running",
        stage: event.stage,
        detail: event.detail,
        elapsedMs: event.elapsedMs,
        account: prev?.account ?? null,
        failureCode: null,
        error: null,
      };
      return { ...state, events, tasks: { ...state.tasks, [event.taskId]: next } };
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
      return { ...state, events, tasks: { ...state.tasks, [event.taskId]: next } };
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
      return { ...state, events, tasks: { ...state.tasks, [event.taskId]: next } };
    }
    case "node_state": {
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
        events,
        runId: state.runId ?? event.runId,
        nodes: { ...state.nodes, [event.nodeId]: next },
      };
    }
    case "run_done": {
      return { ...state, events, runId: event.runId, status: event.status };
    }
  }
}

interface RunStore extends RunState {
  /**
   * Ghi nhan `runId` cua run vua `run_start` tra ve — KHONG ghi de neu da co `runId` (mot su kien
   * `node_state` "queued" phat trong luc `run_start` con dang await co the da toi va tu dien
   * `runId` truoc — xem `applyEvent` nhanh `node_state`). Goi SAU `clearRun()` + `runStart()`.
   */
  beginRun: (runId: string) => void;
  /** Nhan mot `EngineEvent` (goi trong handler `listenEngine`). */
  ingest: (event: EngineEvent) => void;
  /**
   * Ve trang thai rong — goi TRUOC khi goi `runStart` (khong phai sau) de khong co khoang trong
   * nao ma su kien cua run MOI bi ghi vao roi lai bi xoa boi lan reset sau.
   */
  clearRun: () => void;
}

export const useRunStore = create<RunStore>((set) => ({
  ...EMPTY_RUN_STATE,
  beginRun: (runId) => set((s) => ({ runId: s.runId ?? runId })),
  ingest: (event) => set((state) => applyEvent(state, event)),
  clearRun: () => set({ ...EMPTY_RUN_STATE }),
}));
