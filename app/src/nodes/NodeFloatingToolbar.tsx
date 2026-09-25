// Thanh cong cu noi (P-lumina) hien ngay TREN node khi node duoc chon — dung `NodeToolbar` cua
// @xyflow/react (tu an/hien theo `selected`, tu dinh vi tren node). Cac nut: chay lai / tai xuong /
// phong to. Hien tai KHONG co lenh "chay lai mot node" rieng trong store nen ba nut deu la stub
// (title tooltip) — giu cho dung bo cuc tham chieu, noi day that su khi backend co lenh tuong ung.
import { NodeToolbar, Position } from "@xyflow/react";

function IconButton({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      className="nodrag flex h-7 w-7 items-center justify-center rounded-md text-slate-300 hover:bg-white/10 hover:text-white"
      onClick={(e) => {
        // Stub — chua co hanh dong backend tuong ung; chan noi/keo node phia duoi.
        e.stopPropagation();
      }}
    >
      {children}
    </button>
  );
}

export default function NodeFloatingToolbar({ nodeId }: { nodeId: string }) {
  return (
    <NodeToolbar nodeId={nodeId} position={Position.Top} offset={10}>
      <div className="flex items-center gap-0.5 rounded-lg border border-white/10 bg-[#1c1c22] px-1 py-0.5 shadow-lg">
        <IconButton title="Chạy lại">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M21 12a9 9 0 1 1-2.64-6.36" />
            <path d="M21 3v6h-6" />
          </svg>
        </IconButton>
        <IconButton title="Tải xuống">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 3v12" />
            <path d="m7 10 5 5 5-5" />
            <path d="M5 21h14" />
          </svg>
        </IconButton>
        <IconButton title="Phóng to">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M15 3h6v6" />
            <path d="M9 21H3v-6" />
            <path d="M21 3l-7 7" />
            <path d="M3 21l7-7" />
          </svg>
        </IconButton>
      </div>
    </NodeToolbar>
  );
}
