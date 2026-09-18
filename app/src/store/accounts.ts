// Store zustand cho man Tai khoan (Task 14) — danh sach + cac hanh dong tren tung tai khoan.
// `busy`: map ten -> dang co mot lenh IPC chay cho tai khoan do (disable nut/toggle cua dung
// hang do trong luc cho, khong khoa ca bang). `error`: loi CHUNG gan nhat (hien nay chi tu
// `load()` — cac hanh dong tren tung dong RETHROW loi cua chung de component tu quyet dinh hien
// inline o dau, xem `AccountRow`/`ProxyEditor`/`CookieImport`).
import { create } from "zustand";

import {
  accountDelete,
  accountImportCookies,
  accountList,
  accountSetProxy,
  accountSetScheduling,
  accountVerify,
} from "../ipc/commands";
import { describeAccountError } from "../format/account";
import type { AccountStatus, ImportCookiesReport, VerifyReport } from "../ipc/types";

/** Tra ve BAN SAO cua `busy` da bo `name` (khong dung destructuring-rest de tranh bien thua). */
function withoutBusy(busy: Record<string, boolean>, name: string): Record<string, boolean> {
  if (!(name in busy)) {
    return busy;
  }
  const next = { ...busy };
  delete next[name];
  return next;
}

interface AccountsState {
  rows: AccountStatus[];
  loading: boolean;
  error: string | null;
  busy: Record<string, boolean>;

  /** Tai lai toan bo danh sach — goi sau MOI hanh dong ghi thanh cong (yeu cau brief). */
  load: () => Promise<void>;
  importCookies: (name: string, cookieText: string, proxy: string) => Promise<ImportCookiesReport>;
  verify: (name: string) => Promise<VerifyReport>;
  /** Optimistic: lat `scheduling` ngay tai client, revert neu server tu choi. */
  setScheduling: (name: string, on: boolean) => Promise<void>;
  setProxy: (name: string, proxy: string) => Promise<void>;
  /** `false` = khong co gi de xoa (tai khoan da bi xoa boi lan goi truoc — idempotent). */
  remove: (name: string) => Promise<boolean>;
}

export const useAccountsStore = create<AccountsState>((set, get) => ({
  rows: [],
  loading: false,
  error: null,
  busy: {},

  load: async () => {
    set({ loading: true, error: null });
    try {
      const rows = await accountList();
      set({ rows, loading: false });
    } catch (e) {
      set({ loading: false, error: describeAccountError(e) });
    }
  },

  importCookies: async (name, cookieText, proxy) => {
    set((s) => ({ busy: { ...s.busy, [name]: true } }));
    try {
      const report = await accountImportCookies(name, cookieText, proxy);
      await get().load();
      return report;
    } finally {
      set((s) => ({ busy: withoutBusy(s.busy, name) }));
    }
  },

  verify: async (name) => {
    set((s) => ({ busy: { ...s.busy, [name]: true } }));
    try {
      const report = await accountVerify(name);
      await get().load();
      return report;
    } finally {
      set((s) => ({ busy: withoutBusy(s.busy, name) }));
    }
  },

  setScheduling: async (name, on) => {
    const prevRows = get().rows;
    set((s) => ({
      rows: s.rows.map((r) => (r.name === name ? { ...r, scheduling: on } : r)),
      busy: { ...s.busy, [name]: true },
    }));
    try {
      await accountSetScheduling(name, on);
      await get().load();
    } catch (e) {
      set({ rows: prevRows });
      throw e;
    } finally {
      set((s) => ({ busy: withoutBusy(s.busy, name) }));
    }
  },

  setProxy: async (name, proxy) => {
    set((s) => ({ busy: { ...s.busy, [name]: true } }));
    try {
      await accountSetProxy(name, proxy);
      await get().load();
    } finally {
      set((s) => ({ busy: withoutBusy(s.busy, name) }));
    }
  },

  remove: async (name) => {
    set((s) => ({ busy: { ...s.busy, [name]: true } }));
    try {
      const deleted = await accountDelete(name);
      await get().load();
      return deleted;
    } finally {
      set((s) => ({ busy: withoutBusy(s.busy, name) }));
    }
  },
}));
