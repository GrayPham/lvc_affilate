// Thanh cong cu doc BEN TRAI canvas (P-lumina) — mot cot nut icon nen toi noi len tren canvas.
// Wire that: "add" (+) bat/tat menu them node (Palette popover), "templates" mo "Kho mẫu"
// (TemplateGallery cua RunBar), "history" goi undo cua store. Cac nut con lai (tim, cat, tro giup)
// la stub co title tieng Viet cho dung bo cuc tham chieu.
import { useGraphStore } from "../store/graph";
import { useWorkflowUi } from "../store/workflowUi";

function ToolButton({
  title,
  active,
  onClick,
  children,
}: {
  title: string;
  active?: boolean;
  onClick?: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className={
        "flex h-9 w-9 items-center justify-center rounded-lg transition-colors " +
        (active ? "bg-sky-500/20 text-sky-300" : "text-ink2 hover:bg-fill2 hover:text-ink")
      }
    >
      {children}
    </button>
  );
}

const ICON = { size: 18, viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.8, strokeLinecap: "round" as const, strokeLinejoin: "round" as const };

export default function CanvasToolbar() {
  const undo = useGraphStore((s) => s.undo);
  const canUndo = useGraphStore((s) => s.history.length > 0);
  const addMenuOpen = useWorkflowUi((s) => s.addMenuOpen);
  const toggleAddMenu = useWorkflowUi((s) => s.toggleAddMenu);
  const openTemplates = useWorkflowUi((s) => s.openTemplates);

  return (
    <div className="pointer-events-auto flex flex-col items-center gap-1 rounded-2xl border border-line bg-[#17171a]/95 p-1.5 shadow-2xl backdrop-blur-md">
      <ToolButton title="Thêm node" active={addMenuOpen} onClick={toggleAddMenu}>
        <svg width={ICON.size} height={ICON.size} viewBox={ICON.viewBox} fill={ICON.fill} stroke={ICON.stroke} strokeWidth={ICON.strokeWidth} strokeLinecap={ICON.strokeLinecap} strokeLinejoin={ICON.strokeLinejoin}>
          <path d="M12 5v14M5 12h14" />
        </svg>
      </ToolButton>
      <ToolButton title="Tìm kiếm">
        <svg width={ICON.size} height={ICON.size} viewBox={ICON.viewBox} fill={ICON.fill} stroke={ICON.stroke} strokeWidth={ICON.strokeWidth} strokeLinecap={ICON.strokeLinecap} strokeLinejoin={ICON.strokeLinejoin}>
          <circle cx="11" cy="11" r="7" />
          <path d="m21 21-4.3-4.3" />
        </svg>
      </ToolButton>
      <ToolButton title="Kho mẫu" onClick={openTemplates}>
        <svg width={ICON.size} height={ICON.size} viewBox={ICON.viewBox} fill={ICON.fill} stroke={ICON.stroke} strokeWidth={ICON.strokeWidth} strokeLinecap={ICON.strokeLinecap} strokeLinejoin={ICON.strokeLinejoin}>
          <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        </svg>
      </ToolButton>
      <ToolButton title="Hoàn tác" onClick={() => canUndo && undo()}>
        <svg width={ICON.size} height={ICON.size} viewBox={ICON.viewBox} fill={ICON.fill} stroke={ICON.stroke} strokeWidth={ICON.strokeWidth} strokeLinecap={ICON.strokeLinecap} strokeLinejoin={ICON.strokeLinejoin} className={canUndo ? "" : "opacity-40"}>
          <path d="M3 7v6h6" />
          <path d="M3 13a9 9 0 1 0 3-7.7L3 7" />
        </svg>
      </ToolButton>
      <ToolButton title="Cắt">
        <svg width={ICON.size} height={ICON.size} viewBox={ICON.viewBox} fill={ICON.fill} stroke={ICON.stroke} strokeWidth={ICON.strokeWidth} strokeLinecap={ICON.strokeLinecap} strokeLinejoin={ICON.strokeLinejoin}>
          <circle cx="6" cy="6" r="3" />
          <circle cx="6" cy="18" r="3" />
          <path d="M20 4 8.12 15.88M14.47 14.48 20 20M8.12 8.12 12 12" />
        </svg>
      </ToolButton>
      <span className="my-0.5 h-px w-6 bg-fill2" />
      <ToolButton title="Trợ giúp">
        <svg width={ICON.size} height={ICON.size} viewBox={ICON.viewBox} fill={ICON.fill} stroke={ICON.stroke} strokeWidth={ICON.strokeWidth} strokeLinecap={ICON.strokeLinecap} strokeLinejoin={ICON.strokeLinejoin}>
          <circle cx="12" cy="12" r="9" />
          <path d="M9.5 9a2.5 2.5 0 0 1 4.5 1.5c0 1.5-2 2-2 3.5" />
          <path d="M12 17h.01" />
        </svg>
      </ToolButton>
    </div>
  );
}
