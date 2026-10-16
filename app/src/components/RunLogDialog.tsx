// Hop thoai "Chi tiết lần chạy" — dung theo `references/lvc-ui-design/screens/RunLog.dc.html`.
//
// Dong thoi gian theo NODE, gom tu cac dong `tasks` cua lan chay (xem `run_log.ts::buildTimeline`
// — ham THUAN, co test rieng). Node loi bung ra the loi kem ma loi that.
//
// Khac ban thiet ke o hai cho, co ly do:
// - "Log đầy đủ" o day dung tu CHINH cac dong `tasks` da luu (app khong giu log van ban cho tung
//   lan chay) — moi dong deu la du lieu that, khong bia them dong nao.
// - Khong co nut "Chạy lại từ node 3": engine chua ho tro chay lai tu MOT node giua do thi. Thay
//   vao do la "Thêm lại vào hàng chờ" (chay lai ca workflow) — viec that su lam duoc.
import { useEffect, useMemo, useState } from "react";

import { taskList, workflowGet } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { failureLabel, stageLabel } from "../format/task";
import { useQueueStore } from "../store/queue";
import { Button } from "../ui/primitives";
import { buildLogText, buildTimeline, formatClock, formatDuration } from "./run_log";
import type { RunRow, TaskDto } from "../ipc/types";
import type { StageState, TimelineRow } from "./run_log";

/** Mau + ky hieu cua tung trang thai node — khop bang trong ban thiet ke. */
const STATE_STYLE: Record<StageState, { mark: string; dot: string; line: string; name: string }> = {
  ok: { mark: "✓", dot: "bg-ok/15 text-ok", line: "bg-ok", name: "text-fg" },
  err: { mark: "!", dot: "bg-rose/15 text-rose", line: "bg-line2", name: "text-fg" },
  running: {
    mark: "•",
    dot: "bg-accent-soft text-accent",
    line: "bg-primary/40",
    name: "text-fg",
  },
  skip: { mark: "–", dot: "bg-fill2 text-ink3", line: "bg-line2", name: "text-ink3" },
};

/** Dong mo ta ngan ben canh ten node. */
function metaOf(row: TimelineRow): string {
  const parts: string[] = [];
  if (row.taskCount > 1) {
    parts.push(`${row.doneCount}/${row.taskCount} cảnh`);
  }
  if (row.account !== "") {
    parts.push(row.account);
  }
  if (row.state === "running" && row.stage !== "") {
    parts.push(stageLabel(row.stage));
  }
  if (row.state === "skip") {
    parts.push("chưa chạy");
  }
  return parts.join(" · ");
}

interface RunLogDialogProps {
  run: RunRow;
  /** Ten workflow de hien tren tieu de; rong thi dung id. */
  workflowName?: string;
  onClose: () => void;
}

export default function RunLogDialog({ run, workflowName, onClose }: RunLogDialogProps) {
  const [tasks, setTasks] = useState<TaskDto[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [requeueMsg, setRequeueMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const enqueue = useQueueStore((s) => s.add);

  useEffect(() => {
    let cancelled = false;
    // 500: mot lan chay nhieu canh nhat trong thuc te van duoi nguong nay; lay het de dong thoi
    // gian khong bi thieu node cuoi.
    taskList(run.id, 500)
      .then((list) => {
        if (!cancelled) {
          setTasks(list);
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setError(errorText(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [run.id]);

  const rows = useMemo(() => buildTimeline(tasks ?? []), [tasks]);
  const logText = useMemo(() => buildLogText(tasks ?? []), [tasks]);
  const logLines = logText === "" ? 0 : logText.split("\n").length;

  async function handleCopy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(logText);
      setCopied(true);
    } catch (e) {
      setError(errorText(e));
    }
  }

  /** Them lai CA workflow vao hang cho — doc lai do thi hien tai (khong dung anh chup cu). */
  async function handleRequeue(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const doc = await workflowGet(run.workflowId);
      await enqueue(doc.meta.id, doc.meta.name, doc.graph);
      setRequeueMsg(`Đã thêm ${doc.meta.name} vào cuối hàng chờ.`);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  const failedRow = rows.find((r) => r.state === "err");
  const statusPill =
    run.status === "failed"
      ? { text: "Lỗi", cls: "bg-rose/15 text-rose", dot: "bg-rose" }
      : run.status === "running"
        ? { text: "Đang chạy", cls: "bg-accent-soft text-accent", dot: "bg-primary" }
        : run.status === "cancelled"
          ? { text: "Đã huỷ", cls: "bg-fill2 text-ink3", dot: "bg-ink3" }
          : { text: "Hoàn tất", cls: "bg-ok/15 text-ok", dot: "bg-ok" };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center overflow-auto bg-black/60 p-8"
      role="dialog"
      aria-modal="true"
      aria-labelledby="logTitle"
    >
      <div className="w-[620px] max-w-full overflow-hidden rounded-2xl border border-line2 bg-panel shadow-[var(--dialog-shadow)]">
        <header className="flex items-start gap-2.5 border-b border-line px-[18px] py-4">
          <div className="min-w-0 flex-1">
            <h2 id="logTitle" className="truncate text-[16px] font-bold text-fg">
              {workflowName && workflowName !== "" ? workflowName : run.workflowId}
            </h2>
            <p className="mt-1 text-[12.5px] text-ink3">
              bắt đầu {formatClock(run.startedAt)}
              {run.finishedAt > 0 ? ` · xong ${formatClock(run.finishedAt)}` : ""}
              {run.startedAt > 0 && run.finishedAt > 0
                ? ` · ${formatDuration(run.finishedAt - run.startedAt)}`
                : ""}
            </p>
          </div>
          <span
            className={
              "inline-flex min-h-[28px] items-center gap-1.5 rounded-full px-2.5 text-xs font-semibold " +
              statusPill.cls
            }
          >
            <span className={"h-[7px] w-[7px] rounded-full " + statusPill.dot} />
            {statusPill.text}
          </span>
          <button
            type="button"
            aria-label="Đóng"
            onClick={onClose}
            className="grid h-9 w-9 flex-none place-items-center rounded-[9px] text-[16px] text-ink3 hover:text-fg"
          >
            ✕
          </button>
        </header>

        <div className="flex flex-col gap-3.5 px-[18px] py-4">
          {error ? <p className="text-[12.5px] text-rose">{error}</p> : null}
          {requeueMsg ? <p className="text-[12.5px] text-ok">{requeueMsg}</p> : null}

          {tasks === null ? (
            <p className="text-[12.5px] text-ink3">Đang tải…</p>
          ) : rows.length === 0 ? (
            <p className="text-[12.5px] text-ink3">
              Lần chạy này chưa ghi được bước nào.
              {run.error === "" ? "" : ` Lỗi: ${run.error}`}
            </p>
          ) : (
            <ol className="flex flex-col">
              {rows.map((row, i) => {
                const st = STATE_STYLE[row.state];
                const last = i === rows.length - 1;
                return (
                  <li key={row.nodeId} className="flex gap-3">
                    <div className="flex flex-none flex-col items-center">
                      <span
                        className={
                          "grid h-6 w-6 place-items-center rounded-full text-[11.5px] font-bold " +
                          st.dot
                        }
                      >
                        {st.mark}
                      </span>
                      {last ? null : <span className={"min-h-[14px] w-0.5 flex-1 " + st.line} />}
                    </div>
                    <div className="min-w-0 flex-1 pb-3.5">
                      <div className="flex flex-wrap items-baseline gap-2">
                        <span className={"text-[13px] font-semibold " + st.name}>
                          {row.index} · {row.nodeId}
                        </span>
                        <span className="text-xs text-ink3">{metaOf(row)}</span>
                        <span className="ml-auto text-xs text-ink3 tabular-nums">
                          {formatDuration(row.durationSec)}
                        </span>
                      </div>

                      {row.state === "err" ? (
                        <div className="mt-2 flex flex-col gap-1.5 rounded-lg border border-rose/45 bg-rose/[0.08] px-3 py-2.5">
                          <span className="text-[13px] font-semibold text-rose">
                            {row.failureCode === ""
                              ? "Node này thất bại"
                              : failureLabel(row.failureCode)}
                          </span>
                          {row.error === "" ? null : (
                            <span className="text-[12.5px] leading-[1.5] text-muted">
                              {row.error}
                            </span>
                          )}
                          <code className="font-mono text-[11.5px] text-ink3">
                            {[row.failureCode, `node=${row.nodeId}`]
                              .filter((s) => s !== "")
                              .join(" · ")}
                          </code>
                        </div>
                      ) : null}
                    </div>
                  </li>
                );
              })}
            </ol>
          )}

          {logLines > 0 ? (
            <details className="rounded-lg border border-line">
              <summary className="flex min-h-10 cursor-pointer items-center px-3 text-[12.5px] font-semibold text-muted">
                Log đầy đủ ({logLines} dòng)
              </summary>
              <pre className="m-0 border-t border-line px-3 py-2.5 font-mono text-[11.5px] leading-[1.7] whitespace-pre-wrap text-ink3">
                {logText}
              </pre>
            </details>
          ) : null}
        </div>

        <div className="flex flex-wrap items-center gap-2 border-t border-line bg-fill px-[18px] py-3.5">
          <Button disabled={logLines === 0} onClick={() => void handleCopy()}>
            {copied ? "Đã sao chép" : "Sao chép log"}
          </Button>
          <span className="flex-1" />
          {failedRow ? (
            <span className="text-xs text-ink3">Lỗi ở node {failedRow.index}</span>
          ) : null}
          <Button disabled={busy} onClick={() => void handleRequeue()}>
            {busy ? "Đang thêm…" : "Thêm lại vào hàng chờ"}
          </Button>
          <Button variant="primary" onClick={onClose}>
            Đóng
          </Button>
        </div>
      </div>
    </div>
  );
}
