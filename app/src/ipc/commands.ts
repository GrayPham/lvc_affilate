// Mot ham boc typed cho MOI lenh Tauri cua `crates/lvc-app` (24 lenh, xem
// `commands::mod.rs`/`lib.rs::invoke_handler!`) — ten lenh dung HET SUC dung chuoi Rust
// (snake_case), tham so JS dung camelCase (Tauri v2 tu doi snake_case -> camelCase cho tham so
// lenh, xac nhan trong task-9-report.md). Ket qua luon di qua `.parse()` zod (khong `safeParse`)
// — lech hop dong phai nem loi ro rang NGAY tai diem goi, khong am tham lot qua kieu `any`.
import { invoke } from "./tauri";
import {
  AccountStatusSchema,
  AppSettingsSchema,
  CacheInfoSchema,
  ScriptDocSchema,
  ScriptMetaSchema,
  ScriptResetReportSchema,
  ScriptRowSchema,
  ScriptRunReportSchema,
  SceneRowSchema,
  WorkflowUpdateSchema,
  CredentialListSchema,
  CredentialViewSchema,
  ProxyProbeSchema,
  VerifyOutcomeSchema,
  ImportCookiesReportSchema,
  LegacyImportReportSchema,
  NodeCatalogSchema,
  NodeStateDtoSchema,
  PlanSummarySchema,
  PublishResultSchema,
  QueueItemSchema,
  QueueStateSchema,
  RunInfoSchema,
  RunRowSchema,
  TaskDtoSchema,
  TaskResumeInfoSchema,
  TemplateDocSchema,
  TemplateListSchema,
  TemplateMetaSchema,
  type TemplateScope,
  VerifyReportSchema,
  WorkflowDocSchema,
  WorkflowMetaSchema,
  IpcErrorSchema,
  type AccountStatus,
  type AppSettings,
  type BrowserEngineId,
  type CacheInfo,
  type SceneIn,
  type SceneRow,
  type ScriptDoc,
  type ScriptMeta,
  type ScriptResetReport,
  type ScriptRow,
  type ScriptRunReport,
  type UpdateMode,
  type WorkflowUpdate,
  type CredentialView,
  type Graph,
  type ImportCookiesReport,
  type IpcError,
  type LegacyImportReport,
  type NodeCatalog,
  type NodeStateDto,
  type PlanSummary,
  type ProxyProbe,
  type PublishResult,
  type QueueItem,
  type QueueState,
  type RunInfo,
  type RunRow,
  type TaskDto,
  type TaskResumeInfo,
  type TemplateDoc,
  type TemplateMeta,
  type VerifyOutcome,
  type VerifyReport,
  type WorkflowDoc,
  type WorkflowMeta,
} from "./types";
import { z } from "zod";
import { migrateGraphJson } from "../graph/adapter";

/** Type guard cho loi tra ve tu bat ky lenh IPC nao (`invoke()` reject voi gia tri nay). */
export function isIpcError(e: unknown): e is IpcError {
  return IpcErrorSchema.safeParse(e).success;
}

// ---- workflow.rs -------------------------------------------------------------------------

export async function workflowValidate(graph: Graph): Promise<PlanSummary> {
  const res = await invoke<unknown>("workflow_validate", { graph });
  return PlanSummarySchema.parse(res);
}

export async function workflowSave(
  id: string | null,
  name: string,
  graph: Graph,
): Promise<WorkflowMeta> {
  const res = await invoke<unknown>("workflow_save", { id, name, graph });
  return WorkflowMetaSchema.parse(res);
}

export async function workflowList(): Promise<WorkflowMeta[]> {
  const res = await invoke<unknown>("workflow_list");
  return z.array(WorkflowMetaSchema).parse(res);
}

export async function workflowGet(id: string): Promise<WorkflowDoc> {
  const res = await invoke<unknown>("workflow_get", { id });
  // P4a: migrate do thi cu (kind `video` -> `dola_video`, bo kind P4 da xoa) TRUOC zod — phong
  // khi backend tra ve do thi v1 chua migrate.
  if (res !== null && typeof res === "object" && "graph" in res) {
    return WorkflowDocSchema.parse({ ...res, graph: migrateGraphJson(res.graph) });
  }
  return WorkflowDocSchema.parse(res);
}

export async function workflowDelete(id: string): Promise<boolean> {
  const res = await invoke<unknown>("workflow_delete", { id });
  return z.boolean().parse(res);
}

export async function runStart(workflowId: string, graph: Graph): Promise<RunInfo> {
  const res = await invoke<unknown>("run_start", { workflowId, graph });
  return RunInfoSchema.parse(res);
}

export async function runCancel(runId: string): Promise<boolean> {
  const res = await invoke<unknown>("run_cancel", { runId });
  return z.boolean().parse(res);
}

export async function runList(limit: number): Promise<RunRow[]> {
  const res = await invoke<unknown>("run_list", { limit });
  return z.array(RunRowSchema).parse(res);
}

// Run-robustness B: trang thai muc node (khoi phuc `nodes` khi mo lai app).
export async function runNodeStates(runId: string): Promise<NodeStateDto[]> {
  const res = await invoke<unknown>("run_node_states", { runId });
  return z.array(NodeStateDtoSchema).parse(res);
}

// Run-robustness B: map node_id -> duong dan output (khoi phuc preview `nodeVideos`).
export async function runNodeOutputs(runId: string): Promise<Record<string, string>> {
  const res = await invoke<unknown>("run_node_outputs", { runId });
  return z.record(z.string(), z.string()).parse(res);
}

// Run-robustness C: chay lai CHI node loi + node phia sau (tai dung output node da xong) -> run moi.
export async function runRetryFailed(runId: string, graph: Graph): Promise<RunInfo> {
  const res = await invoke<unknown>("run_retry_failed", { runId, graph });
  return RunInfoSchema.parse(res);
}

// ---- tasks.rs -----------------------------------------------------------------------------

export async function taskList(runId: string | null, limit: number): Promise<TaskDto[]> {
  const res = await invoke<unknown>("task_list", { runId, limit });
  return z.array(TaskDtoSchema).parse(res);
}

export async function taskCancel(id: string): Promise<boolean> {
  const res = await invoke<unknown>("task_cancel", { id });
  return z.boolean().parse(res);
}

export async function taskResume(id: string): Promise<TaskResumeInfo> {
  const res = await invoke<unknown>("task_resume", { id });
  return TaskResumeInfoSchema.parse(res);
}

export async function taskOpenFile(id: string): Promise<null> {
  const res = await invoke<unknown>("task_open_file", { id });
  return z.null().parse(res);
}

// ---- nodes (P4a) -------------------------------------------------------------------------

/** Danh muc day du cac loai node (manifest) — lenh `node_catalog`, xem `fixtures/catalog.json`. */
export async function nodeCatalog(): Promise<NodeCatalog> {
  const res = await invoke<unknown>("node_catalog");
  return NodeCatalogSchema.parse(res);
}

// ---- settings.rs --------------------------------------------------------------------------

export async function settingsGet(): Promise<AppSettings> {
  const res = await invoke<unknown>("settings_get");
  return AppSettingsSchema.parse(res);
}

/** Tra ve danh sach canh bao chuan hoa (rong = khong co gi phai sua). */
export async function settingsSet(s: AppSettings): Promise<string[]> {
  const res = await invoke<unknown>("settings_set", { s });
  return z.array(z.string()).parse(res);
}

/** `null` = nguoi dung bam Huy trong hop thoai. */
export async function pickDirectory(): Promise<string | null> {
  const res = await invoke<unknown>("pick_directory");
  return z.string().nullable().parse(res);
}

/** `images = true` loc dinh dang anh (png/jpg/jpeg/webp); huy hop thoai -> mang rong. */
export async function pickFiles(images: boolean): Promise<string[]> {
  const res = await invoke<unknown>("pick_files", { images });
  return z.array(z.string()).parse(res);
}

// ---- scripts.rs (P3c Task 1/2) --------------------------------------------------------------
// Xem task-1-report.md "Lenh IPC cho Task 2" cho bang doi chieu day du (tham so/ma loi). Tham so
// Rust snake_case (`scene_ids`) -> JS camelCase (`sceneIds`) tu dong qua Tauri 2, giong moi lenh
// khac trong file nay.

export async function scriptList(): Promise<ScriptMeta[]> {
  const res = await invoke<unknown>("script_list");
  return z.array(ScriptMetaSchema).parse(res);
}

export async function scriptCreate(name: string, dir: string): Promise<ScriptRow> {
  const res = await invoke<unknown>("script_create", { name, dir });
  return ScriptRowSchema.parse(res);
}

/** `name`/`dir` luon duoc gui (khong `undefined`) — UI o day luon biet ca hai gia tri hien tai
 * khi sua (khong co UI sua "chi mot truong, giu nguyen truong kia" nhu `Option<String>` phia
 * Rust cho phep). */
export async function scriptUpdate(id: string, name: string, dir: string): Promise<ScriptRow> {
  const res = await invoke<unknown>("script_update", { id, name, dir });
  return ScriptRowSchema.parse(res);
}

/** Lenh Rust tra `()` (unit) -> `null` qua JSON. */
export async function scriptDelete(id: string): Promise<null> {
  const res = await invoke<unknown>("script_delete", { id });
  return z.null().parse(res);
}

export async function scriptGet(id: string): Promise<ScriptDoc> {
  const res = await invoke<unknown>("script_get", { id });
  return ScriptDocSchema.parse(res);
}

export async function scriptPutScenes(id: string, scenes: SceneIn[]): Promise<SceneRow[]> {
  const res = await invoke<unknown>("script_put_scenes", { id, scenes });
  return z.array(SceneRowSchema).parse(res);
}

/** `sceneIds` rong = moi canh chua chay/da that bai (R1); `style` rong = khong noi "Style: ...".
 * Loi TUNG canh (`SCENE_RUNNING`/`SCENE_EMPTY`/`SCENE_INVALID`) nam trong `ScriptRunReport.errors`
 * — KHONG lam `invoke()` nay reject. */
export async function scriptRun(
  id: string,
  sceneIds: string[],
  style: string,
): Promise<ScriptRunReport> {
  const res = await invoke<unknown>("script_run", { id, sceneIds, style });
  return ScriptRunReportSchema.parse(res);
}

export async function scriptResetScenes(id: string, sceneIds: string[]): Promise<ScriptResetReport> {
  const res = await invoke<unknown>("script_reset_scenes", { id, sceneIds });
  return ScriptResetReportSchema.parse(res);
}

/** Thay TOAN BO canh cua kich ban bang noi dung TXT (R4) — tra ve so canh moi. */
export async function scriptImportTxt(id: string, text: string): Promise<number> {
  const res = await invoke<unknown>("script_import_txt", { id, text });
  return z.number().parse(res);
}

// ---- updates.rs ---------------------------------------------------------------------------

/** Cac ban cap nhat dang cho. Mang rong = khong co gi moi. KHONG goi mang: chi doc cache kho
 * cong dong ma tab "Miễn phí" da nap. */
export async function workflowUpdateList(): Promise<WorkflowUpdate[]> {
  const res = await invoke<unknown>("workflow_update_list");
  return z.array(WorkflowUpdateSchema).parse(res);
}

/** Ghi nho workflow nay dung tu mau cong dong nao, o phien ban nao — goi NGAY SAU khi luu
 * workflow thanh cong, neu do thi tren canvas den tu mot mau cong dong co `version`. */
export async function workflowSetOrigin(args: {
  workflowId: string;
  sourceId: string;
  sourceVersion: string;
  graph: Graph;
}): Promise<void> {
  await invoke<unknown>("workflow_set_origin", args);
}

/** Ap ban cap nhat -> tra ID workflow ket qua (`overwrite` tra id cu, `keep_both` tra id MOI). */
export async function workflowApplyUpdate(
  workflowId: string,
  mode: UpdateMode,
): Promise<string> {
  const res = await invoke<unknown>("workflow_apply_update", { workflowId, mode });
  return z.string().parse(res);
}

// ---- device.rs ---------------------------------------------------------------------------

/** Ma thiet bi — on dinh cho MOT lan cai dat tren MOT may. Sinh lan dau roi giu nguyen.
 * Dung de nguoi dung gui kem khi bao loi; KHONG phai bi mat. */
export async function deviceId(): Promise<string> {
  const res = await invoke<unknown>("device_id");
  return z.string().parse(res);
}

// ---- cache.rs -----------------------------------------------------------------------------

/** Thu muc tam dang chiem bao nhieu (nhom "Luu tru" tren man Cai dat). */
export async function cacheInfo(): Promise<CacheInfo> {
  const res = await invoke<unknown>("cache_info");
  return CacheInfoSchema.parse(res);
}

/** Don thu muc tam -> so BYTE da giai phong. Khong cham vao thu muc ket qua (`downloads`). */
export async function cacheClear(): Promise<number> {
  const res = await invoke<unknown>("cache_clear");
  return z.number().parse(res);
}

/** Mo thu muc bang trinh quan ly tep cua he dieu hanh. Chi hai dich den KIN — lenh nay KHONG
 * nhan duong dan tu do, de khong tro thanh cach mo bat ky thu gi tren may. */
export async function openAppFolder(which: "output" | "cache"): Promise<void> {
  await invoke<unknown>("open_app_folder", { which });
}

/** Luu MOT khung hinh (PNG base64/dataURL, UI chup tu <video>) thanh file that -> tra duong dan. */
export async function frameSavePng(data: string, nameHint?: string): Promise<string> {
  const res = await invoke<unknown>("frame_save_png", { data, nameHint: nameHint ?? null });
  return z.string().parse(res);
}

/** Cat video `path` theo [start, end] giay -> file moi (ffmpeg), tra duong dan. end<=start = toi het. */
export async function trimVideo(
  path: string,
  start: number,
  end: number,
  nameHint?: string,
): Promise<string> {
  const res = await invoke<unknown>("trim_video", { path, start, end, nameHint: nameHint ?? null });
  return z.string().parse(res);
}

// ---- accounts.rs (Task 10) -----------------------------------------------------------------
// Xem task-10-report.md "Exact command list (Task 14 mirrors this)" cho bang doi chieu day du
// ten lenh/tham so/ma loi. `proxy` trong `AccountStatus` tra ve tu `accountList` DA duoc Rust
// che (`mask_proxy`) — khong bao gio hien nguyen van, chi gui gia tri MOI khi luu.

export async function accountList(): Promise<AccountStatus[]> {
  const res = await invoke<unknown>("account_list");
  return z.array(AccountStatusSchema).parse(res);
}

export async function accountImportCookies(
  name: string,
  cookieText: string,
  proxy: string,
): Promise<ImportCookiesReport> {
  const res = await invoke<unknown>("account_import_cookies", { name, cookieText, proxy });
  return ImportCookiesReportSchema.parse(res);
}

/** Ket qua nap cookie MOT file trong lo (account_import_cookie_files). */
export const BulkCookieItemSchema = z.object({
  name: z.string(),
  ok: z.boolean(),
  error: z.string().nullable(),
});
export type BulkCookieItem = z.infer<typeof BulkCookieItemSchema>;

/** Nap cookie NHIEU FILE cung luc — moi file 1 tai khoan (ten = ten file); mot file loi khong dung lo. */
export async function accountImportCookieFiles(
  paths: string[],
  proxy: string,
): Promise<BulkCookieItem[]> {
  const res = await invoke<unknown>("account_import_cookie_files", { paths, proxy });
  return z.array(BulkCookieItemSchema).parse(res);
}

/** (A) Nap cookie NHIEU FILE vao kho credentials theo NEN TANG (provider), kind session. */
export const LoginImportItemSchema = z.object({
  label: z.string(),
  ok: z.boolean(),
  error: z.string().nullable(),
});
export type LoginImportItem = z.infer<typeof LoginImportItemSchema>;

export async function loginImportCookieFiles(
  provider: string,
  paths: string[],
  proxy: string,
): Promise<LoginImportItem[]> {
  const res = await invoke<unknown>("login_import_cookie_files", { provider, paths, proxy });
  return z.array(LoginImportItemSchema).parse(res);
}

/** Dan HANG LOAT: moi dong mot tai khoan `email|...|cookies` (tu tach cookie + nhan). Moi dong tiem
 * vao profile rieng + luu credential. Mot dong loi KHONG dung ca lo. */
export async function loginCookieTextBulk(
  provider: string,
  text: string,
  proxy: string,
  engine: string,
): Promise<LoginImportItem[]> {
  const res = await invoke<unknown>("login_cookie_text_bulk", { provider, text, proxy, engine });
  return z.array(LoginImportItemSchema).parse(res);
}

/** (A) Dan MOT doan cookie (khong qua file) cho nen tang bat ky KHAC Dola — tiem vao profile roi
 * luu credential kind `session`. `engine`: "chromium" (CDP, mac dinh) hoac "firefox_portable"
 * (ghi cookies.sqlite, chi dung tay — xem BrowserEngine). */
export async function loginCookieText(
  provider: string,
  label: string,
  cookieText: string,
  proxy: string,
  engine: BrowserEngineId = "chromium",
): Promise<CredentialView> {
  const res = await invoke<unknown>("login_cookie_text", {
    provider,
    label,
    cookieText,
    proxy,
    engine,
  });
  return CredentialViewSchema.parse(res);
}

/** (B) Ket qua mo dang nhap thu cong (login_open_interactive). */
export const InteractiveLoginResultSchema = z.object({
  label: z.string(),
  provider: z.string(),
  profileDir: z.string(),
  credentialId: z.string(),
});
export type InteractiveLoginResult = z.infer<typeof InteractiveLoginResultSchema>;

/** (B) Mo cua so trinh duyet THAT toi trang dang nhap cho NGUOI DUNG tu dang nhap, roi luu
 * credential `session` tham chieu profile ben vung. Duong HOP LE — khong tu dien thong tin. */
export async function loginOpenInteractive(
  platform: string,
  label: string,
  proxy: string,
): Promise<InteractiveLoginResult> {
  const res = await invoke<unknown>("login_open_interactive", { platform, label, proxy });
  return InteractiveLoginResultSchema.parse(res);
}

export async function accountVerify(name: string): Promise<VerifyReport> {
  const res = await invoke<unknown>("account_verify", { name });
  return VerifyReportSchema.parse(res);
}

export async function accountSetScheduling(name: string, on: boolean): Promise<null> {
  const res = await invoke<unknown>("account_set_scheduling", { name, on });
  return z.null().parse(res);
}

export async function accountSetProxy(name: string, proxy: string): Promise<null> {
  const res = await invoke<unknown>("account_set_proxy", { name, proxy });
  return z.null().parse(res);
}

/**
 * Kiem tra proxy (nut o hop thoai Proxy & trinh duyet): noi thu qua `proxy` toi dich vu "IP cua
 * ban" va tra IP ra + vi tri + do tre. `""`/`"direct"` = khong proxy -> do IP may. Khong ton credit.
 */
export async function proxyTest(proxy: string): Promise<ProxyProbe> {
  const res = await invoke<unknown>("proxy_test", { proxy });
  return ProxyProbeSchema.parse(res);
}

/** `false` = khong co gi de xoa (idempotent — xem `account_delete_impl`). */
export async function accountDelete(name: string): Promise<boolean> {
  const res = await invoke<unknown>("account_delete", { name });
  return z.boolean().parse(res);
}

/** "Mo trinh duyet" (Dola) — mo cua so Chrome HIEN tren dung profile de nguoi dung tu xem/thao tac. */
export async function accountOpenViewer(name: string): Promise<null> {
  const res = await invoke<unknown>("account_open_viewer", { name });
  return z.null().parse(res);
}

/** Nhap du lieu tu ban Python cu (thu muc goc chua `*.db`/`accounts/`) — dung boi man Cai dat. */
export async function legacyImport(pythonDir: string): Promise<LegacyImportReport> {
  const res = await invoke<unknown>("legacy_import", { pythonDir });
  return LegacyImportReportSchema.parse(res);
}

// ---- publish.rs (publish workflow len kho cong dong GitHub) -------------------------------
// Token GitHub do NGUOI DUNG tu tao, luu ma hoa o tab API key (`provider = "github"`); lenh nay
// chi nhan ID credential, KHONG BAO GIO nhan/tra plaintext token.

export async function publishWorkflow(
  credentialId: string,
  form: {
    id: string;
    name: string;
    description: string;
    category: string;
    cover: string;
    version: string;
    author: string;
    /** Moi DONG mot thay doi, dang `"Thêm: ..."`. Rong = khong ghi gi. Rust tu tach
     * (`commands::publish::parse_changelog`). */
    changelog: string;
    graph: Graph;
  },
): Promise<PublishResult> {
  const res = await invoke<unknown>("publish_workflow", { credentialId, form });
  return PublishResultSchema.parse(res);
}

// ---- queue.rs (hang cho workflow, man "Tác vụ") -------------------------------------------

export async function queueList(): Promise<QueueState> {
  const res = await invoke<unknown>("queue_list");
  return QueueStateSchema.parse(res);
}

/** Them workflow vao CUOI hang cho. Do thi hong -> backend tu choi ngay (GRAPH_INVALID). */
export async function queueAdd(workflowId: string, name: string, graph: Graph): Promise<QueueItem> {
  const res = await invoke<unknown>("queue_add", { workflowId, name, graph });
  return QueueItemSchema.parse(res);
}

/** `false` = khong co gi de xoa (idempotent). */
export async function queueRemove(id: string): Promise<boolean> {
  const res = await invoke<unknown>("queue_remove", { id });
  return z.boolean().parse(res);
}

/** Sap lai toan bo thu tu chay theo danh sach id (keo tha). */
export async function queueReorder(ids: string[]): Promise<null> {
  const res = await invoke<unknown>("queue_reorder", { ids });
  return z.null().parse(res);
}

/** Xoa moi muc da ket thuc; tra so dong da xoa. */
export async function queueClearFinished(): Promise<number> {
  const res = await invoke<unknown>("queue_clear_finished");
  return z.number().parse(res);
}

/** `false` = bo chay von DA chay (khong tao vong thu hai). */
export async function queueStart(): Promise<boolean> {
  const res = await invoke<unknown>("queue_start");
  return z.boolean().parse(res);
}

/** `false` = von khong chay. Workflow DANG chay van chay not. */
export async function queueStop(): Promise<boolean> {
  const res = await invoke<unknown>("queue_stop");
  return z.boolean().parse(res);
}

// ---- credentials.rs (P4c kho tai khoan AI) --------------------------------------------------
// 5 lenh `credential_*`. Tham so Rust snake_case -> JS camelCase tu dong (giong moi lenh khac).
// `secret` la tham so GHI-MOT-CHIEU: chi gui khi them/sua, KHONG BAO GIO tra ve — ket qua chi co
// `secretHint` da che. Ma loi rieng: CRED_BAD_PROVIDER/CRED_BAD_KIND/CRED_BAD_LABEL/
// CRED_EMPTY_SECRET/CRED_NOT_FOUND/CRED_CRYPTO/STORE (xem `format/credential.ts`).

/** `provider` rong = lay TAT CA; nguoc lai loc theo mot provider. */
export async function credentialList(provider?: string): Promise<CredentialView[]> {
  const res = await invoke<unknown>("credential_list", { provider: provider ?? null });
  return CredentialListSchema.parse(res);
}

/** `secret` ghi-mot-chieu; `meta` la object tuy provider (vd `{ base_url }`); `proxy` chuoi (co the rong). */
export async function credentialAdd(
  provider: string,
  kind: string,
  label: string,
  secret: string,
  meta?: Record<string, unknown>,
  proxy?: string,
): Promise<CredentialView> {
  const res = await invoke<unknown>("credential_add", {
    provider,
    kind,
    label,
    secret,
    meta: meta ?? null,
    proxy: proxy ?? null,
  });
  return CredentialViewSchema.parse(res);
}

/** Moi truong `undefined` = KHONG doi truong do (chi gui gia tri co mat). `secret` ghi-mot-chieu. */
export async function credentialUpdate(
  id: string,
  secret?: string,
  label?: string,
  proxy?: string,
  enabled?: boolean,
): Promise<null> {
  const res = await invoke<unknown>("credential_update", {
    id,
    secret: secret ?? null,
    label: label ?? null,
    proxy: proxy ?? null,
    enabled: enabled ?? null,
  });
  return z.null().parse(res);
}

export async function credentialVerify(id: string): Promise<VerifyOutcome> {
  const res = await invoke<unknown>("credential_verify", { id });
  return VerifyOutcomeSchema.parse(res);
}

/** `false` = khong co gi de xoa (idempotent). */
export async function credentialDelete(id: string): Promise<boolean> {
  const res = await invoke<unknown>("credential_delete", { id });
  return z.boolean().parse(res);
}

/** "Mo trinh duyet" (credential khac Dola) — loi `PROFILE_NOT_FOUND` neu credential nay chua co
 * profile that (chua tiem cookie thanh cong qua `provision_profile`). */
export async function credentialOpenViewer(id: string): Promise<null> {
  const res = await invoke<unknown>("credential_open_viewer", { id });
  return z.null().parse(res);
}

/** Dat tag loai node cho credential (`video`/`image`/`script`) — node AI chi lay dung tag nay. */
export async function credentialSetNodeTypes(id: string, nodeTypes: string[]): Promise<null> {
  const res = await invoke<unknown>("credential_set_node_types", { id, nodeTypes });
  return z.null().parse(res);
}

/** Dat nguong "han muc/ngay" cho mot api_key (`null`/`0` = khong gioi han). */
export async function credentialSetDailyLimit(id: string, limit: number | null): Promise<null> {
  const res = await invoke<unknown>("credential_set_daily_limit", { id, limit });
  return z.null().parse(res);
}

// ---- templates.rs (P4d kho template + P7 kho cong dong) --------------------------------------
// 4 lenh `template_*`. Tham so Rust snake_case -> JS camelCase tu dong (giong moi lenh khac).
// Mau Public/Remote (`id` bat dau "public:"/"remote:") CHI DOC — luu de/xoa se bi backend tu choi
// TEMPLATE_READONLY. Ma loi rieng: TEMPLATE_NOT_FOUND/TEMPLATE_CORRUPT/TEMPLATE_READONLY/
// GRAPH_ENCODE/STORE/TEMPLATE_REMOTE_UNAVAILABLE (xem `format/template.ts`).

/** `scope` khong dat = lay public + private (KHONG remote — remote can goi mang, chi tai khi UI
 * yeu cau dich danh `scope="remote"`); dat thi loc dung mot pham vi. */
export async function templateList(scope?: TemplateScope): Promise<TemplateMeta[]> {
  const res = await invoke<unknown>("template_list", { scope: scope ?? null });
  return TemplateListSchema.parse(res);
}

export async function templateGet(id: string): Promise<TemplateDoc> {
  const res = await invoke<unknown>("template_get", { id });
  // Migrate do thi cu (kind `video` -> `dola_video`, bo kind P4 da xoa) TRUOC zod — giong
  // `workflowGet`, phong khi mau luu tu ban cu con do thi v1 chua migrate.
  if (res !== null && typeof res === "object" && "graph" in res) {
    return TemplateDocSchema.parse({ ...res, graph: migrateGraphJson(res.graph) });
  }
  return TemplateDocSchema.parse(res);
}

/** `id` null = tao mau moi; nguoc lai luu de mau private do. `description`/`thumbnail` null = rong. */
export async function templateSavePrivate(
  id: string | null,
  name: string,
  graph: Graph,
  description?: string,
  thumbnail?: string,
): Promise<TemplateMeta> {
  const res = await invoke<unknown>("template_save_private", {
    id,
    name,
    description: description ?? null,
    graph,
    thumbnail: thumbnail ?? null,
  });
  return TemplateMetaSchema.parse(res);
}

/** `false` = khong co gi de xoa (idempotent). Chi xoa duoc mau private. */
export async function templateDelete(id: string): Promise<boolean> {
  const res = await invoke<unknown>("template_delete_private", { id });
  return z.boolean().parse(res);
}
