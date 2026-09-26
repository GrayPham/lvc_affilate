// Thanh cong cu noi (P-lumina) hien ngay TREN node khi node duoc chon — dung `NodeToolbar` cua
// @xyflow/react (tu an/hien theo `selected`, tu dinh vi tren node). Body node de KEO di chuyen;
// moi HANH DONG (them/thay/xoa/phong to...) nam o thanh nay. Truyen `actions`; khong truyen ->
// bo nut stub mac dinh (giu bo cuc tham chieu khi backend chua co lenh tuong ung).
import type { ReactNode } from "react";
import { NodeToolbar, Position } from "@xyflow/react";

export interface ToolbarAction {
  key: string;
  title: string;
  icon: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  /** To do (vd nut Xoa) — mau canh bao khi hover. */
  danger?: boolean;
}

function ActionButton({ action }: { action: ToolbarAction }) {
  return (
    <button
      type="button"
      title={action.title}
      aria-label={action.title}
      disabled={action.disabled}
      className={
        "nodrag flex h-7 w-7 items-center justify-center rounded-md text-slate-300 disabled:cursor-not-allowed disabled:opacity-40 " +
        (action.danger ? "hover:bg-red-600 hover:text-white" : "hover:bg-white/10 hover:text-white")
      }
      onClick={(e) => {
        e.stopPropagation();
        if (!action.disabled) {
          action.onClick();
        }
      }}
    >
      {action.icon}
    </button>
  );
}

/** Bo nut stub mac dinh (chay lai / tai xuong / phong to) — chua noi backend. */
const STUB_ACTIONS: ToolbarAction[] = [
  {
    key: "rerun",
    title: "Chạy lại (sắp có)",
    onClick: () => {},
    disabled: true,
    icon: (
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 12a9 9 0 1 1-2.64-6.36" />
        <path d="M21 3v6h-6" />
      </svg>
    ),
  },
];

export default function NodeFloatingToolbar({
  nodeId,
  actions,
}: {
  nodeId: string;
  actions?: ToolbarAction[];
}) {
  const items = actions ?? STUB_ACTIONS;
  return (
    <NodeToolbar nodeId={nodeId} position={Position.Top} offset={10}>
      <div className="flex items-center gap-0.5 rounded-lg border border-white/10 bg-[#1c1c22] px-1 py-0.5 shadow-lg">
        {items.map((a) => (
          <ActionButton key={a.key} action={a} />
        ))}
      </div>
    </NodeToolbar>
  );
}
