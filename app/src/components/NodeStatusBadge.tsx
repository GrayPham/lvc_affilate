// Badge tien do mot node tren canvas, suy tu su kien `node_state` (crates/lvc-engine/src/dag.rs
// NODE_QUEUED/NODE_RUNNING/NODE_DONE/NODE_FAILED/NODE_CANCELLED) qua `store/run.ts`. Khong hien
// gi khi node chua co trang thai chay (canvas dang sua, chua "Chạy" lan nao).
import type { NodeRunState } from "../store/run";

const STATE_LABEL_VI: Record<string, string> = {
  queued: "Chờ",
  running: "Đang chạy",
  done: "Xong",
  failed: "Lỗi",
  cancelled: "Đã huỷ",
};

const STATE_CLASS: Record<string, string> = {
  queued: "bg-fill2 text-ink3",
  running: "bg-sky-100 text-sky-700",
  done: "bg-emerald-100 text-emerald-700",
  failed: "bg-red-100 text-red-700",
  cancelled: "bg-amber-100 text-amber-700",
};

export default function NodeStatusBadge({ state }: { state?: NodeRunState }) {
  if (!state) {
    return null;
  }
  const label = STATE_LABEL_VI[state.state] ?? state.state;
  const cls = STATE_CLASS[state.state] ?? "bg-fill2 text-ink3";
  const countText = state.total > 0 ? ` ${state.ok}/${state.total}` : "";
  const failedText = state.failed > 0 ? ` · lỗi ${state.failed}` : "";
  return (
    <span className={`whitespace-nowrap rounded px-1.5 py-0.5 text-[10px] font-medium ${cls}`}>
      {label}
      {countText}
      {failedText}
    </span>
  );
}
