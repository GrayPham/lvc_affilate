// Hang cho workflow — dung theo ban ban giao `references/lvc-ui-design/screens/Tasks.dc.html`.
//
// Chay TUAN TU o BACKEND (`commands::queue`): UI chi ra lenh + hoi lai tien do (store tu bat/tat
// vong hoi 2s khi hang cho dang chay). Keo tha doi thu tu bang HTML5 drag-and-drop thuan.
import { useEffect, useState } from "react";

import ConfirmDialog from "./ConfirmDialog";
import { useQueueStore } from "../store/queue";
import AddWorkflowDialog from "./AddWorkflowDialog";
import BulkAddDialog from "./BulkAddDialog";
import { Button, SearchInput } from "../ui/primitives";
import type { QueueItem, QueueStatus } from "../ipc/types";

const STATUS_VI: Record<QueueStatus, string> = {
  waiting: "Chờ",
  running: "Đang chạy",
  done: "Hoàn thành",
  failed: "Lỗi",
  skipped: "Đã bỏ qua",
};

const STATUS_COLOR: Record<QueueStatus, string> = {
  waiting: "var(--unknown)",
  running: "var(--accent-text)",
  done: "var(--ok)",
  failed: "var(--rose)",
  skipped: "var(--warn)",
};

/** "12p 40s" / "48s" — `null` khi chua chay xong. */
function formatElapsed(startedAt: number | null, finishedAt: number | null): string {
  if (startedAt === null) {
    return "—";
  }
  const end = finishedAt ?? Math.floor(Date.now() / 1000);
  const secs = Math.max(0, Math.round(end - startedAt));
  if (secs < 60) {
    return `${secs}s`;
  }
  return `${Math.floor(secs / 60)}p ${secs % 60}s`;
}

export default function QueuePanel() {
  const items = useQueueStore((s) => s.items);
  const running = useQueueStore((s) => s.running);
  const error = useQueueStore((s) => s.error);
  const refresh = useQueueStore((s) => s.refresh);
  const remove = useQueueStore((s) => s.remove);
  const reorder = useQueueStore((s) => s.reorder);
  const clearFinished = useQueueStore((s) => s.clearFinished);
  const start = useQueueStore((s) => s.start);
  const stop = useQueueStore((s) => s.stop);
  const startPoll = useQueueStore((s) => s.startPoll);
  const stopPoll = useQueueStore((s) => s.stopPoll);

  const [query, setQuery] = useState("");
  const [dragId, setDragId] = useState<string | null>(null);
  const [removing, setRemoving] = useState<QueueItem | null>(null);
  const [confirmStop, setConfirmStop] = useState(false);
  /** Hop thoai "Thêm workflow" / "Thêm hàng loạt" — hai duong vao hang cho ngoai nut tren canvas. */
  const [showAdd, setShowAdd] = useState(false);
  const [showBulk, setShowBulk] = useState(false);
  const [addedMsg, setAddedMsg] = useState<string | null>(null);

  useEffect(() => {
    void refresh();
    return () => stopPoll();
  }, [refresh, stopPoll]);

  // Hang cho dang chay (vd app vua mo lai) -> bat vong hoi de thay tien do.
  useEffect(() => {
    if (running) {
      startPoll();
    }
  }, [running, startPoll]);

  const q = query.trim().toLowerCase();
  const visible = q === "" ? items : items.filter((i) => i.name.toLowerCase().includes(q));

  const waiting = items.filter((i) => i.status === "waiting").length;
  const runningItem = items.find((i) => i.status === "running");
  const finished = items.filter((i) => i.status !== "waiting" && i.status !== "running").length;

  function onDrop(targetId: string): void {
    if (dragId === null || dragId === targetId) {
      setDragId(null);
      return;
    }
    const ids = items.map((i) => i.id);
    const from = ids.indexOf(dragId);
    const to = ids.indexOf(targetId);
    if (from < 0 || to < 0) {
      setDragId(null);
      return;
    }
    ids.splice(to, 0, ids.splice(from, 1)[0]!);
    setDragId(null);
    void reorder(ids);
  }

  const TH =
    "border-b border-line px-2 py-3 text-left text-[10.5px] font-semibold uppercase tracking-[.09em] text-ink3";
  const TD = "border-b border-line px-2 py-3.5 align-top";

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex flex-col gap-3.5 border-b border-line px-6 pb-4 pt-5">
        <div className="flex flex-wrap items-start gap-2">
          <div className="min-w-[220px] flex-1">
            <h1 className="text-[20px] font-bold tracking-[-.01em]">Hàng chờ workflow</h1>
            <p className="mt-1 text-[13px] text-ink3">
              {items.length} workflow · {waiting} đang chờ · {finished} đã xong
            </p>
          </div>
          <Button onClick={() => setShowAdd(true)}>+ Thêm workflow</Button>
          <Button onClick={() => setShowBulk(true)}>Thêm hàng loạt</Button>
          {running ? (
            <Button
              className="border-primary/45 bg-primary/10 text-accent"
              onClick={() => setConfirmStop(true)}
            >
              ■ Dừng hàng chờ
            </Button>
          ) : (
            <Button variant="primary" disabled={waiting === 0} onClick={() => void start()}>
              ▶ Chạy hàng chờ
            </Button>
          )}
          <Button disabled={finished === 0} onClick={() => void clearFinished()}>
            Dọn mục đã xong
          </Button>
        </div>

        {runningItem ? (
          <div className="flex flex-wrap items-center gap-3.5 rounded-lg border border-primary/30 bg-primary/5 px-3.5 py-3">
            <span
              className="h-2.5 w-2.5 shrink-0 rounded-full bg-primary"
              style={{ boxShadow: "0 0 0 4px rgb(245 165 36 / .2)" }}
            />
            <div className="flex min-w-0 flex-[1_1_260px] flex-col gap-1">
              <span className="truncate text-[13px] font-semibold">
                Đang chạy · {runningItem.name}
              </span>
              <span className="text-[12px] text-ink3">
                Đã chạy {formatElapsed(runningItem.startedAt, null)}
              </span>
            </div>
          </div>
        ) : null}

        {addedMsg ? <p className="text-[12.5px] text-ok">{addedMsg}</p> : null}

        <div className="flex">
          <SearchInput
            value={query}
            onChange={setQuery}
            label="Tìm trong hàng chờ"
            placeholder="Tìm theo tên workflow…"
          />
        </div>
      </header>

      <div className="flex-1 overflow-auto px-6 pb-6 pt-2">
        {error ? <p className="mb-2 text-xs text-rose">{error}</p> : null}

        {visible.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
            <svg viewBox="0 0 24 24" className="h-8 w-8 text-line2" fill="none" stroke="currentColor" strokeWidth={1.5}>
              <rect x="4" y="4" width="16" height="16" rx="3" />
              <path d="M7.5 11.5l2 2 4-4.5" />
            </svg>
            <p className="text-[13px] font-semibold text-muted">
              {items.length === 0 ? "Hàng chờ đang trống" : "Không có workflow nào khớp."}
            </p>
            {items.length === 0 ? (
              <p className="max-w-[46ch] text-[12px] leading-[1.5] text-ink3">
                Mở một workflow ở tab Workflow rồi bấm "Thêm vào hàng chờ" để xếp việc chạy lần
                lượt.
              </p>
            ) : null}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full min-w-[860px] border-collapse text-[13px]">
              <thead>
                <tr>
                  <th className={`${TH} w-[70px]`}>Thứ tự</th>
                  <th className={TH}>Tên workflow</th>
                  <th className={TH}>Trạng thái</th>
                  <th className={TH}>Thời gian</th>
                  <th className={TH}>Thao tác</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((item, idx) => {
                  const isRunning = item.status === "running";
                  return (
                    <tr
                      key={item.id}
                      draggable={item.status === "waiting"}
                      onDragStart={() => setDragId(item.id)}
                      onDragOver={(e) => e.preventDefault()}
                      onDrop={() => onDrop(item.id)}
                      style={isRunning ? { background: "rgb(245 165 36 / .06)" } : undefined}
                      className={dragId === item.id ? "opacity-50" : undefined}
                    >
                      <td className={`${TD} whitespace-nowrap`}>
                        {item.status === "waiting" ? (
                          <span
                            aria-hidden="true"
                            title="Kéo để đổi thứ tự"
                            className="mr-1.5 cursor-grab tracking-[-2px] text-ink3"
                          >
                            ⋮⋮
                          </span>
                        ) : null}
                        <span
                          className={
                            "inline-grid h-[26px] min-w-[26px] place-items-center rounded-sm px-1.5 text-[12px] font-bold " +
                            (isRunning ? "bg-primary text-primary-fg" : "bg-fill text-muted")
                          }
                        >
                          {idx + 1}
                        </span>
                      </td>
                      <td className={TD}>
                        <span className="font-semibold text-fg">{item.name}</span>
                        {item.error ? (
                          <span className="mt-[3px] block max-w-[320px] text-[12px] leading-[1.45] text-ink3">
                            {item.error}
                          </span>
                        ) : null}
                      </td>
                      <td className={TD}>
                        <span
                          className="inline-flex items-center gap-[7px] whitespace-nowrap font-semibold"
                          style={{ color: STATUS_COLOR[item.status] }}
                        >
                          <span
                            className="h-[7px] w-[7px] rounded-[4px]"
                            style={{ background: STATUS_COLOR[item.status] }}
                          />
                          {STATUS_VI[item.status]}
                        </span>
                      </td>
                      <td className={`${TD} whitespace-nowrap text-muted`}>
                        {formatElapsed(item.startedAt, item.finishedAt)}
                      </td>
                      <td className={`${TD} whitespace-nowrap`}>
                        <Button small variant="danger" onClick={() => setRemoving(item)}>
                          Bỏ khỏi hàng chờ
                        </Button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {removing ? (
        <ConfirmDialog
          title="Bỏ workflow khỏi hàng chờ?"
          body="Workflow gốc trong Kho mẫu không bị xoá — chỉ bỏ lượt chạy đang xếp."
          list={[{ a: removing.name, b: STATUS_VI[removing.status] }]}
          okLabel="Bỏ khỏi hàng chờ"
          onCancel={() => setRemoving(null)}
          onConfirm={() => {
            void remove(removing.id);
            setRemoving(null);
          }}
        />
      ) : null}

      {confirmStop ? (
        <ConfirmDialog
          severity="caution"
          title="Dừng hàng chờ?"
          body={
            runningItem
              ? `Workflow đang chạy: ${runningItem.name}. Nó vẫn chạy nốt; hàng chờ chỉ không lấy mục kế tiếp.`
              : "Hàng chờ sẽ không lấy mục kế tiếp."
          }
          note="Các workflow đang chờ giữ nguyên thứ tự. Bấm “Chạy hàng chờ” để tiếp tục."
          cancelLabel="Tiếp tục chạy"
          okLabel="Dừng hàng chờ"
          onCancel={() => setConfirmStop(false)}
          onConfirm={() => {
            void stop();
            setConfirmStop(false);
          }}
        />
      ) : null}
      {showAdd ? (
        <AddWorkflowDialog
          onClose={() => setShowAdd(false)}
          onAdded={(name) => {
            setAddedMsg(`Đã thêm "${name}" vào hàng chờ.`);
            void refresh();
          }}
        />
      ) : null}

      {showBulk ? (
        <BulkAddDialog
          onClose={() => setShowBulk(false)}
          onAdded={(count) => {
            setAddedMsg(`Đã thêm ${count} workflow vào hàng chờ.`);
            void refresh();
          }}
        />
      ) : null}

    </div>
  );
}
