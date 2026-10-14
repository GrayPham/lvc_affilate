// Store zustand cho man "Cài đặt" (Task 13): `draft` (dang sua tren form) / `saved` (gia tri
// server xac nhan lan doc/luu gan nhat) / `warnings` (server tra ve tu `settings_set`, xem
// `crates/lvc-app/src/commands/settings.rs::settings_set_impl`) / `dirty` (draft != saved).
//
// `toAppSettings`/`THREADS_MAX`/`THREADS_MIN` nam O DAY de tranh store<->component gan-vong-tron.
// `validateSettings` o `components/settings/validate.ts` (THUAN, import `THREADS_MAX`/`THREADS_MIN`
// tu day) — man `SettingsPage` goi no de chan tu-luu khi gia tri ro rang sai.
import { create } from "zustand";

import { settingsGet, settingsSet } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { AppSettings } from "../ipc/types";

/** `lvc_core::settings::THREADS_MAX` (crates/lvc-core/src/settings.rs) — tran tren `threads`. */
export const THREADS_MAX = 64;
export const THREADS_MIN = 1;

/** Chuan hoa phong thu truoc khi goi `settingsSet` (kep threads ve [`THREADS_MIN`,
 * `THREADS_MAX`], trim hai duong dan) — ke ca khi `validateSettings` da qua, dam bao khong bao
 * gio gui mot gia tri ma chinh server se phai tu sua. */
export function toAppSettings(draft: AppSettings): AppSettings {
  const roundedThreads = Number.isFinite(draft.threads) ? Math.round(draft.threads) : THREADS_MIN;
  const threads = Math.min(THREADS_MAX, Math.max(THREADS_MIN, roundedThreads));
  return {
    ...draft,
    threads,
    accountsDir: draft.accountsDir.trim(),
    outputDir: draft.outputDir.trim(),
  };
}

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
   * thanh cong. Tra `true`/`false` de noi goi biet lan luu co thanh cong hay khong.
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
