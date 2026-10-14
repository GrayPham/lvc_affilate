// Yeu cau "mo do thi nay len canvas" phat tu MOT TRANG KHAC (vd Kho workflow o tab Kịch bản).
//
// Vi sao can mot store rieng: tu khi co nhieu tab workflow, `graph` store nam sau
// `GraphStoreProvider` cua TUNG tab (xem `store/graph.tsx`) — trang ngoai khong voi toi duoc, va
// goi thang vao se nem loi (da tung lam TRANG ca app). Thay vao do trang ngoai dat mot YEU CAU o
// day; `WorkflowTabs` (noi duy nhat biet tab nao dang xem) nhan va nap vao dung tab do.
import { create } from "zustand";

import type { Graph } from "../ipc/types";

/** Mot yeu cau mo. `workflowId` rong = do thi chua gan voi workflow da luu nao (vd dung tu mau). */
export interface OpenRequest {
  graph: Graph;
  workflowId: string;
  workflowName: string;
  /** `true` = da co trong "Workflow của tôi" (mo lai ban da luu, khong phai dung mau). */
  saved: boolean;
}

interface OpenWorkflowState {
  pending: OpenRequest | null;
  /** Trang ngoai goi. Ghi de yeu cau cu neu co — nguoi dung bam hai cai lien tiep thi cai SAU thang. */
  request: (req: OpenRequest) => void;
  /** `WorkflowTabs` goi sau khi da nap xong. */
  consume: () => void;
}

export const useOpenWorkflow = create<OpenWorkflowState>((set) => ({
  pending: null,
  request: (pending) => set({ pending }),
  consume: () => set({ pending: null }),
}));
