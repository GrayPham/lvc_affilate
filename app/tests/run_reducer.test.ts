// TDD Task 12 buoc 1: `applyEvent(state, EngineEvent)` (src/store/run.ts) la reducer THUAN xu
// ly chuoi su kien `engine://event` (crates/lvc-engine/src/events.rs) — moi bien the phai cap
// nhat dung truong, giu thu tu (`events` la log cong don), va `run_done` la trang thai ket thuc
// (khong con la `null`, khong bi su kien sau do "reset" nguoc lai).
import { describe, expect, it } from "vitest";

import {
  applyEvent,
  EMPTY_RUN_STATE,
  reconcileFromRunRow,
  reconcileTasksFromDb,
  type RunState,
} from "../src/store/run";
import type { EngineEvent, RunRow, TaskDto } from "../src/ipc/types";

function stage(taskId: string, stg: string, elapsedMs: number, detail: string | null = null): EngineEvent {
  return { type: "task_stage", taskId, stage: stg, detail, elapsedMs };
}

describe("applyEvent — reducer thuan cho EngineEvent (store/run.ts)", () => {
  it("task_stage: tao/cap nhat task voi status running", () => {
    const s1 = applyEvent(EMPTY_RUN_STATE, stage("t1", "page", 100));
    expect(s1.tasks["t1"]).toEqual({
      taskId: "t1",
      status: "running",
      stage: "page",
      detail: null,
      elapsedMs: 100,
      account: null,
      failureCode: null,
      error: null,
    });

    const s2 = applyEvent(s1, stage("t1", "generating", 4200, "acc1"));
    expect(s2.tasks["t1"]?.stage).toBe("generating");
    expect(s2.tasks["t1"]?.elapsedMs).toBe(4200);
    expect(s2.tasks["t1"]?.detail).toBe("acc1");
  });

  it("review fix-round 1 I2: task_stage 'launching' voi detail ghi ngay vao account (generate.rs~1655)", () => {
    const s1 = applyEvent(EMPTY_RUN_STATE, stage("t1", "launching", 0, "acc1"));
    expect(s1.tasks["t1"]?.account).toBe("acc1");
    // Buoc sau (khong phai "launching") KHONG duoc ghi de account da co bang detail cua no.
    const s2 = applyEvent(s1, stage("t1", "page", 50, "khong-phai-account"));
    expect(s2.tasks["t1"]?.account).toBe("acc1");
  });

  it("task_stage 'launching' ma detail null thi khong dong account (giu gia tri cu, van null neu chua co)", () => {
    const s1 = applyEvent(EMPTY_RUN_STATE, stage("t1", "launching", 0, null));
    expect(s1.tasks["t1"]?.account).toBeNull();
  });

  it("task_done: dong task, ghi account, xoa loi cu", () => {
    const s1 = applyEvent(EMPTY_RUN_STATE, stage("t1", "downloading", 9000));
    const s2 = applyEvent(s1, {
      type: "task_done",
      taskId: "t1",
      account: "acc1",
      conversationId: "conv1",
      localPath: "C:/out/a.mp4",
      bytes: 12345,
      watermarked: false,
    });
    expect(s2.tasks["t1"]).toEqual({
      taskId: "t1",
      status: "done",
      stage: "downloading",
      detail: null,
      elapsedMs: 9000,
      account: "acc1",
      failureCode: null,
      error: null,
    });
  });

  it("task_failed: ghi ma loi + thong diep, giu stage/elapsed cu", () => {
    const s1 = applyEvent(EMPTY_RUN_STATE, stage("t1", "sending", 500));
    const s2 = applyEvent(s1, {
      type: "task_failed",
      taskId: "t1",
      code: "timeout",
      message: "TIMEOUT: qua han",
    });
    expect(s2.tasks["t1"]).toEqual({
      taskId: "t1",
      status: "failed",
      stage: "sending",
      detail: null,
      elapsedMs: 500,
      account: null,
      failureCode: "timeout",
      error: "TIMEOUT: qua han",
    });
  });

  it("node_state: cap nhat done/ok/failed/total dung tung truong, suy runId neu chua co", () => {
    const s1 = applyEvent(EMPTY_RUN_STATE, {
      type: "node_state",
      runId: "r1",
      nodeId: "n1",
      state: "queued",
      done: 0,
      total: 3,
      ok: 0,
      failed: 0,
    });
    expect(s1.runId).toBe("r1");
    expect(s1.nodes["n1"]).toEqual({ nodeId: "n1", state: "queued", done: 0, total: 3, ok: 0, failed: 0 });

    const s2 = applyEvent(s1, {
      type: "node_state",
      runId: "r1",
      nodeId: "n1",
      state: "running",
      done: 2,
      total: 3,
      ok: 1,
      failed: 1,
    });
    expect(s2.nodes["n1"]).toEqual({ nodeId: "n1", state: "running", done: 2, total: 3, ok: 1, failed: 1 });
  });

  it("run_done: ghi status ket thuc, la trang thai TERMINAL (khong bi su kien khac ghi de nguoc)", () => {
    const running = applyEvent(EMPTY_RUN_STATE, {
      type: "node_state",
      runId: "r1",
      nodeId: "n1",
      state: "running",
      done: 1,
      total: 3,
      ok: 1,
      failed: 0,
    });
    expect(running.status).toBeNull();

    const done = applyEvent(running, { type: "run_done", runId: "r1", status: "completed" });
    expect(done.status).toBe("completed");
    expect(done.runId).toBe("r1");

    // Su kien den SAU run_done (vd task_stage tre cua mot canh cham) khong duoc lam status quay
    // ve null — status chi doi khi co mot run_done KHAC ghi de.
    const after = applyEvent(done, stage("t9", "downloading", 1));
    expect(after.status).toBe("completed");
  });

  it("review fix-round 1 C1: node_state cua RUN KHAC bi bo qua hoan toan, khong hijack runId", () => {
    const tracking: RunState = { ...EMPTY_RUN_STATE, runId: "r1" };
    const foreign = applyEvent(tracking, {
      type: "node_state",
      runId: "r2",
      nodeId: "n9",
      state: "running",
      done: 1,
      total: 5,
      ok: 1,
      failed: 0,
    });
    // Tra nguyen state — khong doi runId, khong them node "n9" vao map, khong ghi log su kien la.
    expect(foreign).toBe(tracking);
    expect(foreign.runId).toBe("r1");
    expect(foreign.nodes["n9"]).toBeUndefined();
  });

  it("review fix-round 1 C1: run_done cua RUN KHAC khong duoc phep 'ket thuc' run dang theo doi", () => {
    const tracking: RunState = { ...EMPTY_RUN_STATE, runId: "r1" };
    const foreign = applyEvent(tracking, { type: "run_done", runId: "r2", status: "completed" });
    expect(foreign).toBe(tracking);
    expect(foreign.runId).toBe("r1");
    expect(foreign.status).toBeNull();

    // Nguoc lai: dung run_id thi van hoat dong binh thuong.
    const done = applyEvent(tracking, { type: "run_done", runId: "r1", status: "completed" });
    expect(done.status).toBe("completed");
  });

  it("khi CHUA theo doi run nao (runId null), node_state/run_done dau tien van duoc nhan (khong phai run la)", () => {
    const s1 = applyEvent(EMPTY_RUN_STATE, {
      type: "node_state",
      runId: "r-moi",
      nodeId: "n1",
      state: "queued",
      done: 0,
      total: 1,
      ok: 0,
      failed: 0,
    });
    expect(s1.runId).toBe("r-moi");
  });

  it("review fix-round 1 M1: `events` gioi han 500 phan tu, bo cu nhat", () => {
    let s: RunState = EMPTY_RUN_STATE;
    for (let i = 0; i < 520; i += 1) {
      s = applyEvent(s, stage("t1", "generating", i));
    }
    expect(s.events).toHaveLength(500);
    expect((s.events[0] as { elapsedMs: number }).elapsedMs).toBe(20);
    expect((s.events[499] as { elapsedMs: number }).elapsedMs).toBe(519);
  });

  it("giu thu tu su kien trong log `events` (cong don, khong mat, dung thu tu phat)", () => {
    const e1 = stage("t1", "page", 1);
    const e2 = stage("t1", "mode", 2);
    const e3: EngineEvent = { type: "run_done", runId: "r1", status: "failed" };
    let s: RunState = EMPTY_RUN_STATE;
    s = applyEvent(s, e1);
    s = applyEvent(s, e2);
    s = applyEvent(s, e3);
    expect(s.events).toEqual([e1, e2, e3]);
  });

  it("khong lam thay doi state dau vao (thuan, khong mutate)", () => {
    const before = EMPTY_RUN_STATE;
    const snapshot = JSON.parse(JSON.stringify(before)) as unknown;
    applyEvent(before, stage("t1", "page", 1));
    expect(before).toEqual(snapshot);
  });
});

function runRow(id: string, status: RunRow["status"]): RunRow {
  return { id, workflowId: "wf1", status, startedAt: 1, finishedAt: status === "running" ? 0 : 2, error: "" };
}

function taskDto(id: string, runId: string, overrides: Partial<TaskDto> = {}): TaskDto {
  return {
    id,
    promptPreview: "canh 1",
    status: "processing",
    failureCode: null,
    error: null,
    account: null,
    mode: "fast",
    duration: 10,
    ratio: "9:16",
    model: "seedance-2.0",
    localPath: null,
    sceneIndex: 0,
    runId,
    nodeId: "n1",
    stage: "sending",
    createdAt: 1,
    startedAt: 1,
    finishedAt: null,
    hasConversation: false,
    ...overrides,
  };
}

// review fix-round 1 C2: dong bo lai `status` (`reconcileFromRunRow`) va tung `tasks`
// (`reconcileTasksFromDb`) tu DB (`run_list`/`task_list`) khi trang Workflow mo lai va nghi ngo
// da bo lo `run_done`/su kien task — xem doc trong store/run.ts.
describe("reconcileFromRunRow — dong bo status tu RunRow (review C2)", () => {
  it("row khop runId dang theo doi va da ket thuc -> ghi status", () => {
    const state: RunState = { ...EMPTY_RUN_STATE, runId: "r1" };
    const next = reconcileFromRunRow(state, runRow("r1", "completed"));
    expect(next.status).toBe("completed");
  });

  it("row khop runId nhung VAN dang 'running' -> khong doi gi (tra nguyen state)", () => {
    const state: RunState = { ...EMPTY_RUN_STATE, runId: "r1" };
    const next = reconcileFromRunRow(state, runRow("r1", "running"));
    expect(next).toBe(state);
  });

  it("row KHONG khop runId dang theo doi -> bo qua hoan toan", () => {
    const state: RunState = { ...EMPTY_RUN_STATE, runId: "r1" };
    const next = reconcileFromRunRow(state, runRow("r-khac", "completed"));
    expect(next).toBe(state);
    expect(next.status).toBeNull();
  });

  it("status da khop san (vd run_done that su da toi truoc do) -> tra nguyen tham chieu, khong render lai thua", () => {
    const state: RunState = { ...EMPTY_RUN_STATE, runId: "r1", status: "completed" };
    const next = reconcileFromRunRow(state, runRow("r1", "completed"));
    expect(next).toBe(state);
  });
});

describe("reconcileTasksFromDb — dong bo tung task tu TaskDto (review C2)", () => {
  it("bo sung task chua co trong state.tasks tu DB", () => {
    const state: RunState = { ...EMPTY_RUN_STATE, runId: "r1" };
    const next = reconcileTasksFromDb(state, [
      taskDto("t1", "r1", { status: "completed", account: "acc1", sceneIndex: 0 }),
    ]);
    expect(next.tasks["t1"]?.status).toBe("done");
    expect(next.tasks["t1"]?.account).toBe("acc1");
  });

  it("KHONG ghi de mot task da co trang thai ket thuc CUC BO (song moi hon co the da toi)", () => {
    const local: RunState = {
      ...EMPTY_RUN_STATE,
      runId: "r1",
      tasks: {
        t1: {
          taskId: "t1",
          status: "failed",
          stage: "sending",
          detail: null,
          elapsedMs: 10,
          account: "acc-that",
          failureCode: "timeout",
          error: "TIMEOUT",
        },
      },
    };
    const next = reconcileTasksFromDb(local, [taskDto("t1", "r1", { status: "completed" })]);
    expect(next).toBe(local);
    expect(next.tasks["t1"]?.status).toBe("failed");
  });

  it("dto thuoc run KHAC bi bo qua", () => {
    const state: RunState = { ...EMPTY_RUN_STATE, runId: "r1" };
    const next = reconcileTasksFromDb(state, [taskDto("t1", "r-khac")]);
    expect(next).toBe(state);
    expect(next.tasks["t1"]).toBeUndefined();
  });
});
