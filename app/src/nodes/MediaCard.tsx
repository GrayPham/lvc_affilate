// Khung "the media" (P-lumina) cho node INPUT/ket qua (image_upload, dola_video): mot the bo tron
// nen toi ma THAN la vung xem truoc (anh/placeholder), pill nhan o goc tren-trai, cham trang thai +
// badge tien do o goc tren-phai, khung do khi co loi kiem hop le. Tach rieng de hai node media dung
// chung mot dien mao (khac NodeShell — dung cho node co bang dieu khien nhu Prompt/Save).
import type { ReactNode } from "react";
import NodeStatusBadge from "../components/NodeStatusBadge";
import type { NodeRunState } from "../store/run";
import type { GraphError } from "../ipc/types";
import { formatGraphError } from "../graph/errors";

interface MediaCardProps {
  label: string;
  selected: boolean;
  errors?: GraphError[];
  runState?: NodeRunState;
  /** The rong hon cho node ket qua (dola_video) so voi node input (image_upload). */
  wide?: boolean;
  /** Vung xem truoc (than the) — anh thumbnail hoac placeholder. */
  children: ReactNode;
}

/** Mau cham trang thai theo `NodeRunState.state` (mirror mau badge) — xam khi chua chay. */
const DOT_CLASS: Record<string, string> = {
  queued: "bg-slate-400",
  running: "bg-sky-400",
  done: "bg-emerald-400",
  failed: "bg-red-400",
  cancelled: "bg-amber-400",
};

export default function MediaCard({ label, selected, errors, runState, wide, children }: MediaCardProps) {
  const hasErrors = Boolean(errors && errors.length > 0);
  const dotClass = runState ? (DOT_CLASS[runState.state] ?? "bg-slate-400") : "bg-slate-600";
  return (
    <div
      className={
        "relative overflow-hidden rounded-2xl border bg-[#17171a] shadow-lg transition-colors " +
        (wide ? "w-72" : "w-56") +
        " " +
        (hasErrors
          ? "border-red-500/80 ring-1 ring-red-500/40"
          : selected
            ? "border-sky-400/80 ring-1 ring-sky-400/40"
            : "border-white/10")
      }
    >
      {/* Than the = vung xem truoc */}
      <div className="relative">{children}</div>

      {/* Pill nhan goc tren-trai */}
      <span className="pointer-events-none absolute left-2 top-2 rounded-md bg-black/55 px-2 py-0.5 text-[11px] font-medium text-white backdrop-blur-sm">
        {label}
      </span>

      {/* Cham trang thai + badge tien do goc tren-phai */}
      <div className="absolute right-2 top-2 flex items-center gap-1.5">
        {runState ? <NodeStatusBadge state={runState} /> : null}
        <span className={"h-2 w-2 rounded-full " + dotClass} />
      </div>

      {hasErrors ? (
        <div
          className="absolute inset-x-0 bottom-0 bg-red-950/80 px-2 py-1 text-[10px] leading-tight text-red-200"
          title={errors?.map(formatGraphError).join("\n")}
        >
          {errors?.length} lỗi — xem "Kiểm tra"
        </div>
      ) : null}
    </div>
  );
}
