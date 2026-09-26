// Store zustand cho man Tai khoan (Task 14) — danh sach + cac hanh dong tren tung tai khoan.
// `busy`: DEM THAM CHIEU (ref-count) theo ten — `busy[name] > 0` nghia la CO IT NHAT mot lenh IPC
// dang chay cho tai khoan do (disable nut/toggle cua dung hang do trong luc cho, khong khoa ca
// bang). Bat buoc phai la dem, khong phai boolean don: hai hanh dong tren CUNG mot tai khoan co
// the chong lan (vd bam "Kiem tra" roi mo o sua proxy bam "Luu" truoc khi lenh dau xong) — voi
// boolean don, hanh dong nao xong TRUOC se xoa `busy[name]` mat cho ca hanh dong con lai dang
// chay, tam thoi mo lai cac nut le ra van phai bi khoa (review task-14-review.md, Important #2).
// `error`: loi CHUNG gan nhat (hien nay chi tu `load()` — cac hanh dong tren tung dong RETHROW loi
// cua chung de component tu quyet dinh hien inline o dau, xem `AccountRow`/`ProxyEditor`/
// `CookieImport`).
import { create } from "zustand";

import {
  accountDelete,
  accountImportCookieFiles,
  accountImportCookies,
  accountList,
  accountSetProxy,
  accountSetScheduling,
  accountVerify,
  type BulkCookieItem,
} from "../ipc/commands";
import { describeAccountError } from "../format/account";
import type { AccountStatus, ImportCookiesReport, VerifyReport } from "../ipc/types";

/** Tang dem `busy[name]` len 1 (tao moi voi gia tri 1 neu chua co). */
function incBusy(busy: Record<string, number>, name: string): Record<string, number> {
  return { ...busy, [name]: (busy[name] ?? 0) + 1 };
}

/** Giam dem `busy[name]` di 1 — chi XOA HAN key khoi map khi dem ve 0 (cac hanh dong khac cho
 * CUNG ten van con dang chay thi phai giu key voi dem duong). */
function decBusy(busy: Record<string, number>, name: string): Record<string, number> {
  const current = busy[name] ?? 0;
  if (current <= 1) {
    if (!(name in busy)) {
      return busy;
    }
    const next = { ...busy };
    delete next[name];
    return next;
  }
  return { ...busy, [name]: current - 1 };
}

interface AccountsState {
  rows: AccountStatus[];
  loading: boolean;
  error: string | null;
  busy: Record<string, number>;

  /** Tai lai toan bo danh sach — goi sau MOI hanh dong ghi thanh cong (yeu cau brief). */
  load: () => Promise<void>;
  importCookies: (name: string, cookieText: string, proxy: string) => Promise<ImportCookiesReport>;
  /** Nạp cookie nhiều file cùng lúc — trả kết quả từng file; tải lại danh sách sau khi xong. */
  importCookieFiles: (paths: string[], proxy: string) => Promise<BulkCookieItem[]>;
  verify: (name: string) => Promise<VerifyReport>;
  /** Optimistic: lat `scheduling` ngay tai client, revert CHI DONG NAY neu server tu choi (khong
   * duoc ghi de toan bo `rows` bang mot snapshot cu — review task-14-review.md, Important #1). */
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
    set((s) => ({ busy: incBusy(s.busy, name) }));
    try {
      const report = await accountImportCookies(name, cookieText, proxy);
      await get().load();
      return report;
    } finally {
      set((s) => ({ busy: decBusy(s.busy, name) }));
    }
  },

  importCookieFiles: async (paths, proxy) => {
    set({ loading: true });
    try {
      const items = await accountImportCookieFiles(paths, proxy);
      await get().load();
      return items;
    } finally {
      set({ loading: false });
    }
  },

  verify: async (name) => {
    set((s) => ({ busy: incBusy(s.busy, name) }));
    try {
      const report = await accountVerify(name);
      await get().load();
      return report;
    } finally {
      set((s) => ({ busy: decBusy(s.busy, name) }));
    }
  },

  setScheduling: async (name, on) => {
    // Snapshot CHI dong nay (khong phai ca mang) — xem doc `setScheduling` o tren.
    const prevRow = get().rows.find((r) => r.name === name);
    set((s) => ({
      rows: s.rows.map((r) => (r.name === name ? { ...r, scheduling: on } : r)),
      busy: incBusy(s.busy, name),
    }));
    try {
      await accountSetScheduling(name, on);
      await get().load();
    } catch (e) {
      // Chi ghi de LAI dong `name` bang gia tri truoc-optimistic cua no, dua tren `rows` HIEN TAI
      // (co the da doi do mot mutation KHAC — vd tren tai khoan khac — thanh cong va `load()`
      // trong luc lenh nay dang cho) — khong dung `set({rows: prevRows})` voi snapshot ca mang cu,
      // se xoa mat cap nhat that su do cua dong khac.
      if (prevRow) {
        set((s) => ({ rows: s.rows.map((r) => (r.name === name ? prevRow : r)) }));
      }
      throw e;
    } finally {
      set((s) => ({ busy: decBusy(s.busy, name) }));
    }
  },

  setProxy: async (name, proxy) => {
    set((s) => ({ busy: incBusy(s.busy, name) }));
    try {
      await accountSetProxy(name, proxy);
      await get().load();
    } finally {
      set((s) => ({ busy: decBusy(s.busy, name) }));
    }
  },

  remove: async (name) => {
    set((s) => ({ busy: incBusy(s.busy, name) }));
    try {
      const deleted = await accountDelete(name);
      await get().load();
      return deleted;
    } finally {
      set((s) => ({ busy: decBusy(s.busy, name) }));
    }
  },
}));
