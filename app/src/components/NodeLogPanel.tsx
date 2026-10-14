// Bảng NHẬT KÝ CHẠY của node đang chọn (P-lumina, góc dưới-trái canvas) — hiện khi node đã có
// trạng thái chạy (`store/run.ts` nodes[id]). Mục đích: bấm vào node (nhất là node viền ĐỎ = lỗi)
// để đọc lý do lỗi và tiến trình từng cảnh, debug cho dễ.
//
// Nguồn dữ liệu:
// - Cấp NODE: `NodeRunState` (state + đếm ok/failed/total + `error` — thông điệp lỗi của node, kể cả
//   node không fan-out như Prompt/Save/Ghép phân cảnh).
// - Cấp CẢNH (node fan-out như Tạo video Dola): các dòng `tasks` của node, lấy qua `task_list(runId)`
//   rồi lọc theo `nodeId` — mỗi cảnh có stage/status/tài khoản/mã lỗi/đường dẫn.
import { useEffect, useState } from "react";

import { useGraphStore } from "../store/graph";
import { useRunStore } from "../store/run";
import { taskList } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import { NODE_LABEL_VI } from "../graph/ports";
import { nodeChipLabel } from "../graph/refs";
import NodeStatusBadge from "./NodeStatusBadge";
import type { NodeKindTag, TaskDto } from "../ipc/types";

const TASK_STATUS_CLASS: Record<string, string> = {
  completed: "text-emerald-300",
  failed: "text-rose",
  processing: "text-sky-300",
  queued: "text-ink2",
};

function taskStatusLabel(status: string, failureCode: string | null): string {
  if (failureCode === "cancelled") {
    return "Đã huỷ";
  }
  switch (status) {
    case "completed":
      return "Xong";
    case "failed":
      return "Lỗi";
    case "processing":
      return "Đang chạy";
    case "queued":
      return "Chờ";
    default:
      return status;
  }
}

export default function NodeLogPanel() {
  const selectedNodeId = useGraphStore((s) => s.selectedNodeId);
  const nodes = useGraphStore((s) => s.nodes);
  const setSelected = useGraphStore((s) => s.setSelected);
  const runId = useRunStore((s) => s.runId);
  const runState = useRunStore((s) => (selectedNodeId ? s.nodes[selectedNodeId] : undefined));

  const [tasks, setTasks] = useState<TaskDto[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Lấy dòng tasks của node đang chọn từ DB (một lần khi đổi node/run) — cấp cảnh cho node fan-out.
  useEffect(() => {
    setTasks([]);
    setLoadError(null);
    if (!runId || !selectedNodeId) {
      return;
    }
    let cancelled = false;
    taskList(runId, 500)
      .then((list) => {
        if (!cancelled) {
          setTasks(list.filter((t) => t.nodeId === selectedNodeId));
        }
      })
      .catch((e: unknown) => {
        if (!cancelled) {
          setLoadError(errorText(e));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [runId, selectedNodeId]);

  const node = selectedNodeId ? nodes.find((n) => n.id === selectedNodeId) : undefined;
  // Chỉ hiện khi node đã có trạng thái chạy — canvas đang sửa (chưa "Chạy") thì không chen bảng này.
  if (!node || !runState) {
    return null;
  }
  const kindLabel = NODE_LABEL_VI[node.data.kind as NodeKindTag] ?? node.data.kind;
  const displayName = nodeChipLabel(node.data);
  const scenes = [...tasks].sort(
    (a, b) => (a.sceneIndex ?? Number.MAX_SAFE_INTEGER) - (b.sceneIndex ?? Number.MAX_SAFE_INTEGER),
  );

  return (
    <div className="pointer-events-auto flex max-h-[min(60vh,22rem)] w-80 max-w-[calc(100vw-2rem)] flex-col overflow-hidden rounded-2xl border border-line bg-[#17171a]/95 text-ink shadow-2xl backdrop-blur-md">
      {/* Tiêu đề */}
      <div className="flex items-center gap-2 border-b border-line px-3 py-2">
        <div className="min-w-0 flex-1">
          <div className="truncate text-xs font-semibold text-ink" title={displayName}>
            Nhật ký · {displayName}
          </div>
          <div className="text-[10px] text-ink3">{kindLabel}</div>
        </div>
        <NodeStatusBadge state={runState} />
        <button
          type="button"
          title="Đóng"
          aria-label="Đóng nhật ký"
          className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-ink2 hover:bg-fill2 hover:text-ink"
          onClick={() => setSelected(null)}
        >
          ×
        </button>
      </div>

      <div className="flex flex-col gap-2 overflow-y-auto p-3">
        {/* Tóm tắt đếm */}
        <div className="flex flex-wrap gap-x-3 gap-y-0.5 text-[11px] text-ink2">
          {runState.total > 0 ? (
            <span>
              Hoàn tất <span className="text-ink">{runState.done}/{runState.total}</span>
            </span>
          ) : null}
          {runState.ok > 0 ? <span className="text-emerald-300">✓ {runState.ok}</span> : null}
          {runState.failed > 0 ? <span className="text-rose">✗ {runState.failed}</span> : null}
        </div>

        {/* Lỗi cấp node (nhật ký chính cho node không fan-out) */}
        {runState.error ? (
          <div className="whitespace-pre-wrap break-words rounded-md border border-red-500/30 bg-red-500/10 px-2 py-1.5 text-[11px] leading-relaxed text-red-200">
            {runState.error}
          </div>
        ) : null}

        {loadError ? <p className="text-[11px] text-rose">Không tải được nhật ký cảnh: {loadError}</p> : null}

        {/* Cấp cảnh (node fan-out) */}
        {scenes.length > 0 ? (
          <div className="flex flex-col gap-1">
            <div className="text-[10px] uppercase tracking-wide text-ink3">Từng cảnh</div>
            {scenes.map((t) => {
              const statusCls = TASK_STATUS_CLASS[t.status] ?? "text-ink2";
              return (
                <div key={t.id} className="rounded-md border border-line bg-fill px-2 py-1 text-[11px]">
                  <div className="flex items-center gap-2">
                    <span className="font-mono text-ink2">#{t.sceneIndex ?? "?"}</span>
                    <span className={statusCls}>{taskStatusLabel(t.status, t.failureCode)}</span>
                    {t.account ? <span className="truncate text-ink3" title={t.account}>· {t.account}</span> : null}
                    <span className="ml-auto text-ink3">{t.stage}</span>
                  </div>
                  {t.error ? (
                    <div className="mt-0.5 whitespace-pre-wrap break-words text-rose/90">
                      {t.failureCode ? <span className="font-mono">[{t.failureCode}] </span> : null}
                      {t.error}
                    </div>
                  ) : null}
                  {t.localPath ? (
                    <div className="mt-0.5 truncate text-emerald-300/80" title={t.localPath}>{t.localPath}</div>
                  ) : null}
                </div>
              );
            })}
          </div>
        ) : null}

        {!runState.error && scenes.length === 0 ? (
          <p className="text-[11px] text-ink3">
            {runState.state === "done"
              ? "Node chạy xong, không có cảnh chi tiết."
              : runState.state === "running"
                ? "Đang chạy…"
                : "Chưa có nhật ký chi tiết cho node này."}
          </p>
        ) : null}
      </div>
    </div>
  );
}
