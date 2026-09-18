// Kieu TypeScript + schema zod soi guong 1-1 cac kieu Rust duoc Tauri (lvc-app, Task 9)
// serialize qua IPC. MOI kieu o day phai khop dung fixture JSON commit tai
// `src/ipc/fixtures/*.json` (sinh boi `cargo test -p lvc-app commands::fixtures`, xem
// `tests/ipc_fixtures.test.ts`) — lech fixture la lech hop dong that giua Rust va UI.
//
// Hai quy uoc case khac nhau CO CHU DICH, khong phai loi go:
// - `Graph`/`Node`/`Edge`/`NodeKind` (crates/lvc-core/src/graph/{node,edge,mod}.rs) CHI co
//   `#[serde(rename_all = "snake_case")]` tren TAG (`"kind"`) — cac truong DU LIEU ben trong
//   (`from_port`, `to_port`, `out_dir`, ...) giu nguyen snake_case nhu khai bao Rust.
// - `GraphError`/`EngineEvent` (validate.rs, lvc-engine/events.rs) co THEM
//   `rename_all_fields = "camelCase"` — truong ben trong (`sceneIndex`, `taskId`, `elapsedMs`,
//   ...) la camelCase.
// - Cac struct DTO rieng cua lvc-app/lvc-store (`PlanSummary`, `WorkflowMeta`, `RunRow`,
//   `TaskDto`, `AppSettings`, `IpcError`, ...) deu `#[serde(rename_all = "camelCase")]` toan bo.
import { z } from "zod";

// ---------------------------------------------------------------------------------------------
// Enum dung chung (crates/lvc-core/src/{mode,value}.rs) — chuoi wire khop `as_str()`/serde.
// ---------------------------------------------------------------------------------------------

export const ModeSchema = z.enum(["fast", "pro", "pro_max", "agent"]);
export type Mode = z.infer<typeof ModeSchema>;

export const ModelSchema = z.enum(["seedance-2.0", "seedance-2.5", "seedance-1.0"]);
export type Model = z.infer<typeof ModelSchema>;

export const RatioSchema = z.enum(["9:16", "16:9", "1:1", "4:3", "3:4"]);
export type Ratio = z.infer<typeof RatioSchema>;

export const LogoModeSchema = z.enum(["off", "crop", "delogo", "auto"]);
export type LogoMode = z.infer<typeof LogoModeSchema>;

export const PortTypeSchema = z.enum(["text", "image", "video"]);
export type PortType = z.infer<typeof PortTypeSchema>;

// ---------------------------------------------------------------------------------------------
// Graph — crates/lvc-core/src/graph/{node.rs,edge.rs,mod.rs}. Dinh dang file workflow JSON.
// ---------------------------------------------------------------------------------------------

export const NodeKindTagSchema = z.enum([
  "prompt",
  "image_upload",
  "image_from_url",
  "image_gen",
  "video",
  "remove_logo",
  "ensure_h264",
  "concat",
  "save",
]);
export type NodeKindTag = z.infer<typeof NodeKindTagSchema>;

/** Danh sach `NodeKindTag` theo dung thu tu ghim trong `fixtures/ports.json`. */
export const NODE_KIND_ORDER: readonly NodeKindTag[] = NodeKindTagSchema.options;

const PromptKindSchema = z.object({ kind: z.literal("prompt"), text: z.string() });
const ImageUploadKindSchema = z.object({
  kind: z.literal("image_upload"),
  paths: z.array(z.string()),
});
const ImageFromUrlKindSchema = z.object({
  kind: z.literal("image_from_url"),
  url: z.string(),
});
const ImageGenKindSchema = z.object({
  kind: z.literal("image_gen"),
  prompt: z.string(),
  backend: z.string(),
});
const VideoKindSchema = z.object({
  kind: z.literal("video"),
  mode: ModeSchema,
  model: ModelSchema,
  ratio: RatioSchema,
  duration: z.number(),
  logo: LogoModeSchema,
  account: z.string(),
});
const RemoveLogoKindSchema = z.object({ kind: z.literal("remove_logo") });
const EnsureH264KindSchema = z.object({ kind: z.literal("ensure_h264") });
const ConcatKindSchema = z.object({ kind: z.literal("concat") });
const SaveKindSchema = z.object({ kind: z.literal("save"), out_dir: z.string() });

export const NodeKindSchema = z.discriminatedUnion("kind", [
  PromptKindSchema,
  ImageUploadKindSchema,
  ImageFromUrlKindSchema,
  ImageGenKindSchema,
  VideoKindSchema,
  RemoveLogoKindSchema,
  EnsureH264KindSchema,
  ConcatKindSchema,
  SaveKindSchema,
]);
export type NodeKind = z.infer<typeof NodeKindSchema>;

export const NodeSchema = z.object({
  id: z.string(),
  kind: NodeKindSchema,
  x: z.number(),
  y: z.number(),
});
export type Node = z.infer<typeof NodeSchema>;

export const EdgeSchema = z.object({
  id: z.string(),
  from: z.string(),
  from_port: z.string(),
  to: z.string(),
  to_port: z.string(),
});
export type Edge = z.infer<typeof EdgeSchema>;

export const GraphSchema = z.object({
  version: z.number(),
  nodes: z.array(NodeSchema),
  edges: z.array(EdgeSchema),
});
export type Graph = z.infer<typeof GraphSchema>;

// ---------------------------------------------------------------------------------------------
// GraphError — crates/lvc-core/src/graph/validate.rs (tag `type`, truong camelCase).
// ---------------------------------------------------------------------------------------------

const DuplicateNodeIdSchema = z.object({
  type: z.literal("duplicate_node_id"),
  id: z.string(),
});
const UnknownNodeSchema = z.object({ type: z.literal("unknown_node"), id: z.string() });
const UnknownPortSchema = z.object({
  type: z.literal("unknown_port"),
  node: z.string(),
  port: z.string(),
});
const TypeMismatchSchema = z.object({
  type: z.literal("type_mismatch"),
  edge: z.string(),
  from: PortTypeSchema,
  to: PortTypeSchema,
});
const MissingRequiredInputSchema = z.object({
  type: z.literal("missing_required_input"),
  node: z.string(),
  port: z.string(),
});
const CycleSchema = z.object({ type: z.literal("cycle"), nodes: z.array(z.string()) });
const UnsupportedSchema = z.object({
  type: z.literal("unsupported"),
  node: z.string(),
  kind: z.string(),
});
const EmptySchema = z.object({ type: z.literal("empty") });
const MultipleInputsOnSinglePortSchema = z.object({
  type: z.literal("multiple_inputs_on_single_port"),
  node: z.string(),
  port: z.string(),
});
const SceneErrorSchema = z.object({
  type: z.literal("scene"),
  node: z.string(),
  line: z.number().nullable(),
  message: z.string(),
});
const TooManyRefsSchema = z.object({
  type: z.literal("too_many_refs"),
  node: z.string(),
  sceneIndex: z.number(),
  count: z.number(),
  max: z.number(),
});

export const GraphErrorSchema = z.discriminatedUnion("type", [
  DuplicateNodeIdSchema,
  UnknownNodeSchema,
  UnknownPortSchema,
  TypeMismatchSchema,
  MissingRequiredInputSchema,
  CycleSchema,
  UnsupportedSchema,
  EmptySchema,
  MultipleInputsOnSinglePortSchema,
  SceneErrorSchema,
  TooManyRefsSchema,
]);
export type GraphError = z.infer<typeof GraphErrorSchema>;

// ---------------------------------------------------------------------------------------------
// EngineEvent — crates/lvc-engine/src/events.rs (tag `type`, truong camelCase). Phat qua kenh
// Tauri `engine://event` (xem `src/ipc/events.ts`).
// ---------------------------------------------------------------------------------------------

const TaskStageEventSchema = z.object({
  type: z.literal("task_stage"),
  taskId: z.string(),
  stage: z.string(),
  detail: z.string().nullable(),
  elapsedMs: z.number(),
});
const TaskDoneEventSchema = z.object({
  type: z.literal("task_done"),
  taskId: z.string(),
  account: z.string(),
  conversationId: z.string(),
  localPath: z.string(),
  bytes: z.number(),
  watermarked: z.boolean(),
});
const TaskFailedEventSchema = z.object({
  type: z.literal("task_failed"),
  taskId: z.string(),
  code: z.string().nullable(),
  message: z.string(),
});
const NodeStateEventSchema = z.object({
  type: z.literal("node_state"),
  runId: z.string(),
  nodeId: z.string(),
  state: z.string(),
  done: z.number(),
  total: z.number(),
  ok: z.number(),
  failed: z.number(),
});
const RunDoneEventSchema = z.object({
  type: z.literal("run_done"),
  runId: z.string(),
  status: z.string(),
});

export const EngineEventSchema = z.discriminatedUnion("type", [
  TaskStageEventSchema,
  TaskDoneEventSchema,
  TaskFailedEventSchema,
  NodeStateEventSchema,
  RunDoneEventSchema,
]);
export type EngineEvent = z.infer<typeof EngineEventSchema>;

// ---------------------------------------------------------------------------------------------
// DTO lenh IPC (crates/lvc-app/src/commands/*.rs, crates/lvc-store/src/workflows.rs) — toan bo
// `rename_all = "camelCase"`.
// ---------------------------------------------------------------------------------------------

export const IpcErrorSchema = z.object({
  code: z.string(),
  message: z.string(),
  details: z.unknown().nullable(),
});
export type IpcError = z.infer<typeof IpcErrorSchema>;

export const PlanSummarySchema = z.object({
  videoTotal: z.number(),
  warnings: z.array(z.string()),
});
export type PlanSummary = z.infer<typeof PlanSummarySchema>;

export const WorkflowMetaSchema = z.object({
  id: z.string(),
  name: z.string(),
  updatedAt: z.number(),
});
export type WorkflowMeta = z.infer<typeof WorkflowMetaSchema>;

export const WorkflowDocSchema = z.object({
  meta: WorkflowMetaSchema,
  graph: GraphSchema,
});
export type WorkflowDoc = z.infer<typeof WorkflowDocSchema>;

export const RunInfoSchema = z.object({ runId: z.string() });
export type RunInfo = z.infer<typeof RunInfoSchema>;

export const RunStatusSchema = z.enum(["running", "completed", "failed", "cancelled"]);
export type RunStatus = z.infer<typeof RunStatusSchema>;

export const RunRowSchema = z.object({
  id: z.string(),
  workflowId: z.string(),
  status: RunStatusSchema,
  startedAt: z.number(),
  finishedAt: z.number(),
  error: z.string(),
});
export type RunRow = z.infer<typeof RunRowSchema>;

export const TaskDtoSchema = z.object({
  id: z.string(),
  promptPreview: z.string(),
  status: z.string(),
  failureCode: z.string().nullable(),
  error: z.string().nullable(),
  account: z.string().nullable(),
  mode: z.string(),
  duration: z.number(),
  ratio: z.string(),
  model: z.string(),
  localPath: z.string().nullable(),
  sceneIndex: z.number().nullable(),
  runId: z.string(),
  nodeId: z.string(),
  stage: z.string(),
  createdAt: z.number(),
  startedAt: z.number().nullable(),
  finishedAt: z.number().nullable(),
  hasConversation: z.boolean(),
});
export type TaskDto = z.infer<typeof TaskDtoSchema>;

export const TaskResumeInfoSchema = z.object({ taskId: z.string() });
export type TaskResumeInfo = z.infer<typeof TaskResumeInfoSchema>;

export const AppSettingsSchema = z.object({
  threads: z.number(),
  silentMode: z.boolean(),
  modeFallback: z.boolean(),
  accountsDir: z.string(),
  outputDir: z.string(),
  defaultMode: ModeSchema,
  defaultModel: ModelSchema,
  defaultRatio: RatioSchema,
  defaultDuration: z.number(),
});
export type AppSettings = z.infer<typeof AppSettingsSchema>;

export const RequeueReportSchema = z.object({
  requeued: z.array(z.string()),
  resumable: z.array(z.string()),
  // Fix-round 1 (final-review C1): run_id con "running" luc khoi dong lai -> da chuyen "failed"
  // (RunRepo::fail_running, crates/lvc-store/src/workflows.rs).
  runsFailed: z.array(z.string()),
});
export type RequeueReport = z.infer<typeof RequeueReportSchema>;

// ---------------------------------------------------------------------------------------------
// PortSpec — dinh dang wire cua `fixtures/ports.json` (`{kind, inputs:[{name,type}], outputs}`,
// sinh tu `NodeKind::inputs()/outputs()` that, xem doc fixture o task-9-report.md). Bang tra cuu
// day du theo `NodeKind` nam o `src/graph/ports.ts`.
// ---------------------------------------------------------------------------------------------

export const PortSpecSchema = z.object({
  name: z.string(),
  type: PortTypeSchema,
});
export type PortSpec = z.infer<typeof PortSpecSchema>;

export const PortsFixtureEntrySchema = z.object({
  kind: NodeKindTagSchema,
  inputs: z.array(PortSpecSchema),
  outputs: z.array(PortSpecSchema),
});
export type PortsFixtureEntry = z.infer<typeof PortsFixtureEntrySchema>;

export const PortsFixtureSchema = z.array(PortsFixtureEntrySchema);

// ---------------------------------------------------------------------------------------------
// Stage — crates/lvc-engine (task_stage.set_stage) qua `fixtures/stages.json`: danh sach buoc
// hop le cua `EngineEvent::TaskStage.stage`/`TaskDto.stage` (STAGE_LAUNCHING..STAGE_DONE, da khu
// trung `generating`).
// ---------------------------------------------------------------------------------------------

export const STAGES = [
  "launching",
  "page",
  "mode",
  "composer",
  "refs",
  "prompt",
  "sending",
  "generating",
  "confirm",
  "downloading",
  "done",
] as const;
export const StageSchema = z.enum(STAGES);
export type Stage = z.infer<typeof StageSchema>;

// ---------------------------------------------------------------------------------------------
// Tai khoan (Task 10 `crates/lvc-engine/src/accounts.rs` + `lvc-app/src/commands/accounts.rs`,
// Task 14 man UI) — 7 lenh `account_*`/`legacy_import`, xem task-10-report.md "Exact command
// list" + fixtures `account_status.json`/`import_reports.json`.
// ---------------------------------------------------------------------------------------------

/** `crates/lvc-core/src/session.rs::SessionState` — `#[serde(rename_all = "snake_case")]`. */
export const SessionStateSchema = z.enum(["logged_in", "logged_out", "unknown"]);
export type SessionState = z.infer<typeof SessionStateSchema>;

/**
 * `AccountStatus` (`lvc-engine::accounts`) — moi truong o day co MAT trong Rust, `proxy` la chuoi
 * DA QUA `mask_proxy` (khong bao gio mat khau day du) — UI TUYET DOI khong tu "un-mask", chi gui
 * gia tri MOI khi luu (`ProxyEditor`). `loginOk` la `bool` THAT (khong nullable) — trang thai
 * "chua ro" duoc suy tu `loginCheckedAt <= 0` (chua tung kiem tra), xem `format/account.ts::
 * accountStatusLabel`.
 */
export const AccountStatusSchema = z.object({
  name: z.string(),
  scheduling: z.boolean(),
  note: z.string(),
  email: z.string(),
  createdAt: z.number(),
  lastUsedAt: z.number(),
  loginOk: z.boolean(),
  loginCheckedAt: z.number(),
  cooldownUntil: z.number(),
  cooling: z.boolean(),
  rateLimitedUntil: z.number(),
  rateLimited: z.boolean(),
  limitReason: z.string(),
  quotaBlockedUntil: z.number(),
  quotaBlocked: z.boolean(),
  quotaReason: z.string(),
  creditBalance: z.number().nullable(),
  creditCheckedAt: z.number(),
  proxy: z.string(),
  browser: z.string(),
  proxySet: z.boolean(),
  loggedOutAt: z.number(),
  autoVerifyState: z.string(),
  autoVerifyTries: z.number(),
  cookieExpiresAt: z.number(),
  usedToday: z.number(),
  limit: z.number(),
  remaining: z.number(),
  usedPro: z.number(),
  limitPro: z.number(),
  usedProMax: z.number(),
  limitProMax: z.number(),
  usedAgent: z.number(),
  limitAgent: z.number(),
});
export type AccountStatus = z.infer<typeof AccountStatusSchema>;

/** Ket qua `account_import_cookies`. */
export const ImportCookiesReportSchema = z.object({
  name: z.string(),
  sessionidExpiresAt: z.number().nullable(),
});
export type ImportCookiesReport = z.infer<typeof ImportCookiesReportSchema>;

/** Ket qua `account_verify` — `loginOk: null` khi `state === "unknown"` (DB KHONG doi). */
export const VerifyReportSchema = z.object({
  state: SessionStateSchema,
  loginOk: z.boolean().nullable(),
  checkedAt: z.number(),
});
export type VerifyReport = z.infer<typeof VerifyReportSchema>;

/**
 * Ket qua `legacy_import`. `profilesSkipped` la truong MOI (Task 10 fix-round 1, dang chay song
 * song luc Task 14 viet file nay) — `.default([])` de vAn parse duoc fixture CU (chua co truong
 * nay) LAN fixture MOI (da them), khong ep mot phia phai xong truoc.
 */
export const LegacyImportReportSchema = z.object({
  tasks: z.number(),
  accounts: z.number(),
  usage: z.number(),
  settings: z.number(),
  skipped: z.array(z.string()),
  profilesCopied: z.number(),
  profilesSkipped: z.array(z.string()).default([]),
});
export type LegacyImportReport = z.infer<typeof LegacyImportReportSchema>;

/**
 * Dinh dang RIENG cua fixture `import_reports.json` — gop `ImportCookiesReport` +
 * `LegacyImportReport` vao MOT file thay vi hai file gan-rong (xem task-10-report.md
 * "Fixtures"). Khong phai mot kieu Rust that — chi dung de test fixture nay parse dung.
 */
export const ImportReportsFixtureSchema = z.object({
  cookies: ImportCookiesReportSchema,
  legacy: LegacyImportReportSchema,
});
