// Bang tien do tung canh (tac vu) cua LAN CHAY hien tai — Task 12 brief: mot dong moi tac vu
// "#sceneIndex · tài khoản · stage · elapsed". `sceneIndex`/`account` KHONG co trong
// `task_stage`/`task_failed` (chi `task_done` co `account`) nen lay tu `task_list({runId})` goi
// mot lan khi `runId` doi (Task 13 se lam sau day hoan chinh hon o man Tac vu); stage/elapsed lay
// UU TIEN tu su kien song (`store/run.ts`) vi cap nhat theo thoi gian thuc, roi moi rot ve
// `task_list` khi chua co su kien nao cho tac vu do.
import { useEffect, useState } from "react";

import { useRunStore } from "../store/run";
import { taskList } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { TaskDto } from "../ipc/types";

function formatElapsed(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const mm = Math.floor(totalSeconds / 60);
  const ss = totalSeconds % 60;
  return mm > 0 ? `${mm}p${ss.toString().padStart(2, "0")}s` : `${ss}s`;
}

type DisplayStatus = "running" | "done" | "failed";

function statusOf(dtoStatus: string | undefined, failureCode: string | null | undefined): DisplayStatus {
  if (dtoStatus === "completed") {
    return "done";
  }
  if (dtoStatus === "failed") {
    return "failed";
  }
  void failureCode;
  return "running";
}

const ROW_CLASS: Record<DisplayStatus, string> = {
  running: "border-sky-200 bg-sky-50",
  done: "border-emerald-200 bg-emerald-50",
  failed: "border-red-200 bg-red-50",
};

export default function ProgressPanel() {
  const runId = useRunStore((s) => s.runId);
  const liveTasks = useRunStore((s) => s.tasks);
  const [meta, setMeta] = useState<Record<string, TaskDto>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setMeta({});
    setError(null);
    if (!runId) {
      return;
    }
    let cancelled = false;
    taskList(runId, 500)
      .then((list) => {
        if (cancelled) {
          return;
        }
        const byId: Record<string, TaskDto> = {};
        for (const t of list) {
          byId[t.id] = t;
        }
        setMeta(byId);
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setError(errorText(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [runId]);

  if (!runId) {
    return <p className="p-2 text-xs text-slate-400">Chưa có lần chạy nào — bấm &quot;Chạy&quot; để bắt đầu.</p>;
  }

  const ids = Array.from(new Set([...Object.keys(meta), ...Object.keys(liveTasks)]));
  ids.sort((a, b) => {
    const ia = meta[a]?.sceneIndex ?? Number.MAX_SAFE_INTEGER;
    const ib = meta[b]?.sceneIndex ?? Number.MAX_SAFE_INTEGER;
    return ia - ib;
  });

  return (
    <div className="flex flex-col gap-1 overflow-auto p-2">
      {error ? <p className="text-xs text-red-600">{error}</p> : null}
      {ids.length === 0 ? <p className="text-xs text-slate-400">Chưa có tác vụ nào.</p> : null}
      {ids.map((id) => {
        const live = liveTasks[id];
        const info = meta[id];
        const sceneIndex = info?.sceneIndex ?? null;
        const account = live?.account ?? info?.account ?? null;
        const stage = live?.stage ?? info?.stage ?? "-";
        const elapsedMs = live?.elapsedMs ?? 0;
        const failureCode = live?.failureCode ?? info?.failureCode ?? null;
        const status = live ? (live.status === "done" ? "done" : live.status === "failed" ? "failed" : "running") : statusOf(info?.status, failureCode);
        const stageLabel = failureCode === "cancelled" ? "Đã huỷ" : stage;

        return (
          <div key={id} className={`flex items-center gap-2 rounded border px-2 py-1 text-xs ${ROW_CLASS[status]}`}>
            <span className="w-8 shrink-0 font-mono">#{sceneIndex ?? "?"}</span>
            <span className="w-28 shrink-0 truncate" title={account ?? ""}>
              {account || "auto"}
            </span>
            <span className="flex-1 truncate">{stageLabel}</span>
            <span className="w-14 shrink-0 text-right font-mono">{formatElapsed(elapsedMs)}</span>
          </div>
        );
      })}
    </div>
  );
}
