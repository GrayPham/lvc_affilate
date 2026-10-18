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

export const PortTypeSchema = z.enum(["text", "image", "video", "audio"]);
export type PortType = z.infer<typeof PortTypeSchema>;

// ---------------------------------------------------------------------------------------------
// Graph — crates/lvc-core/src/graph/{node.rs,edge.rs,mod.rs}. Dinh dang file workflow JSON.
// ---------------------------------------------------------------------------------------------

// P4a: node thanh module doc lap — CHI con 4 kind (5 kind P4 cu `image_from_url`/`image_gen`/
// `remove_logo`/`ensure_h264`/`concat` da bi xoa khoi backend; `video` doi ten `dola_video`, do thi
// cu duoc migrate o `src/graph/adapter.ts::migrateGraphJson`).
// `audio_extract`/`audio_split` (nut "Tách audio"/"Tách audio↔video" tren node video, kieu Lumina):
// `crates/nodes/node-audio-extract`/`node-audio-split`.
export const NodeKindTagSchema = z.enum([
  "prompt",
  "note",
  "image_upload",
  "load_video",
  "dola_video",
  "concat",
  "save",
  "trim",
  "frame_capture",
  "audio_extract",
  "audio_split",
  "script_gen",
  "script_agent",
  "veo3_image",
  "veo3_video",
  "google_image",
  "google_video",
  "seedance_image",
  "seedance_video",
  "grok_image",
  "museai_video",
  "museai_image",
  "tts",
]);
export type NodeKindTag = z.infer<typeof NodeKindTagSchema>;

/** Danh sach `NodeKindTag` theo thu tu hien tren Palette (`fixtures/ports.json` xep theo ten kind,
 * KHONG theo thu tu nay — test so sanh sau khi sap xep). */
export const NODE_KIND_ORDER: readonly NodeKindTag[] = NodeKindTagSchema.options;

const PromptKindSchema = z.object({ kind: z.literal("prompt"), text: z.string() });
// Node ghi chu tinh tren canvas (crates/nodes/node-note) — khong cong vao/ra, chi hien thi
// luu y/cach dung/cac buoc setup template cho nguoi doc sau.
const NoteKindSchema = z.object({ kind: z.literal("note"), text: z.string() });
const ImageUploadKindSchema = z.object({
  kind: z.literal("image_upload"),
  paths: z.array(z.string()),
  // P4a: nhan tuy chon (paramsSchema cua catalog.json) — UI chua dung.
  label: z.string().optional(),
});
const LoadVideoKindSchema = z.object({
  kind: z.literal("load_video"),
  paths: z.array(z.string()),
  label: z.string().optional(),
});
const DolaVideoKindSchema = z.object({
  kind: z.literal("dola_video"),
  mode: ModeSchema,
  model: ModelSchema,
  ratio: RatioSchema,
  duration: z.number(),
  logo: LogoModeSchema,
  account: z.string(),
});
const ConcatKindSchema = z.object({ kind: z.literal("concat") });
const SaveKindSchema = z.object({ kind: z.literal("save"), out_dir: z.string() });
const TrimKindSchema = z.object({
  kind: z.literal("trim"),
  start: z.number(),
  end: z.number(),
});
const FrameCaptureKindSchema = z.object({
  kind: z.literal("frame_capture"),
  time: z.number(),
});
const AudioFormatSchema = z.enum(["mp3", "aac", "wav"]);
export type AudioFormat = z.infer<typeof AudioFormatSchema>;
const AudioExtractKindSchema = z.object({
  kind: z.literal("audio_extract"),
  format: AudioFormatSchema,
});
const AudioSplitKindSchema = z.object({ kind: z.literal("audio_split") });
const ScriptGenKindSchema = z.object({
  kind: z.literal("script_gen"),
  topic: z.string(),
  num_scenes: z.number(),
  language: z.string(),
  style: z.string(),
  system: z.string(),
  credential: z.string(),
  provider: z.enum(["gemini", "openai"]),
  // Luu ket qua thanh kich ban moi trong tab "Kịch bản" khi chay (xem CLAUDE.md node-script-gen).
  save_as_script: z.boolean(),
  script_name: z.string(),
});
// Node "Agent kịch bản affiliate" (crates/nodes/node-script-agent) — hop dong giong het
// `script_gen` ngoai tru doi ten `system` -> `extra_instructions` (noi THEM vao chi dan affiliate
// co san, khong thay the - xem CLAUDE.md node-script-agent).
const ScriptAgentKindSchema = z.object({
  kind: z.literal("script_agent"),
  topic: z.string(),
  num_scenes: z.number(),
  language: z.string(),
  style: z.string(),
  extra_instructions: z.string(),
  credential: z.string(),
  provider: z.enum(["gemini", "openai"]),
  save_as_script: z.boolean(),
  script_name: z.string(),
});
// P5: node doc lap Google Labs Flow/Veo3 (crates/nodes/node-veo3-image, node-veo3-video) — khop
// dung ImageParams/VideoParams cua Rust (xem manifest.json cua tung crate). `account` la id
// credential BAT BUOC (chua co "auto" nhu Dola).
const Veo3ImageKindSchema = z.object({
  kind: z.literal("veo3_image"),
  model: z.string(),
  aspect: z.string(),
  seed: z.number().nullable().optional(),
  account: z.string(),
});
const Veo3VideoKindSchema = z.object({
  kind: z.literal("veo3_video"),
  mode: z.string(),
  model: z.string(),
  aspect: z.string(),
  seed_count: z.number(),
  account: z.string(),
});
// P6: node goi API CHINH THUC (crates/nodes/node-google-image, node-google-video) — khop dung
// ImageParams/VideoParams cua Rust. `credential` la TEN (label) API key Google, BAT BUOC.
const GoogleImageKindSchema = z.object({
  kind: z.literal("google_image"),
  model: z.string(),
  aspect: z.string(),
  credential: z.string(),
});
const GoogleVideoKindSchema = z.object({
  kind: z.literal("google_video"),
  mode: z.string(),
  model: z.string(),
  aspect: z.string(),
  credential: z.string(),
});
// P6: Seedance (Volcengine) + Grok (xAI) — khop ImageParams/VideoParams cua Rust.
const SeedanceImageKindSchema = z.object({
  kind: z.literal("seedance_image"),
  model: z.string(),
  aspect: z.string(),
  credential: z.string(),
});
const SeedanceVideoKindSchema = z.object({
  kind: z.literal("seedance_video"),
  mode: z.string(),
  model: z.string(),
  aspect: z.string(),
  credential: z.string(),
});
const GrokImageKindSchema = z.object({
  kind: z.literal("grok_image"),
  model: z.string(),
  credential: z.string(),
});
// Node MuseAI (muse.ai) — sinh video qua giao thuc Hatch/Noise that (xem CLAUDE.md
// crates/providers/lvc-museai muc "Co che tao video"). `account` la id credential BAT BUOC
// (`museai/login`), giong het `veo3_video` (chua co "auto" chon tai khoan).
const MuseaiVideoKindSchema = z.object({
  kind: z.literal("museai_video"),
  account: z.string(),
  // Chi CHEN THEM vao cau prompt (muse.ai khong co tham so API that — xac nhan qua GET /model) —
  // rong = khong chen gi, xem lvc_museai::protocol::client::build_video_prompt.
  aspect: z.string(),
  duration: z.number().nullable(),
});
// Node MuseAI anh tinh — cung giao thuc/co che dang nhap voi museai_video, chi khac cong ra (image).
const MuseaiImageKindSchema = z.object({
  kind: z.literal("museai_image"),
  account: z.string(),
});
// Node TTS (ElevenLabs/MiniMax, crates/nodes/node-tts) — cong `prompt` (Text*) -> `out` (Audio*),
// moi TextItem noi vao la mot cau doc rieng (giong museai_video/grok_image). `credential` la TEN
// (label) API key, BAT BUOC chon dung provider trong kho.
const TtsKindSchema = z.object({
  kind: z.literal("tts"),
  provider: z.enum(["elevenlabs", "minimax"]),
  voice_id: z.string(),
  model: z.string(),
  speed: z.number(),
  credential: z.string(),
});

export const NodeKindSchema = z.discriminatedUnion("kind", [
  PromptKindSchema,
  NoteKindSchema,
  ImageUploadKindSchema,
  LoadVideoKindSchema,
  DolaVideoKindSchema,
  ConcatKindSchema,
  SaveKindSchema,
  TrimKindSchema,
  FrameCaptureKindSchema,
  AudioExtractKindSchema,
  AudioSplitKindSchema,
  ScriptGenKindSchema,
  ScriptAgentKindSchema,
  Veo3ImageKindSchema,
  Veo3VideoKindSchema,
  GoogleImageKindSchema,
  GoogleVideoKindSchema,
  SeedanceImageKindSchema,
  SeedanceVideoKindSchema,
  GrokImageKindSchema,
  MuseaiVideoKindSchema,
  MuseaiImageKindSchema,
  TtsKindSchema,
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
  // Thong diep loi cua node (nhat ky debug) — chi khac null khi state === "failed".
  error: z.string().nullable(),
});
const RunDoneEventSchema = z.object({
  type: z.literal("run_done"),
  runId: z.string(),
  status: z.string(),
});
// P-lumina: node API (Veo3/Google/Seedance/Grok/MuseAI...) khong co dong `tasks` rieng nhu
// dola_video — day la nguon DUY NHAT bao duong dan anh/video vua sinh de `OutputPreview` hien
// (xem `crates/lvc-engine/src/dag.rs::emit_preview_if_media`).
const NodeOutputsEventSchema = z.object({
  type: z.literal("node_outputs"),
  runId: z.string(),
  nodeId: z.string(),
  previewPath: z.string(),
});

export const EngineEventSchema = z.discriminatedUnion("type", [
  TaskStageEventSchema,
  TaskDoneEventSchema,
  TaskFailedEventSchema,
  NodeStateEventSchema,
  RunDoneEventSchema,
  NodeOutputsEventSchema,
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
  /** So node trong do thi — the trong kho hien "N node" ma khong phai tai ca do thi. */
  nodeCount: z.number().default(0),
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
  dolaChannel: z.enum(["api", "browser"]),
  // Nhom "Luu tru" (crates/lvc-core/src/settings/storage.rs).
  subdirPattern: z.enum(["none", "workflow", "workflow_date", "date"]),
  filenameTemplate: z.string(),
  duplicatePolicy: z.enum(["number", "overwrite", "skip"]),
  // Nhom "Chay tac vu" (crates/lvc-core/src/settings/run.rs). `restBetween` tinh bang GIAY, chi
  // nhan mot trong `REST_CHOICES` = [0, 30, 60, 180].
  onError: z.enum(["skip", "stop"]),
  restBetween: z.number(),
  missingAccount: z.enum(["skip", "wait"]),
});
export type AppSettings = z.infer<typeof AppSettingsSchema>;
export type DolaChannel = AppSettings["dolaChannel"];
export type SubdirPattern = AppSettings["subdirPattern"];
export type DuplicatePolicy = AppSettings["duplicatePolicy"];
export type OnErrorPolicy = AppSettings["onError"];
export type MissingAccountPolicy = AppSettings["missingAccount"];

/** `commands::cache::CacheInfo` — thong tin thu muc tam cho nhom "Luu tru". */
export const CacheInfoSchema = z.object({
  path: z.string(),
  bytes: z.number(),
  files: z.number(),
  /** Moc quy chieu cua thanh do (10 GB) — backend gui de UI khong tu doan. */
  barFullBytes: z.number(),
  /** So ngay giu file truoc khi tu xoa luc khoi dong. */
  maxAgeDays: z.number(),
});
export type CacheInfo = z.infer<typeof CacheInfoSchema>;

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
// NodeManifest — `fixtures/catalog.json` / lenh `node_catalog` (P4a, crates/lvc-node-api): mo ta
// day du mot loai node (cong kem `many`/`required`/`collect`, nhom `requireAny`, credential, tham
// so mac dinh + JSON Schema cua tham so). Truong camelCase.
// ---------------------------------------------------------------------------------------------

export const ManifestPortSchema = z.object({
  name: z.string(),
  type: PortTypeSchema,
  many: z.boolean(),
  required: z.boolean(),
  collect: z.boolean(),
});
export type ManifestPort = z.infer<typeof ManifestPortSchema>;

export const CredentialSpecSchema = z.object({
  provider: z.string(),
  kinds: z.array(z.string()),
});
export type CredentialSpec = z.infer<typeof CredentialSpecSchema>;

export const NodeManifestSchema = z.object({
  kind: z.string(),
  version: z.number(),
  category: z.string(),
  labelVi: z.string(),
  inputs: z.array(ManifestPortSchema),
  outputs: z.array(ManifestPortSchema),
  requireAny: z.array(z.array(z.string())).optional(),
  credential: CredentialSpecSchema.optional(),
  defaultParams: z.record(z.string(), z.unknown()),
  paramsSchema: z.record(z.string(), z.unknown()),
});
export type NodeManifest = z.infer<typeof NodeManifestSchema>;

export const NodeCatalogSchema = z.array(NodeManifestSchema);
export type NodeCatalog = z.infer<typeof NodeCatalogSchema>;

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
 * nay) LAN fixture MOI (da them), khong ep mot phia phai xong truoc. `scripts`/`scenes` (P3c Task
 * 1, R5) la truong PLAIN `usize` phia Rust (khong `Option`, luon duoc serialize) — fixture
 * `import_reports.json` da tai sinh voi ca hai (`scripts: 2, scenes: 5`), nen KHONG can
 * `.default()` nhu `profilesSkipped`.
 */
export const LegacyImportReportSchema = z.object({
  tasks: z.number(),
  accounts: z.number(),
  usage: z.number(),
  settings: z.number(),
  skipped: z.array(z.string()),
  profilesCopied: z.number(),
  profilesSkipped: z.array(z.string()).default([]),
  scripts: z.number(),
  scenes: z.number(),
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

// ---------------------------------------------------------------------------------------------
// Kich ban (P3c Task 2, `crates/lvc-app/src/commands/scripts.rs` + `crates/lvc-store/src/
// scripts.rs` + `crates/lvc-engine/src/scripts.rs`) — 9 lenh `script_*`. Nullable vs optional
// khop DUNG task-1-report.md "DTO" + 3 fixture `script_meta.json`/`script_doc.json`/
// `script_run_report.json`: `ratio`/`duration`/`model`/`mode`/`logo` la `null` khi canh CHUA dat
// gia tri rieng (dung mac dinh AppSettings luc chay) — KHONG phai chuoi rong.
// ---------------------------------------------------------------------------------------------

/** `lvc_store::scripts::ScriptMeta` — mot kich ban + so dem canh theo trang thai (`running` gop
 * CA `queued` LAN `processing`, khac ban Python tach rieng — xem task-1-report.md "DTO"). */
export const ScriptMetaSchema = z.object({
  id: z.string(),
  name: z.string(),
  dir: z.string(),
  createdAt: z.number(),
  updatedAt: z.number(),
  total: z.number(),
  done: z.number(),
  failed: z.number(),
  running: z.number(),
  /** `kind` node Video dung khi Chay (vd "dola_video"/"veo3_video"/...) — mac dinh "dola_video". */
  videoKind: z.string(),
  /** Chuoi JSON tho (rong = "chua dat") — xem `ScriptRowSchema.videoParams`. */
  videoParams: z.string(),
  /** Nhan phan loai tu do (vd "Affiliate") — rong = chua phan loai. */
  category: z.string(),
});
export type ScriptMeta = z.infer<typeof ScriptMetaSchema>;

/** `lvc_store::scripts::ScriptRow` — ket qua `script_create`/`script_update` (khong kem so dem). */
export const ScriptRowSchema = z.object({
  id: z.string(),
  name: z.string(),
  dir: z.string(),
  createdAt: z.number(),
  updatedAt: z.number(),
  videoKind: z.string(),
  /** Chuoi JSON tho (rong = "chua dat", xem `lvc_engine::scripts::effective_video_params"`). */
  videoParams: z.string(),
  category: z.string(),
});
export type ScriptRow = z.infer<typeof ScriptRowSchema>;

/** `lvc_app::commands::scripts::SceneDto` — mot canh + trang thai task hien tai (`script_get`).
 * `taskStatus` la mot trong `"queued"|"processing"|"completed"|"failed"` hoac `null` (chua tung
 * chay HOAC `taskId` da "mo coi" — task bi xoa o noi khac). `failureCode` la mot trong 15 gia tri
 * `FailureCode` hoac `null`. */
export const SceneDtoSchema = z.object({
  id: z.string(),
  idx: z.number(),
  prompt: z.string(),
  ratio: z.string().nullable(),
  duration: z.number().nullable(),
  model: z.string().nullable(),
  mode: z.string().nullable(),
  logo: z.string().nullable(),
  account: z.string(),
  refs: z.array(z.string()),
  taskId: z.string().nullable(),
  taskStatus: z.string().nullable(),
  failureCode: z.string().nullable(),
  localPath: z.string().nullable(),
});
export type SceneDto = z.infer<typeof SceneDtoSchema>;

/** Ket qua `script_get`. */
export const ScriptDocSchema = z.object({
  meta: ScriptMetaSchema,
  scenes: z.array(SceneDtoSchema),
});
export type ScriptDoc = z.infer<typeof ScriptDocSchema>;

// ---------------------------------------------------------------------------------------------
// Kho tai khoan AI (P4c, `crates/lvc-app/src/commands/credentials.rs` + `crates/lvc-store` +
// `crates/lvc-vault`) — 5 lenh `credential_*`. Fixture nguon su that: `fixtures/credentials.json`
// (`CredentialView[]`) + `fixtures/verify_outcome.json` (`VerifyOutcome`). Toan bo camelCase.
//
// AN TOAN BI MAT: `CredentialView` KHONG BAO GIO chua bi mat nguyen van — chi co `secretHint`
// (chuoi da che san phia Rust) va `proxyMasked` (proxy da che, giong `mask_proxy` cua accounts).
// UI khong duoc "un-mask", chi gui bi mat MOI khi them/sua (write-only, xem `store/credentials.ts`).
// ---------------------------------------------------------------------------------------------

/** Nha cung cap tai khoan AI — khop wire cua backend (P4c contract). */
export const CredentialProviderSchema = z.enum([
  "dola",
  "google",
  "chatgpt_web",
  "openai",
  "gemini",
  "facebook",
  "apple",
  "seedance",
  "grok",
  "museai",
  "elevenlabs",
  "minimax",
  // Token publish workflow len kho cong dong (xem `crates/providers/lvc-github`).
  "github",
]);
export type CredentialProvider = z.infer<typeof CredentialProviderSchema>;

/** Loai bi mat theo provider (dola->session; openai/gemini->api_key; google/chatgpt_web->login|session). */
export const CredentialKindSchema = z.enum(["session", "api_key", "login"]);
export type CredentialKind = z.infer<typeof CredentialKindSchema>;

/** Trang thai kiem tra gan nhat cua mot tai khoan AI. */
export const CredentialStatusSchema = z.enum(["ok", "expired", "banned", "quota", "unknown"]);
export type CredentialStatus = z.infer<typeof CredentialStatusSchema>;

/**
 * `CredentialView` — ban chieu (view) an toan cua mot tai khoan AI. `secretHint` la BIEU DIEN DUY
 * NHAT cua bi mat (da che phia Rust); khong co truong `secret` nao — `tests/ipc_fixtures.test.ts`
 * ghim bat bien nay. `meta` la object tuy provider (vd `base_url` cho openai/gemini,
 * `profile_dir` cho dola). `lastUsedAt`/`lastVerifiedAt` la epoch giay (0 = chua tung).
 */
export const CredentialViewSchema = z.object({
  id: z.string(),
  provider: CredentialProviderSchema,
  kind: CredentialKindSchema,
  label: z.string(),
  secretHint: z.string(),
  meta: z.record(z.string(), z.unknown()),
  proxyMasked: z.string(),
  status: CredentialStatusSchema,
  enabled: z.boolean(),
  lastUsedAt: z.number(),
  lastVerifiedAt: z.number(),
});
export type CredentialView = z.infer<typeof CredentialViewSchema>;

export const CredentialListSchema = z.array(CredentialViewSchema);

/** Ket qua `credential_verify` (`fixtures/verify_outcome.json`). */
export const VerifyOutcomeSchema = z.object({
  status: CredentialStatusSchema,
  detail: z.string(),
});
export type VerifyOutcome = z.infer<typeof VerifyOutcomeSchema>;

/** Ket qua `proxy_test` — IP ra sau proxy, vi tri (quoc gia), do tre (ms). */
export const ProxyProbeSchema = z.object({
  outboundIp: z.string(),
  location: z.string(),
  latencyMs: z.number(),
});
export type ProxyProbe = z.infer<typeof ProxyProbeSchema>;

/** `lvc_store::scripts::SceneRow` — ket qua `script_put_scenes` (khong kem trang thai task, khac
 * `SceneDto`). */
export const SceneRowSchema = z.object({
  id: z.string(),
  scriptId: z.string(),
  idx: z.number(),
  prompt: z.string(),
  ratio: z.string().nullable(),
  duration: z.number().nullable(),
  model: z.string().nullable(),
  mode: z.string().nullable(),
  logo: z.string().nullable(),
  account: z.string(),
  refs: z.array(z.string()),
  taskId: z.string().nullable(),
  updatedAt: z.number(),
});
export type SceneRow = z.infer<typeof SceneRowSchema>;

/**
 * `lvc_store::scripts::SceneIn` — dau vao `script_put_scenes`. Rust chap nhan MOI truong TRU
 * `id`/`prompt` vang mat (`#[serde(default)]`) — UI o day luon gui du CA object (khong dua vao
 * mac dinh phia server), nen schema doi hoi du truong thay vi `.optional()`/`.default()`.
 */
export const SceneInSchema = z.object({
  id: z.string().nullable(),
  prompt: z.string(),
  ratio: z.string().nullable(),
  duration: z.number().nullable(),
  model: z.string().nullable(),
  mode: z.string().nullable(),
  logo: z.string().nullable(),
  account: z.string(),
  refs: z.array(z.string()),
});
export type SceneIn = z.infer<typeof SceneInSchema>;

/** `lvc_engine::scripts::ScriptRunTask` — mot canh da duoc gan `taskId` sau `script_run`. */
export const ScriptRunTaskSchema = z.object({
  sceneId: z.string(),
  taskId: z.string(),
});
export type ScriptRunTask = z.infer<typeof ScriptRunTaskSchema>;

/** `lvc_engine::scripts::ScriptRunError` — `code` la mot trong `SCENE_RUNNING`/`SCENE_EMPTY`/
 * `SCENE_INVALID` (xem `format/scene.ts::scriptErrorLabel`). */
export const ScriptRunErrorSchema = z.object({
  sceneId: z.string(),
  code: z.string(),
});
export type ScriptRunError = z.infer<typeof ScriptRunErrorSchema>;

/** Ket qua `script_run`. `runId: null` = KHONG canh nao hop le de chay (moi canh chon deu roi
 * vao `errors`) — UI khong goi `run_cancel` khi `runId` la `null`. */
export const ScriptRunReportSchema = z.object({
  runId: z.string().nullable(),
  tasks: z.array(ScriptRunTaskSchema),
  errors: z.array(ScriptRunErrorSchema),
});
export type ScriptRunReport = z.infer<typeof ScriptRunReportSchema>;

/** Ket qua `script_reset_scenes` (Rust: `ResetReport`, doi ten sang `ScriptResetReport` o day de
 * ro rang la mot kieu cua mien "kich ban" khi doc doc lap voi `commands/scripts.rs`). */
export const ScriptResetReportSchema = z.object({
  reset: z.array(z.string()),
  skipped: z.array(z.string()),
});
export type ScriptResetReport = z.infer<typeof ScriptResetReportSchema>;

// ---------------------------------------------------------------------------------------------
// Kho template (P4d, `crates/lvc-app/src/commands/templates.rs` + `crates/lvc-store`) — 4 lenh
// `template_*`. Fixture nguon su that: `fixtures/template_list.json` (`TemplateMeta[]`) +
// `fixtures/template_doc.json` (`TemplateDoc`). Toan bo camelCase.
//
// `scope` "public" (id bat dau "public:", CHI DOC — khong luu de/xoa) vs "private" (id "tpl_") vs
// "remote" (id "remote:", P7 — kho CONG DONG tai qua mang, cache offline o tang Rust, CHI DOC).
// `graph` trong `TemplateDoc` dung y het `GraphSchema` (dinh dang file workflow, version 2).
// ---------------------------------------------------------------------------------------------

/** Pham vi template — khop wire cua backend (P4d + P7 contract). */
export const TemplateScopeSchema = z.enum(["public", "private", "remote"]);
export type TemplateScope = z.infer<typeof TemplateScopeSchema>;

/** `TemplateMeta` — sieu du lieu mot mau (khong kem do thi). `thumbnail` co the rong (""). */
export const TemplateMetaSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  scope: TemplateScopeSchema,
  /** Nhom de loc/gom trong gallery. Mau/fixture cu thieu -> "Cơ bản" (tuong thich nguoc). */
  category: z.string().default("Cơ bản"),
  thumbnail: z.string(),
  updatedAt: z.number(),
  /** So node cua do thi — the trong kho hien "N node" ma khong phai tai ca do thi tung mau. */
  nodeCount: z.number().default(0),
  /** CHI muc cong dong co; rong voi mau dung san/rieng. Dung de ghi nguon goc khi luu workflow
   * dung tu mau nay (`workflowSetOrigin`) -> ve sau biet co ban moi hay khong. */
  version: z.string().default(""),
});
export type TemplateMeta = z.infer<typeof TemplateMetaSchema>;

// ---------------------------------------------------------------------------------------------
// Cap nhat workflow tu kho cong dong (`crates/lvc-app/src/commands/updates.rs`).
// ---------------------------------------------------------------------------------------------

export const ChangeLineSchema = z.object({
  /** "Thêm" | "Sửa" | "Bỏ" tren thuc te, nhung backend khong gioi han nen de chuoi tu do. */
  kind: z.string(),
  text: z.string(),
});
export type ChangeLine = z.infer<typeof ChangeLineSchema>;

export const WorkflowUpdateSchema = z.object({
  workflowId: z.string(),
  workflowName: z.string(),
  sourceId: z.string(),
  currentVersion: z.string(),
  newVersion: z.string(),
  author: z.string(),
  thumbnail: z.string(),
  updatedAt: z.number(),
  changelog: z.array(ChangeLineSchema),
  /** `true` = do thi tren may da khac luc tai ve -> phai hoi "Giu ca hai / Ghi de". */
  locallyModified: z.boolean(),
});
export type WorkflowUpdate = z.infer<typeof WorkflowUpdateSchema>;

/** `keep_both` = tao workflow MOI, giu ban cu; `overwrite` = de len ban cu. */
export const UpdateModeSchema = z.enum(["keep_both", "overwrite"]);
export type UpdateMode = z.infer<typeof UpdateModeSchema>;

export const TemplateListSchema = z.array(TemplateMetaSchema);

/** Ket qua `template_get` — meta + do thi day du (`GraphSchema` tai su dung tu tren). */
export const TemplateDocSchema = z.object({
  meta: TemplateMetaSchema,
  graph: GraphSchema,
});
export type TemplateDoc = z.infer<typeof TemplateDocSchema>;

// ---------------------------------------------------------------------------------------------
// Kho kịch bản (`crates/lvc-app/src/commands/script_gallery.rs`) — 3 lệnh `script_gallery_*`.
// Mirror kiến trúc Kho mẫu Ở TRÊN nhưng kèm `scenes` (mảng dòng prompt) thay cho `graph`. Quy ước
// scope/label UI GIỮ Y HỆT Kho mẫu (xem `TemplateScopeSchema` — `scope="public"` hiện chữ
// "Công khai" trong UI, `scope="remote"` hiện chữ "Miễn phí", `scope="private"` hiện "Của tôi").
// ---------------------------------------------------------------------------------------------

export const ScriptGalleryMetaSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  scope: TemplateScopeSchema,
  category: z.string().default("Affiliate"),
  sceneCount: z.number(),
  /** TEN anh bia (khong phai duong dan) — UI ghep `/covers/<cover>.svg`. Rong = chua co bia. */
  cover: z.string().default(""),
  updatedAt: z.number(),
});
export type ScriptGalleryMeta = z.infer<typeof ScriptGalleryMetaSchema>;

// ---------------------------------------------------------------------------------------------
// Hang cho workflow (`crates/lvc-app/src/commands/queue.rs`) — man "Tác vụ". 7 lenh `queue_*`.
// Chay TUAN TU tung workflow; `graph` duoc CHUP lai luc them nen sua workflow goc khong lam doi
// muc dang cho.
// ---------------------------------------------------------------------------------------------

/** Ket qua `publish_workflow` — PR vua mo tren kho cong dong. */
export const PublishResultSchema = z.object({
  pullRequestUrl: z.string(),
  branch: z.string(),
  /** `true` = thay the muc cung id, `false` = them muc moi. */
  updatedExisting: z.boolean(),
});
export type PublishResult = z.infer<typeof PublishResultSchema>;

export const QueueStatusSchema = z.enum(["waiting", "running", "done", "failed", "skipped"]);
export type QueueStatus = z.infer<typeof QueueStatusSchema>;

export const QueueItemSchema = z.object({
  id: z.string(),
  workflowId: z.string(),
  name: z.string(),
  position: z.number(),
  status: QueueStatusSchema,
  runId: z.string().nullable(),
  error: z.string(),
  createdAt: z.number(),
  startedAt: z.number().nullable(),
  finishedAt: z.number().nullable(),
});
export type QueueItem = z.infer<typeof QueueItemSchema>;

export const QueueStateSchema = z.object({
  items: z.array(QueueItemSchema),
  /** Bo chay dang hoat dong (ke ca luc dang nghi giua hai workflow). */
  running: z.boolean(),
});
export type QueueState = z.infer<typeof QueueStateSchema>;

export const ScriptGalleryListSchema = z.array(ScriptGalleryMetaSchema);

export const ScriptGalleryDocSchema = z.object({
  meta: ScriptGalleryMetaSchema,
  scenes: z.array(z.string()),
});
export type ScriptGalleryDoc = z.infer<typeof ScriptGalleryDocSchema>;
