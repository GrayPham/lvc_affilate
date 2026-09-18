// TDD Task 13 buoc 1: `failureLabel`/`stageLabel` phai bao phu DUNG danh sach dong (pin) tu Rust —
// 15 `FailureCode` (crates/lvc-core/src/failure.rs, hang `ALL` trong test
// `round_trip_parse_as_str_cho_ca_muoi_lam_gia_tri`) va 11 phan tu `fixtures/stages.json` (sinh tu
// STAGE_LAUNCHING..STAGE_DONE, xem `tests/ipc_fixtures.test.ts`). `isResumable`/`isCancellable`
// mirror dieu kien chan trong `task_resume_impl` (tasks.rs). `formatElapsed` dinh dang ms hien
// thi.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { describe, expect, it } from "vitest";

import {
  failureLabel,
  formatElapsed,
  isCancellable,
  isResumable,
  modeLabel,
  stageLabel,
  statusLabel,
} from "../src/format/task";
import type { TaskDto } from "../src/ipc/types";

// Doc truc tiep fixture JSON qua node:fs (khong dung import attribute `with { type: "json" }`) —
// cung mot cach lam voi `tests/ipc_fixtures.test.ts` (Task 11), tranh phu thuoc vao ho tro JSON
// module cua toolchain TS 7 / Vite 8 con moi.
const stagesFixture: string[] = JSON.parse(
  readFileSync(
    path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "src", "ipc", "fixtures", "stages.json"),
    "utf-8",
  ),
) as string[];

/** Sao chep dung tung chuoi tu `FailureCode::as_str()` (failure.rs) — 15 gia tri, hang `ALL`. */
const RUST_FAILURE_CODES: readonly string[] = [
  "cancelled",
  "LOGGED_OUT",
  "429",
  "ACCOUNT_UNAVAILABLE",
  "PROMPT_VIOLATION",
  "REF_UPLOAD",
  "PROFILE_BUSY",
  "PROXY_DOWN",
  "DURATION_UNAVAILABLE",
  "AGENT_QUESTION",
  "MODE_UNAVAILABLE",
  "NO_CONVERSATION",
  "RESTART",
  "RATIO_UNAVAILABLE",
  "AGENT_NO_VIDEO",
];

describe("failureLabel — bao phu dung 15 FailureCode (failure.rs)", () => {
  it("dung 15 ma, khong sot khong thua", () => {
    expect(RUST_FAILURE_CODES.length).toBe(15);
  });

  it.each(RUST_FAILURE_CODES)("%s -> nhan tieng Viet khac rong, khac chinh ma tho", (code) => {
    const label = failureLabel(code);
    expect(label.length).toBeGreaterThan(0);
    expect(label).not.toBe(code);
  });

  it("null (chua/khong loi) -> chuoi rong", () => {
    expect(failureLabel(null)).toBe("");
  });

  it("ma khong nam trong 15 gia tri -> tra nguyen ma tho (khong nem loi)", () => {
    expect(failureLabel("KHONG_TON_TAI")).toBe("KHONG_TON_TAI");
  });
});

describe("stageLabel — bao phu dung tung phan tu cua fixtures/stages.json", () => {
  it("stages.json dung 11 phan tu (ghim boi tests/ipc_fixtures.test.ts)", () => {
    expect(stagesFixture.length).toBe(11);
  });

  it.each(stagesFixture)("%s -> nhan tieng Viet khac rong, khac chinh stage tho", (stage) => {
    const label = stageLabel(stage);
    expect(label.length).toBeGreaterThan(0);
    expect(label).not.toBe(stage);
  });

  it("stage khong nam trong danh sach -> tra nguyen chuoi tho", () => {
    expect(stageLabel("khong-ton-tai")).toBe("khong-ton-tai");
  });
});

describe("statusLabel / modeLabel — nhan hien thi, fallback ve chuoi tho", () => {
  it.each(["queued", "processing", "completed", "failed"])("%s co nhan tieng Viet", (status) => {
    expect(statusLabel(status).length).toBeGreaterThan(0);
  });

  it("status la khong nam trong 4 gia tri -> tra nguyen", () => {
    expect(statusLabel("bogus")).toBe("bogus");
  });

  it.each(["fast", "pro", "pro_max", "agent"])("%s co nhan tieng Viet", (mode) => {
    expect(modeLabel(mode).length).toBeGreaterThan(0);
  });

  it("mode la khong nam trong 4 gia tri -> tra nguyen", () => {
    expect(modeLabel("bogus")).toBe("bogus");
  });
});

function baseTask(overrides: Partial<TaskDto>): TaskDto {
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

describe("isResumable — mirror hai dieu kien dau cua task_resume_impl (tasks.rs)", () => {
  it("failed + co hoi thoai -> resumable", () => {
    expect(isResumable(baseTask({ status: "failed", hasConversation: true }))).toBe(true);
  });

  it("queued + co hoi thoai -> resumable", () => {
    expect(isResumable(baseTask({ status: "queued", hasConversation: true }))).toBe(true);
  });

  it("failed nhung KHONG co hoi thoai -> khong resumable (NOT_RESUMABLE, tasks.rs dieu kien 3)", () => {
    expect(isResumable(baseTask({ status: "failed", hasConversation: false }))).toBe(false);
  });

  it.each(["processing", "completed"])(
    "%s (co hoi thoai) -> khong resumable (tasks.rs dieu kien 2)",
    (status) => {
      expect(isResumable(baseTask({ status, hasConversation: true }))).toBe(false);
    },
  );
});

describe("isCancellable", () => {
  it.each(["queued", "processing"])("%s -> huy duoc", (status) => {
    expect(isCancellable(baseTask({ status }))).toBe(true);
  });

  it.each(["completed", "failed"])("%s -> khong huy duoc nua", (status) => {
    expect(isCancellable(baseTask({ status }))).toBe(false);
  });
});

describe("formatElapsed", () => {
  it.each([
    [0, "0s"],
    [999, "0s"],
    [1000, "1s"],
    [59_000, "59s"],
    [60_000, "1p00s"],
    [65_000, "1p05s"],
    [3_661_000, "61p01s"],
  ] as const)("%i ms -> %s", (ms, want) => {
    expect(formatElapsed(ms)).toBe(want);
  });

  it("gia tri am -> 0s (khong nem loi, khong ra so am)", () => {
    expect(formatElapsed(-500)).toBe("0s");
  });
});
