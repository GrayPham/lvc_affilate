// Mot dong trong bang canh (P3c Task 2, `SceneTable`) — prompt (textarea 2 dong) + 4 tham so
// GHI DE (rong = dung mac dinh AppSettings luc chay, xem `SceneDto`) + tai khoan (rong = tu chon)
// + trang thai + hanh dong Chay/Chay lai/Dung/Mo thu muc + Xoa dong. Component THUAN hien thi —
// nhan CA `draft` (dang sua) LAN `docScene` (su that server, `undefined` cho dong MOI chua luu)
// tu `SceneTable`, khong tu doc store.
import { ModeSchema, ModelSchema, RatioSchema, type Mode, type Model, type SceneDto } from "../ipc/types";
import { confirmDialog } from "../store/dialog";
import { allowedDurations } from "../graph/rules";
import { canEditScene, canRerun, canRun, canStop, sceneStatusLabel } from "../format/scene";
import type { DraftScene } from "../store/scripts";

/** Sieu tap thoi luong CO THE co (hop cua `allowedDurations` qua moi mode/model) — dung khi canh
 * CHUA dat rieng mode/model (se duoc resolve tu `AppSettings` luc chay, khong biet truoc o day
 * — chi mirror UX, server van la nguon su that cuoi cung, cung quy uoc voi `graph/rules.ts`). */
const ALL_DURATIONS: readonly number[] = [10, 15, 30];

const MODE_VALUES: readonly string[] = ModeSchema.options;
const MODEL_VALUES: readonly string[] = ModelSchema.options;

function durationsFor(mode: string | null, model: string | null): readonly number[] {
  if (mode !== null && model !== null && MODE_VALUES.includes(mode) && MODEL_VALUES.includes(model)) {
    return allowedDurations(mode as Mode, model as Model);
  }
  return ALL_DURATIONS;
}

const BTN = "rounded border px-1.5 py-0.5 text-[11px] disabled:opacity-50";
const BTN_NEUTRAL = `${BTN} border-line2 hover:bg-fill2`;
const BTN_DANGER = `${BTN} border-red-300 text-red-700 hover:bg-red-50`;

interface SceneRowProps {
  index: number;
  draft: DraftScene;
  /** `undefined` = dong MOI THEM, chua tung luu len server (khong co trang thai task nao ca). */
  docScene: SceneDto | undefined;
  /** `true` khi dang co lenh Dung (`task_cancel`) cho DUNG canh nay dang cho. */
  pendingStop: boolean;
  /** `true` khi dang co lenh Mo thu muc cho DUNG canh nay dang cho. */
  pendingOpen: boolean;
  /** `true` khi mot hanh dong TOAN kich ban (Luu/Chay/Dat lai...) dang cho — khoa Chay/Chay
   * lai/Xoa dong de tranh nguoi dung bam chong. */
  disabled: boolean;
  /** Danh cho "Đặt lại trạng thái" (chon canh muc tieu) — chi co y nghia khi `docScene` da co
   * (canh MOI chua luu khong the "dat lai" gi ca). */
  selected: boolean;
  onToggleSelect: () => void;
  onPromptChange: (prompt: string) => void;
  onFieldChange: (patch: Partial<Omit<DraftScene, "key" | "id" | "prompt">>) => void;
  onRun: () => void;
  onStop: () => void;
  onOpenFile: () => void;
  onRemove: () => void;
}

export default function SceneRow({
  index,
  draft,
  docScene,
  pendingStop,
  pendingOpen,
  disabled,
  selected,
  onToggleSelect,
  onPromptChange,
  onFieldChange,
  onRun,
  onStop,
  onOpenFile,
  onRemove,
}: SceneRowProps) {
  const status = docScene ? sceneStatusLabel(docScene) : "Chưa chạy";
  const showRun = !docScene || canRun(docScene);
  const showRerun = docScene ? canRerun(docScene) : false;
  const showStop = docScene ? canStop(docScene) : false;
  // Fix-round 1 (review C1 — Critical): canh dang `queued`/`processing` KHOA moi o nhap (prompt/
  // ti le/thoi luong/model/mode/tai khoan) VA o chon "Đặt lại trạng thái" (M2) — giong het dieu
  // kien da khoa "Xoá dòng" tu truoc (`showStop`), qua ham thuan da test rieng `canEditScene`.
  // Neu khong khoa, nguoi dung sua duoc prompt cua mot canh dang chay, bam "Lưu cảnh" -> Rust
  // (`replace_scenes_rows`, dung R2) AM THAM giu NGUYEN noi dung server (chi doi `idx`) roi tra ve
  // "thanh cong" — edit mat khong bao loi (xem them guard `saveScenes()` o `store/scripts.ts`,
  // "belt and braces" cho dung truong hop dua — canh CHUYEN sang chay NGAY SAU khi nguoi dung bat
  // dau sua, truoc khi khoa nay kip ap dung).
  const fieldsDisabled = disabled || !canEditScene(docScene);
  const runningTitle = showStop ? "Cảnh đang chạy — dừng cảnh trước khi sửa" : undefined;

  function handleRunClick(): void {
    onRun();
  }

  async function handleRerunClick(): Promise<void> {
    if (
      await confirmDialog({
        title: "Chạy lại cảnh",
        message: "Chạy lại cảnh này sẽ tạo video mới và tốn credit. Tiếp tục?",
        okText: "Chạy lại",
      })
    ) {
      onRun();
    }
  }

  return (
    <tr className="border-b border-line align-top text-xs">
      <td className="px-2 py-1 text-center text-muted">
        <div className="flex flex-col items-center gap-0.5">
          {docScene ? (
            <input
              type="checkbox"
              checked={selected}
              disabled={showStop}
              title={runningTitle}
              onChange={onToggleSelect}
              aria-label={`Chọn cảnh ${index} cho "Đặt lại trạng thái"`}
            />
          ) : null}
          <span>{index}</span>
        </div>
      </td>
      <td className="min-w-[16rem] px-2 py-1">
        <textarea
          className="w-full resize-none rounded border border-line2 px-1.5 py-1 text-xs disabled:opacity-50"
          rows={2}
          value={draft.prompt}
          disabled={fieldsDisabled}
          title={runningTitle}
          onChange={(e) => onPromptChange(e.target.value)}
          placeholder="Nội dung cảnh…"
        />
      </td>
      <td className="px-2 py-1">
        <select
          className="rounded border border-line2 px-1 py-0.5 text-xs disabled:opacity-50"
          value={draft.ratio ?? ""}
          disabled={fieldsDisabled}
          title={runningTitle}
          onChange={(e) => onFieldChange({ ratio: e.target.value === "" ? null : e.target.value })}
          aria-label={`Tỉ lệ cảnh ${index}`}
        >
          <option value="">(mặc định)</option>
          {RatioSchema.options.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
      </td>
      <td className="px-2 py-1">
        <select
          className="rounded border border-line2 px-1 py-0.5 text-xs disabled:opacity-50"
          value={draft.duration ?? ""}
          disabled={fieldsDisabled}
          title={runningTitle}
          onChange={(e) =>
            onFieldChange({ duration: e.target.value === "" ? null : Number(e.target.value) })
          }
          aria-label={`Thời lượng cảnh ${index}`}
        >
          <option value="">(mặc định)</option>
          {durationsFor(draft.mode, draft.model).map((d) => (
            <option key={d} value={d}>
              {d}s
            </option>
          ))}
        </select>
      </td>
      <td className="px-2 py-1">
        <select
          className="rounded border border-line2 px-1 py-0.5 text-xs disabled:opacity-50"
          value={draft.model ?? ""}
          disabled={fieldsDisabled}
          title={runningTitle}
          onChange={(e) => onFieldChange({ model: e.target.value === "" ? null : e.target.value })}
          aria-label={`Model cảnh ${index}`}
        >
          <option value="">(mặc định)</option>
          {ModelSchema.options.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </td>
      <td className="px-2 py-1">
        <select
          className="rounded border border-line2 px-1 py-0.5 text-xs disabled:opacity-50"
          value={draft.mode ?? ""}
          disabled={fieldsDisabled}
          title={runningTitle}
          onChange={(e) => onFieldChange({ mode: e.target.value === "" ? null : e.target.value })}
          aria-label={`Mode cảnh ${index}`}
        >
          <option value="">(mặc định)</option>
          {ModeSchema.options.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </td>
      <td className="px-2 py-1">
        <input
          type="text"
          className="w-24 rounded border border-line2 px-1 py-0.5 text-xs disabled:opacity-50"
          value={draft.account}
          disabled={fieldsDisabled}
          title={runningTitle}
          onChange={(e) => onFieldChange({ account: e.target.value })}
          placeholder="(tự chọn)"
          aria-label={`Tài khoản cảnh ${index}`}
        />
      </td>
      <td className="px-2 py-1">{status}</td>
      <td className="whitespace-nowrap px-2 py-1">
        <div className="flex flex-wrap gap-1">
          {showRun ? (
            <button type="button" disabled={disabled} className={BTN_NEUTRAL} onClick={handleRunClick}>
              Chạy
            </button>
          ) : null}
          {showRerun ? (
            <button type="button" disabled={disabled} className={BTN_NEUTRAL} onClick={() => void handleRerunClick()}>
              Chạy lại
            </button>
          ) : null}
          {showStop && docScene?.taskId ? (
            <button type="button" disabled={pendingStop} className={BTN_NEUTRAL} onClick={onStop}>
              Dừng
            </button>
          ) : null}
          {docScene?.localPath && docScene.taskId ? (
            <button type="button" disabled={pendingOpen} className={BTN_NEUTRAL} onClick={onOpenFile}>
              Mở thư mục
            </button>
          ) : null}
          <button type="button" disabled={fieldsDisabled} title={runningTitle} className={BTN_DANGER} onClick={onRemove}>
            Xoá dòng
          </button>
        </div>
      </td>
    </tr>
  );
}
