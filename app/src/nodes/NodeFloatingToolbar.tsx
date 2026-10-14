// Thanh cong cu noi (P-lumina) hien ngay TREN node khi node duoc chon — dung `NodeToolbar` cua
// @xyflow/react (tu an/hien theo `selected`, tu dinh vi tren node). Body node de KEO di chuyen;
// moi HANH DONG (thay/xoa anh/phong to...) nam o thanh nay. Nut "Xoa node" LUON co o cuoi thanh cho
// MOI node (khong node nao phai tu them) — truoc day khong co cach xoa node tren UI.
import type { ReactNode } from "react";
import { NodeToolbar, Position } from "@xyflow/react";

import { useGraphStore } from "../store/graph";

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
        "nodrag flex h-7 w-7 items-center justify-center rounded-md text-ink2 disabled:cursor-not-allowed disabled:opacity-40 " +
        (action.danger ? "hover:bg-red-600 hover:text-ink" : "hover:bg-fill2 hover:text-ink")
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

const TrashIcon = (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6" />
    <path d="M10 11v6M14 11v6" />
  </svg>
);

export default function NodeFloatingToolbar({
  nodeId,
  actions,
}: {
  nodeId: string;
  actions?: ToolbarAction[];
}) {
  const deleteNode = useGraphStore((s) => s.deleteNode);
  const items = actions ?? [];
  return (
    <NodeToolbar nodeId={nodeId} position={Position.Top} offset={10}>
      <div className="flex items-center gap-0.5 rounded-lg border border-line bg-panel px-1 py-0.5 shadow-lg">
        {items.map((a) => (
          <ActionButton key={a.key} action={a} />
        ))}
        {items.length > 0 ? <span className="mx-0.5 h-5 w-px bg-fill2" /> : null}
        <ActionButton
          action={{
            key: "delete-node",
            title: "Xoá node",
            danger: true,
            onClick: () => deleteNode(nodeId),
            icon: TrashIcon,
          }}
        />
      </div>
    </NodeToolbar>
  );
}
