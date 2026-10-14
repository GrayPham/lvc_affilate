// `buildTimeline`/`formatDuration`/`buildLogText` (src/components/run_log.ts) — gom cac dong
// `tasks` cua mot lan chay thanh dong thoi gian theo NODE cho hop thoai "Chi tiết lần chạy".
import { describe, expect, it } from "vitest";

import { buildLogText, buildTimeline, formatClock, formatDuration } from "../src/components/run_log";
import type { TaskDto } from "../src/ipc/types";

function task(over: Partial<TaskDto> = {}): TaskDto {
  return {
    id: "t1",
    promptPreview: "canh 1",
    status: "completed",
    failureCode: null,
    error: null,
    account: "reed01",
    mode: "fast",
    duration: 10,
    ratio: "9:16",
    model: "seedance-2.0",
    localPath: null,
    sceneIndex: 0,
    runId: "r1",
    nodeId: "n1",
    stage: "done",
    createdAt: 1000,
    startedAt: 1000,
    finishedAt: 1004,
    hasConversation: false,
    ...over,
  };
}

describe("buildTimeline", () => {
  it("gom nhieu canh cua CUNG mot node thanh MOT dong", () => {
    const rows = buildTimeline([
      task({ id: "a", nodeId: "n1", sceneIndex: 0 }),
      task({ id: "b", nodeId: "n1", sceneIndex: 1 }),
      task({ id: "c", nodeId: "n1", sceneIndex: 2 }),
    ]);
    expect(rows.length).toBe(1);
    expect(rows[0]!.taskCount).toBe(3);
    expect(rows[0]!.doneCount).toBe(3);
    expect(rows[0]!.state).toBe("ok");
  });

  it("giu THU TU XUAT HIEN cua node, khong sap lai theo id", () => {
    const rows = buildTimeline([
      task({ nodeId: "zz" }),
      task({ nodeId: "aa" }),
      task({ nodeId: "mm" }),
    ]);
    expect(rows.map((r) => r.nodeId)).toEqual(["zz", "aa", "mm"]);
    expect(rows.map((r) => r.index)).toEqual([1, 2, 3]);
  });

  it("MOT canh that bai -> ca node la 'err', kem ma loi va thong diep", () => {
    const rows = buildTimeline([
      task({ nodeId: "n1", status: "completed" }),
      task({
        nodeId: "n1",
        status: "failed",
        failureCode: "no_account",
        error: "khong con tai khoan",
      }),
    ]);
    expect(rows[0]!.state).toBe("err");
    expect(rows[0]!.failureCode).toBe("no_account");
    expect(rows[0]!.error).toBe("khong con tai khoan");
  });

  it("loi duoc uu tien hon dang chay — nguoi dung mo log la de tim cho hong", () => {
    const rows = buildTimeline([
      task({ nodeId: "n1", status: "processing" }),
      task({ nodeId: "n1", status: "failed", failureCode: "x" }),
    ]);
    expect(rows[0]!.state).toBe("err");
  });

  it("con canh dang xu ly -> 'running', kem buoc hien tai", () => {
    const rows = buildTimeline([
      task({ nodeId: "n1", status: "completed" }),
      task({ nodeId: "n1", status: "processing", stage: "generating" }),
    ]);
    expect(rows[0]!.state).toBe("running");
    expect(rows[0]!.stage).toBe("generating");
  });

  it("moi canh con dang cho -> 'skip'", () => {
    const rows = buildTimeline([task({ nodeId: "n1", status: "queued", startedAt: null, finishedAt: null })]);
    expect(rows[0]!.state).toBe("skip");
    expect(rows[0]!.durationSec).toBeNull();
  });

  it("thoi luong tinh tu moc bat dau SOM nhat den moc xong MUON nhat", () => {
    const rows = buildTimeline([
      task({ nodeId: "n1", startedAt: 100, finishedAt: 110 }),
      task({ nodeId: "n1", startedAt: 105, finishedAt: 130 }),
    ]);
    expect(rows[0]!.durationSec).toBe(30);
  });

  it("dong ho nhay lui khong bao gio ra thoi luong AM", () => {
    const rows = buildTimeline([task({ nodeId: "n1", startedAt: 200, finishedAt: 100 })]);
    expect(rows[0]!.durationSec).toBeNull();
  });

  it("lay tai khoan dau tien CO gia tri, bo qua canh khong co", () => {
    const rows = buildTimeline([
      task({ nodeId: "n1", account: null }),
      task({ nodeId: "n1", account: "lvcmedia11" }),
    ]);
    expect(rows[0]!.account).toBe("lvcmedia11");
  });

  it("danh sach rong -> khong co dong nao", () => {
    expect(buildTimeline([])).toEqual([]);
  });
});

describe("formatDuration", () => {
  it("dinh dang m:ss va h:mm:ss", () => {
    expect(formatDuration(4)).toBe("0:04");
    expect(formatDuration(201)).toBe("3:21");
    expect(formatDuration(3723)).toBe("1:02:03");
  });

  it("null -> chuoi rong (node chua chay)", () => {
    expect(formatDuration(null)).toBe("");
  });

  it("so am ve 0, khong ra dau tru", () => {
    expect(formatDuration(-5)).toBe("0:00");
  });
});

describe("formatClock", () => {
  it("moc 0/null -> dau gach, khong phai 1970", () => {
    expect(formatClock(0)).toBe("--:--:--");
    expect(formatClock(null)).toBe("--:--:--");
  });

  it("moc that -> gio:phut:giay 2 chu so", () => {
    expect(formatClock(1_760_000_000)).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });
});

describe("buildLogText", () => {
  it("moi task mot dong, canh that bai co ma loi + thong diep", () => {
    const text = buildLogText([
      task({ nodeId: "n1", status: "completed", stage: "done" }),
      task({
        nodeId: "n2",
        status: "failed",
        failureCode: "no_account",
        error: "het tai khoan",
        sceneIndex: 1,
      }),
    ]);
    const lines = text.split("\n");
    expect(lines.length).toBe(2);
    expect(lines[0]).toContain("node=n1");
    expect(lines[0]).toContain("buoc=done");
    expect(lines[1]).toContain("no_account");
    expect(lines[1]).toContain("het tai khoan");
    expect(lines[1]).toContain("canh=2");
  });

  it("danh sach rong -> chuoi rong, khong phai mot dong trong", () => {
    expect(buildLogText([])).toBe("");
  });
});
