// Khung dung chung cho 4 node P3 (Prompt/ImageUpload/Video/Save) — tieu de + badge tien do
// (NodeStatusBadge, tu run_state) + khung loi kiem hop le (tu workflow_validate, nodeErrors cua
// store/graph.ts). Tach rieng file de 4 node khong lap lai cung mot doan style/badge/loi.
import type { ReactNode } from "react";
import NodeStatusBadge from "../components/NodeStatusBadge";
import type { NodeRunState } from "../store/run";
import type { GraphError } from "../ipc/types";
import { formatGraphError } from "../graph/errors";

interface NodeShellProps {
  label: string;
  selected: boolean;
  errors?: GraphError[];
  runState?: NodeRunState;
  children: ReactNode;
}

export default function NodeShell({ label, selected, errors, runState, children }: NodeShellProps) {
  const hasErrors = Boolean(errors && errors.length > 0);
  return (
    <div
      className={
        "w-56 rounded-lg border bg-white p-2 shadow-sm " +
        (hasErrors
          ? "border-red-500 ring-1 ring-red-300"
          : selected
            ? "border-slate-900 ring-1 ring-slate-400"
            : "border-slate-300")
      }
    >
      <div className="mb-1 flex items-center justify-between gap-2">
        <span className="text-xs font-semibold text-slate-700">{label}</span>
        <NodeStatusBadge state={runState} />
      </div>
      <div className="flex flex-col gap-1">{children}</div>
      {hasErrors ? (
        <div
          className="mt-1 rounded bg-red-50 px-1 py-0.5 text-[10px] leading-tight text-red-700"
          title={errors?.map(formatGraphError).join("\n")}
        >
          {errors?.length} lỗi — xem "Kiểm tra"
        </div>
      ) : null}
    </div>
  );
}
