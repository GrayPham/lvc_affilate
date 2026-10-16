import { create } from "zustand";

// Zustand store toi thieu cho khung UI (Task 11) — tab dang mo tren thanh dieu huong
// tren cung. Task 12-14 se them store rieng cho tung mien du lieu (workflow dang sua,
// danh sach run/task, cai dat) trong cung thu muc `src/store/`; xem huong dan trong
// task-11-report.md. "scripts" (P3c Task 2) them SAU "workflow" — xem `App.tsx::TABS`.
export type TabId = "workflow" | "scripts" | "tasks" | "settings" | "accounts" | "credentials";

/** Theme giao dien — khop ban ban giao `references/lvc-ui-design` (cot "Tối"/"Sáng"). */
export type Theme = "dark" | "light";

const THEME_KEY = "lvc.ui.theme";
const SIDEBAR_KEY = "lvc.ui.sidebarCollapsed";

/** Doc gia tri da luu; hong/khong co -> mac dinh. `localStorage` co the nem (webview khoa). */
function readStored<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return allowed.includes(raw as T) ? (raw as T) : fallback;
  } catch {
    return fallback;
  }
}

/** Doc co bool da luu ("1"/"0"); hong/khong co -> mac dinh. */
function readBool(key: string, fallback: boolean): boolean {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? fallback : raw === "1";
  } catch {
    return fallback;
  }
}

function store(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Khong luu duoc thi thoi — chi mat ghi nho giua cac lan mo, khong phai loi can bao.
  }
}

/** Bat/bo class `dark` tren <html> — chuan LVC Fox (xem `index.css`: `:root` sang, `.dark` toi).
    Khong con dung `data-theme`. */
export function applyTheme(theme: Theme): void {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

const initialTheme = readStored<Theme>(THEME_KEY, ["dark", "light"], "dark");
applyTheme(initialTheme);

interface UiState {
  /** `true` = video xem truoc trong node PHAT IM TIENG.
   *
   * Mac dinh `true` vi trinh duyet CHAN tu phat khi co tieng — re chuot vao ma may ruc tieng cung
   * la kieu lam nguoi dung giat minh. Bam nut loa (mot cu cham that) moi mo tieng, va luu lai cho
   * cac lan sau. */
  previewMuted: boolean;
  togglePreviewMuted: () => void;
  activeTab: TabId;
  setActiveTab: (tab: TabId) => void;
  /** Thu gon sidebar con dai icon (nut tron noi o mep phai thanh ben — xem `App.tsx`). */
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
}

export const useUiStore = create<UiState>((set, get) => ({
  previewMuted: readBool("lvc.ui.previewMuted", true),
  togglePreviewMuted: () =>
    set((st) => {
      const next = !st.previewMuted;
      store("lvc.ui.previewMuted", next ? "1" : "0");
      return { previewMuted: next };
    }),
  activeTab: "workflow",
  setActiveTab: (tab) => set({ activeTab: tab }),

  sidebarCollapsed: readStored(SIDEBAR_KEY, ["1", "0"], "0") === "1",
  toggleSidebar: () =>
    set((s) => {
      const next = !s.sidebarCollapsed;
      store(SIDEBAR_KEY, next ? "1" : "0");
      return { sidebarCollapsed: next };
    }),

  theme: initialTheme,
  setTheme: (theme) => {
    applyTheme(theme);
    store(THEME_KEY, theme);
    set({ theme });
  },
  toggleTheme: () => get().setTheme(get().theme === "dark" ? "light" : "dark"),
}));
