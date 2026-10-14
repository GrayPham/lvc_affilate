// Khung dung chung cho node co BANG DIEU KHIEN (Prompt/Save) — tieu de + badge tien do
// (NodeStatusBadge, tu run_state) + khung loi kiem hop le (tu workflow_validate, nodeErrors cua
// store/graph.ts). P-lumina: nen toi cho khop canvas Lumina. Node media (image_upload/dola_video)
// dung `MediaCard` rieng (than the la vung xem truoc).
import type { ReactNode } from "react";
import NodeStatusBadge from "../components/NodeStatusBadge";
import type { NodeRunState } from "../store/run";
import type { GraphError } from "../ipc/types";
import { formatGraphError } from "../graph/errors";
import { nodeBorderClass, toneChipClass, toneEdgeClass, toneForKind } from "./nodeCardStyle";

interface NodeShellProps {
  label: string;
  /** `kind` của node — quyết định tông màu (xem `toneForKind`). Bỏ trống = trung tính. */
  kind?: string;
  selected: boolean;
  errors?: GraphError[];
  runState?: NodeRunState;
  children: ReactNode;
}

export default function NodeShell({ label, kind, selected, errors, runState, children }: NodeShellProps) {
  const hasErrors = Boolean(errors && errors.length > 0);
  const tone = toneForKind(kind ?? "");
  return (
    <div
      className={
        "relative w-56 overflow-hidden rounded-2xl border bg-panel p-2.5 text-ink shadow-lg transition-colors " +
        nodeBorderClass({ hasErrors, selected, runState })
      }
    >
      {/* Dải màu theo loại node — nhìn canvas là biết node làm gì mà không phải đọc nhãn. */}
      <span className={"absolute inset-x-0 top-0 h-[3px] " + toneEdgeClass(tone)} />
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <span className={"rounded-md px-2 py-0.5 text-[11px] font-medium " + toneChipClass(tone)}>
          {label}
        </span>
        <NodeStatusBadge state={runState} />
      </div>
      <div className="flex flex-col gap-1">{children}</div>
      {hasErrors ? (
        <div
          className="mt-1.5 rounded-md bg-red-950/60 px-1.5 py-0.5 text-[10px] leading-tight text-red-200"
          title={errors?.map(formatGraphError).join("\n")}
        >
          {errors?.length} lỗi — xem "Kiểm tra"
        </div>
      ) : null}
    </div>
  );
}
