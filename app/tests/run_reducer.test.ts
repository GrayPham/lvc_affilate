// TDD Task 12 buoc 1: `applyEvent(state, EngineEvent)` (src/store/run.ts) la reducer THUAN xu
// ly chuoi su kien `engine://event` (crates/lvc-engine/src/events.rs) — moi bien the phai cap
// nhat dung truong, giu thu tu (`events` la log cong don), va `run_done` la trang thai ket thuc
// (khong con la `null`, khong bi su kien sau do "reset" nguoc lai).
import { describe, expect, it } from "vitest";

import { applyEvent, EMPTY_RUN_STATE, type RunState } from "../src/store/run";
import type { EngineEvent } from "../src/ipc/types";

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
