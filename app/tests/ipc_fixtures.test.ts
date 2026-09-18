// TDD Task 11 buoc 1: fixture JSON commit boi Task 9 (`src/ipc/fixtures/*.json`, sinh THAT tu
// Rust — `cargo test -p lvc-app commands::fixtures`) la nguon su that cho hop dong IPC. Test nay
// ghim schema zod cua `src/ipc/types.ts` + bang `PORTS`/`STAGES` vao dung 12 file do — lech mot ky
// tu la fail, khong the am tham trôi.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { z } from "zod";

import {
  AppSettingsSchema,
  EngineEventSchema,
  GraphErrorSchema,
  GraphSchema,
  IpcErrorSchema,
  NODE_KIND_ORDER,
  PlanSummarySchema,
  PortsFixtureSchema,
  RequeueReportSchema,
  RunRowSchema,
  STAGES,
  StageSchema,
  TaskDtoSchema,
  WorkflowMetaSchema,
} from "../src/ipc/types";
import { PORTS } from "../src/graph/ports";

const FIXTURES_DIR = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "src",
  "ipc",
  "fixtures",
);

function loadFixture(name: string): unknown {
  const raw = readFileSync(path.join(FIXTURES_DIR, name), "utf-8");
  return JSON.parse(raw) as unknown;
}

interface AnySchema {
  parse: (data: unknown) => unknown;
}

const FIXTURE_CASES: ReadonlyArray<readonly [string, AnySchema]> = [
  ["app_settings.json", AppSettingsSchema],
  ["engine_events.json", z.array(EngineEventSchema)],
  ["graph.json", GraphSchema],
  ["graph_errors.json", z.array(GraphErrorSchema)],
  ["ipc_error.json", IpcErrorSchema],
  ["plan_summary.json", PlanSummarySchema],
  ["ports.json", PortsFixtureSchema],
  ["requeue_report.json", RequeueReportSchema],
  ["run_row.json", RunRowSchema],
  ["stages.json", z.array(StageSchema)],
  ["task_dto.json", z.array(TaskDtoSchema)],
  ["workflow_meta.json", WorkflowMetaSchema],
];

describe("fixture IPC (Task 9) parse dung voi schema zod (Task 11)", () => {
  it("bao phu dung 12 file da commit — khong sot, khong thua", () => {
    expect(FIXTURE_CASES).toHaveLength(12);
  });

  it.each(FIXTURE_CASES)("%s parse OK", (file, schema) => {
    const data = loadFixture(file);
    expect(() => schema.parse(data)).not.toThrow();
  });
});

describe("ports.json khop bang PORTS (src/graph/ports.ts)", () => {
  it("cau truc giong het, theo dung thu tu NODE_KIND_ORDER", () => {
    const fixture = PortsFixtureSchema.parse(loadFixture("ports.json"));
    const expected = NODE_KIND_ORDER.map((kind) => ({
      kind,
      inputs: PORTS[kind].inputs,
      outputs: PORTS[kind].outputs,
    }));
    expect(fixture).toEqual(expected);
  });
});

describe("stages.json khop STAGES (src/ipc/types.ts)", () => {
  it("danh sach giong het, dung thu tu", () => {
    const fixture = z.array(StageSchema).parse(loadFixture("stages.json"));
    expect(fixture).toEqual([...STAGES]);
  });
});
