// Khung "the media" (P-lumina) cho node INPUT/ket qua (image_upload, dola_video): mot the bo tron
// nen toi ma THAN la vung xem truoc (anh/placeholder), pill nhan o goc tren-trai, cham trang thai +
// badge tien do o goc tren-phai, khung do khi co loi kiem hop le. Tach rieng de hai node media dung
// chung mot dien mao (khac NodeShell — dung cho node co bang dieu khien nhu Prompt/Save).
import { useEffect, useRef, useState, type ReactNode } from "react";
import NodeStatusBadge from "../components/NodeStatusBadge";
import type { NodeRunState } from "../store/run";
import type { GraphError } from "../ipc/types";
import { formatGraphError } from "../graph/errors";
import { nodeBorderClass } from "./nodeCardStyle";

interface MediaCardProps {
  label: string;
  selected: boolean;
  errors?: GraphError[];
  runState?: NodeRunState;
  /** The rong hon cho node ket qua (dola_video) so voi node input (image_upload). */
  wide?: boolean;
  /** Neu co: pill nhan doi ten duoc (nhap dup -> o nhap). Dung cho node input de dat ten tham chieu. */
  onRename?: (name: string) => void;
  /** Vung xem truoc (than the) — anh thumbnail hoac placeholder. */
  children: ReactNode;
}

/** Pill nhan goc tren-trai; nhap dup de sua khi co `onRename`. */
function LabelPill({ label, onRename }: { label: string; onRename?: (name: string) => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(label);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editing]);

  function commit(): void {
    const name = draft.trim();
    if (onRename && name.length > 0 && name !== label) {
      onRename(name);
    }
    setEditing(false);
  }

  if (editing) {
    return (
      <input
        ref={inputRef}
        className="nodrag absolute left-2 top-2 z-10 w-28 rounded-md border border-sky-400/60 bg-black/80 px-1.5 py-0.5 text-[11px] text-white outline-none"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            commit();
          } else if (e.key === "Escape") {
            setDraft(label);
            setEditing(false);
          }
        }}
      />
    );
  }
  return (
    <span
      className={
        "absolute left-2 top-2 z-10 rounded-md bg-black/55 px-2 py-0.5 text-[11px] font-medium text-white backdrop-blur-sm " +
        (onRename ? "nodrag cursor-text hover:bg-black/75" : "pointer-events-none")
      }
      title={onRename ? "Nhấp đúp để đổi tên" : undefined}
      onDoubleClick={onRename ? () => { setDraft(label); setEditing(true); } : undefined}
    >
      {label}
    </span>
  );
}

/** Mau cham trang thai theo `NodeRunState.state` (mirror mau badge) — xam khi chua chay. */
const DOT_CLASS: Record<string, string> = {
  queued: "bg-slate-400",
  running: "bg-sky-400",
  done: "bg-emerald-400",
  failed: "bg-red-400",
  cancelled: "bg-amber-400",
};

export default function MediaCard({ label, selected, errors, runState, wide, onRename, children }: MediaCardProps) {
  const hasErrors = Boolean(errors && errors.length > 0);
  const dotClass = runState ? (DOT_CLASS[runState.state] ?? "bg-slate-400") : "bg-slate-600";
  return (
    <div
      className={
        "relative overflow-hidden rounded-2xl border bg-[#17171a] shadow-lg transition-colors " +
        (wide ? "w-72" : "w-56") +
        " " +
        nodeBorderClass({ hasErrors, selected, runState })
      }
    >
      {/* Than the = vung xem truoc */}
      <div className="relative">{children}</div>

      {/* Pill nhan goc tren-trai (nhap dup de doi ten neu co onRename) */}
      <LabelPill label={label} onRename={onRename} />

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
