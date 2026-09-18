// TDD Task 13 fix round 1 (review task-13-review.md, Important #2) — `cancelTask` phai DOC gia
// tri boolean `taskCancel` tra ve (`task_cancel_impl` -> `Engine::cancel` ->
// `CancelRegistry::cancel`, crates/lvc-engine/src/cancel.rs) thay vi bo qua: `false` nghia la
// khong con token huy song cho task nay (vd task `queued` mo coi tu phien truoc chua duoc run moi
// nhan, hoac task da xong dung luc lenh toi engine) — phai bao loi ro rang cho nguoi dung thay vi
// im lang khong lam gi.
//
// Mock DUY NHAT lop `src/ipc/tauri.ts` (cung quy uoc voi `tests/store_accounts.test.ts`) —
// `commands.ts` (zod parse that) khong bi mock.
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TaskDto } from "../src/ipc/types";
import { DEFAULT_TASK_FILTER } from "../src/store/tasks";

const mockInvoke = vi.fn(
  (_cmd: string, _args?: Record<string, unknown>): Promise<unknown> =>
    Promise.reject(new Error("mockInvoke: chua cau hinh cho lenh nay")),
);

vi.mock("../src/ipc/tauri", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => mockInvoke(cmd, args),
  listen: vi.fn(),
}));

import { useTasksStore } from "../src/store/tasks";

function makeTask(overrides: Partial<TaskDto> = {}): TaskDto {
  return {
    id: "t1",
    promptPreview: "canh 1",
    status: "queued",
    failureCode: null,
    error: null,
    account: null,
    mode: "fast",
    duration: 10,
    ratio: "9:16",
    model: "seedance-2.0",
    localPath: null,
    sceneIndex: 1,
    runId: "run_1",
    nodeId: "n2",
    stage: "launching",
    createdAt: 1_700_000_000,
    startedAt: null,
    finishedAt: null,
    hasConversation: false,
    ...overrides,
  };
}

beforeEach(() => {
  mockInvoke.mockReset();
  useTasksStore.setState({
    rows: [],
    filter: DEFAULT_TASK_FILTER,
    loading: false,
    error: null,
    pendingIds: new Set(),
  });
});

describe("cancelTask — doc gia tri boolean cua taskCancel (review Important #2)", () => {
  it("task_cancel tra false -> bao loi ro rang, khong im lang", async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "task_cancel") {
        return Promise.resolve(false);
      }
      if (cmd === "task_list") {
        return Promise.resolve([makeTask({ id: "t1", status: "queued" })]);
      }
      return Promise.reject(new Error(`lenh khong mong doi trong test nay: ${cmd}`));
    });

    await useTasksStore.getState().cancelTask("t1");

    expect(useTasksStore.getState().error).toBe(
      "Không huỷ được: tác vụ không còn chạy (có thể đã xong hoặc từ phiên trước)",
    );
    // Van phai refresh sau do (danh sach co the da doi — vd task vua chuyen completed).
    expect(mockInvoke).toHaveBeenCalledWith("task_list", { runId: null, limit: 200 });
  });

  it("task_cancel tra true -> KHONG bao loi", async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "task_cancel") {
        return Promise.resolve(true);
      }
      if (cmd === "task_list") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`lenh khong mong doi trong test nay: ${cmd}`));
    });

    await useTasksStore.getState().cancelTask("t1");

    expect(useTasksStore.getState().error).toBeNull();
  });

  it("pendingIds duoc xoa sau khi xong du task_cancel tra false", async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "task_cancel") {
        return Promise.resolve(false);
      }
      if (cmd === "task_list") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`lenh khong mong doi trong test nay: ${cmd}`));
    });

    await useTasksStore.getState().cancelTask("t1");

    expect(useTasksStore.getState().pendingIds.has("t1")).toBe(false);
  });

  it("task_cancel nem loi (IpcError) -> van bao loi qua errorText, khac thong diep 'false'", async () => {
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === "task_cancel") {
        return Promise.reject({ code: "STORE", message: "loi gia lap", details: null });
      }
      if (cmd === "task_list") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`lenh khong mong doi trong test nay: ${cmd}`));
    });

    await useTasksStore.getState().cancelTask("t1");

    expect(useTasksStore.getState().error).toBe("STORE: loi gia lap");
  });
});
