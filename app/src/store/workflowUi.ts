// Store UI cuc bo cho canvas Workflow (P-lumina): trang thai mo cua "Kho mau" (TemplateGallery, so
// huu boi RunBar) va menu them node (Palette popover, so huu boi WorkflowCanvas) — de thanh cong cu
// trai (CanvasToolbar) mo duoc ca hai ma khong phai xau prop qua nhieu tang. Chi la trang thai
// hien thi, khong dung vao du lieu do thi.
import { create } from "zustand";

/** Muc tieu cho bang "Chụp khung hình" (Lumina): video nguon + vi tri node de dat anh moi. */
export interface CaptureTarget {
  nodeId: string;
  src: string;
  label: string;
  x: number;
  y: number;
}

interface WorkflowUiState {
  /** "Kho mau" (TemplateGallery) dang mo — RunBar render, CanvasToolbar mo. */
  templatesOpen: boolean;
  /** Menu them node (Palette popover) dang mo — WorkflowCanvas render, CanvasToolbar bat/tat. */
  addMenuOpen: boolean;
  /** Bang chup khung hinh dang mo cho node nao (null = dong). */
  captureTarget: CaptureTarget | null;
  openTemplates: () => void;
  closeTemplates: () => void;
  toggleAddMenu: () => void;
  openAddMenu: () => void;
  closeAddMenu: () => void;
  openCapture: (t: CaptureTarget) => void;
  closeCapture: () => void;
}

export const useWorkflowUi = create<WorkflowUiState>((set) => ({
  templatesOpen: false,
  addMenuOpen: false,
  captureTarget: null,
  openTemplates: () => set({ templatesOpen: true }),
  closeTemplates: () => set({ templatesOpen: false }),
  toggleAddMenu: () => set((s) => ({ addMenuOpen: !s.addMenuOpen })),
  openAddMenu: () => set({ addMenuOpen: true }),
  closeAddMenu: () => set({ addMenuOpen: false }),
  openCapture: (t) => set({ captureTarget: t }),
  closeCapture: () => set({ captureTarget: null }),
}));
