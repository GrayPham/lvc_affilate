// Thanh loc cua man "Tác vụ" (Task 13 brief): dropdown run (`runList(50)` + "tất cả") + dropdown
// trang thai (loc client-side qua `store/tasks.ts::filterByStatus`, xem doc trong file do — server
// `task_list` khong co tham so trang thai rieng).
import { useEffect, useState } from "react";

import { runList } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import RunLogDialog from "./RunLogDialog";
import type { RunRow } from "../ipc/types";
import { useTasksStore, type StatusFilter } from "../store/tasks";

/** So run toi da liet ke o dropdown — man hinh chi can vai chuc run gan day, khong can phan trang. */
const RUN_LIST_LIMIT = 50;

const STATUS_OPTIONS: ReadonlyArray<{ value: StatusFilter; label: string }> = [
  { value: "all", label: "Tất cả trạng thái" },
  { value: "queued", label: "Đang chờ" },
  { value: "processing", label: "Đang xử lý" },
  { value: "completed", label: "Hoàn tất" },
  { value: "failed", label: "Thất bại" },
];

function formatRunLabel(run: RunRow): string {
  const shortId = run.id.length > 8 ? `${run.id.slice(0, 8)}…` : run.id;
  let when = "";
  try {
    when = new Date(run.startedAt * 1000).toLocaleString("vi-VN");
  } catch {
    when = String(run.startedAt);
  }
  return `${shortId} — ${when}`;
}

export default function TaskFilters() {
  const filter = useTasksStore((s) => s.filter);
  const setRunId = useTasksStore((s) => s.setRunId);
  const setStatus = useTasksStore((s) => s.setStatus);
  const loading = useTasksStore((s) => s.loading);

  const [runs, setRuns] = useState<RunRow[]>([]);
  const [runsError, setRunsError] = useState<string | null>(null);
  /** Lan chay dang mo hop thoai "Chi tiết lần chạy"; `null` = dong. */
  const [showLog, setShowLog] = useState<RunRow | null>(null);

  useEffect(() => {
    let cancelled = false;
    runList(RUN_LIST_LIMIT)
      .then((list) => {
        if (!cancelled) {
          setRuns(list);
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setRunsError(errorText(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-white/10 bg-neutral-900/60 p-2">
      <label className="flex items-center gap-1 text-xs text-neutral-300">
        Run
        <select
          className="rounded border border-white/10 bg-white/5 px-2 py-1 text-xs text-neutral-100"
          value={filter.runId ?? ""}
          onChange={(e) => setRunId(e.target.value === "" ? null : e.target.value)}
        >
          <option value="" className="bg-neutral-900 text-neutral-100">Tất cả</option>
          {runs.map((run) => (
            <option key={run.id} value={run.id} className="bg-neutral-900 text-neutral-100">
              {formatRunLabel(run)}
            </option>
          ))}
        </select>
      </label>

      <label className="flex items-center gap-1 text-xs text-neutral-300">
        Trạng thái
        <select
          className="rounded border border-white/10 bg-white/5 px-2 py-1 text-xs text-neutral-100"
          value={filter.status}
          onChange={(e) => setStatus(e.target.value as StatusFilter)}
        >
          {STATUS_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value} className="bg-neutral-900 text-neutral-100">
              {opt.label}
            </option>
          ))}
        </select>
      </label>

      <button
        type="button"
        disabled={filter.runId === null}
        title={
          filter.runId === null
            ? "Chọn một lần chạy ở ô Run để xem chi tiết"
            : "Xem dòng thời gian từng node của lần chạy này"
        }
        className="rounded border border-white/10 bg-white/5 px-2 py-1 text-xs text-neutral-200 hover:bg-white/10 disabled:cursor-not-allowed disabled:opacity-40"
        onClick={() => {
          const run = runs.find((r) => r.id === filter.runId);
          if (run) {
            setShowLog(run);
          }
        }}
      >
        Chi tiết lần chạy
      </button>

      {loading ? <span className="text-xs text-neutral-500">Đang tải…</span> : null}
      {runsError ? <span className="text-xs text-red-400">Không tải được danh sách run: {runsError}</span> : null}

      {showLog ? <RunLogDialog run={showLog} onClose={() => setShowLog(null)} /> : null}
    </div>
  );
}
