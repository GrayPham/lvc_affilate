// Store zustand cho man "Cài đặt" (Task 13): `draft` (dang sua tren form) / `saved` (gia tri
// server xac nhan lan doc/luu gan nhat) / `warnings` (server tra ve tu `settings_set`, xem
// `crates/lvc-app/src/commands/settings.rs::settings_set_impl`) / `dirty` (draft != saved). Tach
// rieng khoi `SettingsForm.tsx` (component) de test duoc doc lap khong can render React — hai ham
// THUAN `validateSettings`/`toAppSettings` van nam o `SettingsForm.tsx` (xuat cung component,
// import lai o day cho `save()`; xem doc trong file do vi sao — cung mot ly do
// `store/graph.ts` dung `graph/rules.ts::isValidConnection` thay vi tu viet lai).
import { create } from "zustand";

import { settingsGet, settingsSet } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { AppSettings } from "../ipc/types";
import { toAppSettings } from "../components/SettingsForm";

function isDirty(draft: AppSettings | null, saved: AppSettings | null): boolean {
  if (!draft || !saved) {
    return false;
  }
  return JSON.stringify(draft) !== JSON.stringify(saved);
}

interface SettingsState {
  draft: AppSettings | null;
  saved: AppSettings | null;
  warnings: string[];
  loading: boolean;
  saving: boolean;
  error: string | null;
  dirty: boolean;

  /** Doc `settings_get()` — goi luc `SettingsPage` mount. */
  load: () => Promise<void>;
  /** Gop `patch` vao `draft` hien tai (khong dong gi neu chua `load()` xong). */
  setDraft: (patch: Partial<AppSettings>) => void;
  /**
   * Chuan hoa `draft` qua `toAppSettings` roi goi `settings_set` — cap nhat `saved`/`warnings` khi
   * thanh cong. Tra `true`/`false` de `SettingsForm` biet co hien thong bao "Đã lưu" hay khong.
   */
  save: () => Promise<boolean>;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  draft: null,
  saved: null,
  warnings: [],
  loading: false,
  saving: false,
  error: null,
  dirty: false,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const s = await settingsGet();
      set({ draft: s, saved: s, loading: false, dirty: false, warnings: [] });
    } catch (e) {
      set({ loading: false, error: errorText(e) });
    }
  },

  setDraft: (patch) => {
    const { draft, saved } = get();
    if (!draft) {
      return;
    }
    const next: AppSettings = { ...draft, ...patch };
    set({ draft: next, dirty: isDirty(next, saved) });
  },

  save: async () => {
    const { draft } = get();
    if (!draft) {
      return false;
    }
    set({ saving: true, error: null });
    try {
      const normalized = toAppSettings(draft);
      const warnings = await settingsSet(normalized);
      set({ draft: normalized, saved: normalized, warnings, saving: false, dirty: false });
      return true;
    } catch (e) {
      set({ saving: false, error: errorText(e) });
      return false;
    }
  },
}));
