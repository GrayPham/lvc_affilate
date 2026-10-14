// Bang task cua man "Tác vụ" (Task 13 brief): thời gian tạo, run (rút gọn), cảnh #, tài khoản,
// mode/thời lượng, trạng thái + mã lỗi (nhãn Việt, tooltip mã gốc), stage, hội thoại (✓/–), thao
// tác (Mở thư mục / Tiếp tục / Huỷ — ẩn/hiện theo `format/task.ts::isResumable`/`isCancellable`,
// "Mở thư mục" chỉ hiện khi có `localPath`).
import { failureLabel, formatElapsed, isCancellable, isResumable, modeLabel, stageLabel, statusLabel } from "../format/task";
import { filterByStatus, useTasksStore } from "../store/tasks";
import type { TaskDto } from "../ipc/types";

const STATUS_BADGE_CLASS: Record<string, string> = {
  queued: "bg-fill text-ink2",
  processing: "bg-sky-500/15 text-rest",
  completed: "bg-emerald-500/15 text-ok",
  failed: "bg-red-500/15 text-rose",
};

function formatCreatedAt(unixSeconds: number): string {
  try {
    return new Date(unixSeconds * 1000).toLocaleString("vi-VN");
  } catch {
    return String(unixSeconds);
  }
}

function shortRunId(runId: string): string {
  return runId.length > 8 ? `${runId.slice(0, 8)}…` : runId;
}

/** Thoi gian da troi qua hien thi trong bang: uu tien khoang startedAt..finishedAt (hoac ..now
 * neu chua xong) — `null` khi chua bat dau (con `queued`, chua co `startedAt`). */
function elapsedMsOf(task: TaskDto): number | null {
  if (task.startedAt === null) {
    return null;
  }
  const end = task.finishedAt ?? Date.now() / 1000;
  return Math.max(0, (end - task.startedAt) * 1000);
}

interface TaskRowProps {
  task: TaskDto;
  pending: boolean;
  onOpenFile: (id: string) => void;
  onResume: (id: string) => void;
  onCancel: (id: string) => void;
}

function TaskRow({ task, pending, onOpenFile, onResume, onCancel }: TaskRowProps) {
  const elapsedMs = elapsedMsOf(task);
  const badgeClass = STATUS_BADGE_CLASS[task.status] ?? "bg-fill text-ink2";

  return (
    <tr className="border-b border-line text-xs text-ink2">
      <td className="whitespace-nowrap px-2 py-1">{formatCreatedAt(task.createdAt)}</td>
      <td className="whitespace-nowrap px-2 py-1 font-mono" title={task.runId}>
        {shortRunId(task.runId)}
      </td>
      <td className="px-2 py-1 text-center">{task.sceneIndex ?? "–"}</td>
      <td className="px-2 py-1" title={task.account ?? ""}>
        {task.account ?? "(tự chọn)"}
      </td>
      <td className="whitespace-nowrap px-2 py-1">
        {modeLabel(task.mode)} · {task.duration}s
      </td>
      <td className="px-2 py-1">
        <span className={`inline-block rounded px-1.5 py-0.5 font-medium ${badgeClass}`}>
          {statusLabel(task.status)}
        </span>
        {task.failureCode ? (
          <div className="mt-0.5 text-rose" title={task.failureCode}>
            {failureLabel(task.failureCode)}
          </div>
        ) : null}
        {elapsedMs !== null ? <div className="text-[10px] text-ink3">{formatElapsed(elapsedMs)}</div> : null}
      </td>
      <td className="whitespace-nowrap px-2 py-1">{stageLabel(task.stage)}</td>
      <td className="px-2 py-1 text-center" title={task.hasConversation ? "Có hội thoại" : "Chưa có hội thoại"}>
        {task.hasConversation ? "✓" : "–"}
      </td>
      <td className="whitespace-nowrap px-2 py-1">
        <div className="flex gap-1">
          {task.localPath ? (
            <button
              type="button"
              disabled={pending}
              className="rounded border border-line px-1.5 py-0.5 hover:bg-fill2 disabled:opacity-50"
              onClick={() => onOpenFile(task.id)}
            >
              Mở thư mục
            </button>
          ) : null}
          {isResumable(task) ? (
            <button
              type="button"
              disabled={pending}
              className="rounded border border-sky-500/30 px-1.5 py-0.5 text-rest hover:bg-rest/10 disabled:opacity-50"
              onClick={() => onResume(task.id)}
            >
              Tiếp tục
            </button>
          ) : null}
          {isCancellable(task) ? (
            <button
              type="button"
              disabled={pending}
              className="rounded border border-rose/35 px-1.5 py-0.5 text-rose hover:bg-rose/10 disabled:opacity-50"
              onClick={() => onCancel(task.id)}
            >
              Huỷ
            </button>
          ) : null}
        </div>
      </td>
    </tr>
  );
}

export default function TaskTable() {
  const rows = useTasksStore((s) => s.rows);
  const status = useTasksStore((s) => s.filter.status);
  const pendingIds = useTasksStore((s) => s.pendingIds);
  const error = useTasksStore((s) => s.error);
  const openTaskFile = useTasksStore((s) => s.openTaskFile);
  const resumeTask = useTasksStore((s) => s.resumeTask);
  const cancelTask = useTasksStore((s) => s.cancelTask);

  const visible = filterByStatus(rows, status);

  return (
    <div className="flex-1 overflow-auto p-2">
      {error ? <p className="mb-2 text-xs text-rose">{error}</p> : null}
      {visible.length === 0 ? (
        <p className="p-4 text-center text-xs text-ink3">
          {rows.length === 0 ? "Chưa có tác vụ nào." : "Không có tác vụ nào khớp bộ lọc."}
        </p>
      ) : (
        <table className="w-full border-collapse text-left">
          <thead>
            <tr className="border-b border-line text-[10px] uppercase tracking-wide text-ink3">
              <th className="px-2 py-1 font-medium">Tạo lúc</th>
              <th className="px-2 py-1 font-medium">Run</th>
              <th className="px-2 py-1 font-medium">Cảnh</th>
              <th className="px-2 py-1 font-medium">Tài khoản</th>
              <th className="px-2 py-1 font-medium">Mode/thời lượng</th>
              <th className="px-2 py-1 font-medium">Trạng thái</th>
              <th className="px-2 py-1 font-medium">Bước</th>
              <th className="px-2 py-1 font-medium">Hội thoại</th>
              <th className="px-2 py-1 font-medium">Thao tác</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((task) => (
              <TaskRow
                key={task.id}
                task={task}
                pending={pendingIds.has(task.id)}
                onOpenFile={(id) => void openTaskFile(id)}
                onResume={(id) => void resumeTask(id)}
                onCancel={(id) => void cancelTask(id)}
              />
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
