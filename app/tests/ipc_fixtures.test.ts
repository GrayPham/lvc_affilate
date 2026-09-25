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
  AccountStatusSchema,
  AppSettingsSchema,
  CredentialListSchema,
  EngineEventSchema,
  GraphErrorSchema,
  GraphSchema,
  ImportReportsFixtureSchema,
  IpcErrorSchema,
  NODE_KIND_ORDER,
  NodeCatalogSchema,
  NodeKindTagSchema,
  PlanSummarySchema,
  PortsFixtureSchema,
  RequeueReportSchema,
  RunRowSchema,
  STAGES,
  ScriptDocSchema,
  ScriptMetaSchema,
  ScriptRunReportSchema,
  StageSchema,
  TaskDtoSchema,
  TemplateDocSchema,
  TemplateListSchema,
  VerifyOutcomeSchema,
  WorkflowMetaSchema,
} from "../src/ipc/types";
import { NODE_LABEL_VI, PORTS } from "../src/graph/ports";

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
  ["account_status.json", AccountStatusSchema],
  ["app_settings.json", AppSettingsSchema],
  ["engine_events.json", z.array(EngineEventSchema)],
  ["graph.json", GraphSchema],
  ["graph_errors.json", z.array(GraphErrorSchema)],
  ["import_reports.json", ImportReportsFixtureSchema],
  ["ipc_error.json", IpcErrorSchema],
  ["plan_summary.json", PlanSummarySchema],
  ["ports.json", PortsFixtureSchema],
  ["requeue_report.json", RequeueReportSchema],
  ["run_row.json", RunRowSchema],
  ["stages.json", z.array(StageSchema)],
  ["task_dto.json", z.array(TaskDtoSchema)],
  ["workflow_meta.json", WorkflowMetaSchema],
  // P3c Task 2 (frontend): 3 fixture kich ban sinh boi Task 1 (Rust) — xem
  // task-1-report.md "DTO" + task-2-report.md.
  ["script_meta.json", ScriptMetaSchema],
  ["script_doc.json", ScriptDocSchema],
  ["script_run_report.json", ScriptRunReportSchema],
  // P4a: manifest node (lenh `node_catalog`).
  ["catalog.json", NodeCatalogSchema],
  // P4c: kho tai khoan AI (lenh `credential_*`).
  ["credentials.json", CredentialListSchema],
  ["verify_outcome.json", VerifyOutcomeSchema],
  // P4d: kho template (lenh `template_*`).
  ["template_list.json", TemplateListSchema],
  ["template_doc.json", TemplateDocSchema],
];

describe("fixture IPC (Task 9 + Task 10 tai khoan + P3c Task 1/2 kich ban + P4c credentials + P4d template) parse dung voi schema zod (Task 11/14)", () => {
  it("bao phu dung 22 file da commit — khong sot, khong thua", () => {
    expect(FIXTURE_CASES).toHaveLength(22);
  });

  it.each(FIXTURE_CASES)("%s parse OK", (file, schema) => {
    const data = loadFixture(file);
    expect(() => schema.parse(data)).not.toThrow();
  });
});

describe("ports.json khop bang PORTS (src/graph/ports.ts)", () => {
  it("cau truc giong het (fixture xep theo ten kind, NODE_KIND_ORDER la thu tu Palette)", () => {
    const fixture = PortsFixtureSchema.parse(loadFixture("ports.json"));
    const expected = [...NODE_KIND_ORDER].sort().map((kind) => ({
      kind,
      inputs: PORTS[kind].inputs,
      outputs: PORTS[kind].outputs,
    }));
    expect(fixture).toEqual(expected);
  });
});

describe("catalog.json (P4a, manifest node) khop NODE_LABEL_VI + PORTS", () => {
  const catalog = NodeCatalogSchema.parse(loadFixture("catalog.json"));

  it("bao phu dung cac kind cua NODE_KIND_ORDER — khong sot, khong thua", () => {
    expect(catalog.map((m) => m.kind).sort()).toEqual([...NODE_KIND_ORDER].sort());
  });

  it.each(catalog.map((m) => [m.kind, m] as const))("%s: labelVi + cong vao/ra", (_kind, m) => {
    const kind = NodeKindTagSchema.parse(m.kind);
    expect(NODE_LABEL_VI[kind]).toBe(m.labelVi);
    const strip = (ports: typeof m.inputs) => ports.map((p) => ({ name: p.name, type: p.type }));
    expect(PORTS[kind].inputs).toEqual(strip(m.inputs));
    expect(PORTS[kind].outputs).toEqual(strip(m.outputs));
  });
});

describe("stages.json khop STAGES (src/ipc/types.ts)", () => {
  it("danh sach giong het, dung thu tu", () => {
    const fixture = z.array(StageSchema).parse(loadFixture("stages.json"));
    expect(fixture).toEqual([...STAGES]);
  });
});

describe("credentials.json — bat bien AN TOAN: KHONG bao gio lo bi mat nguyen van (P4c)", () => {
  it("khong `CredentialView` nao co truong ten 'secret' (chi duoc co 'secretHint' da che)", () => {
    const rows = CredentialListSchema.parse(loadFixture("credentials.json"));
    for (const row of rows) {
      // `secretHint` la BIEU DIEN DUY NHAT cua bi mat — khong duoc co truong 'secret' tho.
      expect(Object.prototype.hasOwnProperty.call(row, "secret")).toBe(false);
      expect(row).toHaveProperty("secretHint");
    }
  });

  it("meta cung khong chua khoa ten 'secret' (phong bi mat lot qua meta)", () => {
    const rows = CredentialListSchema.parse(loadFixture("credentials.json"));
    for (const row of rows) {
      expect(Object.prototype.hasOwnProperty.call(row.meta, "secret")).toBe(false);
    }
  });
});
