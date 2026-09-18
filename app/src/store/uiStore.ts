import { create } from "zustand";

// Zustand store toi thieu cho khung UI (Task 11) — tab dang mo tren thanh dieu huong
// tren cung. Task 12-14 se them store rieng cho tung mien du lieu (workflow dang sua,
// danh sach run/task, cai dat) trong cung thu muc `src/store/`; xem huong dan trong
// task-11-report.md. "scripts" (P3c Task 2) them SAU "workflow" — xem `App.tsx::TABS`.
export type TabId = "workflow" | "scripts" | "tasks" | "settings" | "accounts";

interface UiState {
  activeTab: TabId;
  setActiveTab: (tab: TabId) => void;
}

export const useUiStore = create<UiState>((set) => ({
  activeTab: "workflow",
  setActiveTab: (tab) => set({ activeTab: tab }),
}));
