// TDD P3c Task 2 buoc 1 — `format/scene.ts`: nhan trang thai canh, dieu kien hien nut
// Chay/Chay lai/Dung, nhan ma loi rieng cua kich ban, va reducer chon canh muc tieu cho
// "Đặt lại trạng thái" (`sceneIdsToRun`).
import { describe, expect, it } from "vitest";

import {
  canRerun,
  canRun,
  canStop,
  sceneIdsToRun,
  sceneStatusLabel,
  scriptErrorLabel,
} from "../src/format/scene";
import type { ScriptDoc, SceneDto } from "../src/ipc/types";

function scene(overrides: Partial<SceneDto> = {}): SceneDto {
  return {
    id: "scene_1",
    idx: 1,
    prompt: "canh 1",
    ratio: null,
    duration: null,
    model: null,
    mode: null,
    logo: null,
    account: "",
    refs: [],
    taskId: null,
    taskStatus: null,
    failureCode: null,
    localPath: null,
    ...overrides,
  };
}

describe("sceneStatusLabel", () => {
  it('"Chưa chạy" khi taskStatus la null (chua tung chay hoac task_id mo coi)', () => {
    expect(sceneStatusLabel(scene({ taskStatus: null }))).toBe("Chưa chạy");
  });

  it("dung nhan tieng Viet cho tung trang thai task (queued/processing/completed)", () => {
    expect(sceneStatusLabel(scene({ taskStatus: "queued" }))).toBe("Đang chờ");
    expect(sceneStatusLabel(scene({ taskStatus: "processing" }))).toBe("Đang xử lý");
    expect(sceneStatusLabel(scene({ taskStatus: "completed" }))).toBe("Hoàn tất");
  });

  it("failed kem failureCode -> gop nhan loi vao trong ngoac", () => {
    const label = sceneStatusLabel(scene({ taskStatus: "failed", failureCode: "PROXY_DOWN" }));
    expect(label).toBe("Thất bại (Proxy không hoạt động)");
  });

  it("failed nhung khong co failureCode -> chi nhan trang thai, khong co ngoac rong", () => {
    expect(sceneStatusLabel(scene({ taskStatus: "failed", failureCode: null }))).toBe("Thất bại");
  });
});

describe("canRun / canRerun / canStop", () => {
  it("canRun: true khi taskStatus null hoac failed, false khi queued/processing/completed", () => {
    expect(canRun(scene({ taskStatus: null }))).toBe(true);
    expect(canRun(scene({ taskStatus: "failed" }))).toBe(true);
    expect(canRun(scene({ taskStatus: "queued" }))).toBe(false);
    expect(canRun(scene({ taskStatus: "processing" }))).toBe(false);
    expect(canRun(scene({ taskStatus: "completed" }))).toBe(false);
  });

  it("canRerun: chi true khi completed", () => {
    expect(canRerun(scene({ taskStatus: "completed" }))).toBe(true);
    expect(canRerun(scene({ taskStatus: "failed" }))).toBe(false);
    expect(canRerun(scene({ taskStatus: null }))).toBe(false);
  });

  it("canStop: chi true khi queued hoac processing", () => {
    expect(canStop(scene({ taskStatus: "queued" }))).toBe(true);
    expect(canStop(scene({ taskStatus: "processing" }))).toBe(true);
    expect(canStop(scene({ taskStatus: "completed" }))).toBe(false);
    expect(canStop(scene({ taskStatus: "failed" }))).toBe(false);
    expect(canStop(scene({ taskStatus: null }))).toBe(false);
  });
});

describe("scriptErrorLabel", () => {
  it("dung nhan tieng Viet cho 5 ma da biet", () => {
    expect(scriptErrorLabel("SCENE_RUNNING")).toBe("Cảnh đang chạy");
    expect(scriptErrorLabel("SCENE_EMPTY")).toBe("Prompt trống");
    expect(scriptErrorLabel("SCENE_INVALID")).toBe("Cảnh không hợp lệ");
    expect(scriptErrorLabel("TOO_MANY_SCENES")).toBe("Quá nhiều cảnh");
    expect(scriptErrorLabel("SCRIPT_NOT_FOUND")).toBe("Không tìm thấy kịch bản");
  });

  it("ma la nhung khong nam trong bang -> tra nguyen ma tho", () => {
    expect(scriptErrorLabel("GRAPH_INVALID")).toBe("GRAPH_INVALID");
  });
});

describe("sceneIdsToRun", () => {
  const doc: Pick<ScriptDoc, "scenes"> = {
    scenes: [
      scene({ id: "s1", idx: 1, taskStatus: "completed" }),
      scene({ id: "s2", idx: 2, taskStatus: "failed" }),
      scene({ id: "s3", idx: 3, taskStatus: null }),
      scene({ id: "s4", idx: 4, taskStatus: "failed" }),
    ],
  };

  it("selectedIds rong -> roi ve MOI canh dang failed, theo dung thu tu doc.scenes", () => {
    expect(sceneIdsToRun(doc, new Set())).toEqual(["s2", "s4"]);
  });

  it("selectedIds khong rong -> uu tien danh sach da chon (loc theo doc, giu thu tu doc.scenes)", () => {
    expect(sceneIdsToRun(doc, new Set(["s3", "s1"]))).toEqual(["s1", "s3"]);
  });

  it("id da chon khong con trong doc -> bi loc bo, khong lam sai ket qua", () => {
    expect(sceneIdsToRun(doc, new Set(["s1", "ma-coi"]))).toEqual(["s1"]);
  });
});
