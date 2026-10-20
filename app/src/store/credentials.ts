// Store zustand cho man "Tài khoản AI" (P4c) — danh sach `CredentialView` + hanh dong tren tung
// tai khoan. Mo phong `store/accounts.ts`:
// - `busy`: DEM THAM CHIEU (ref-count) theo `id` — `busy[id] > 0` nghia la co it nhat mot lenh IPC
//   dang chay cho tai khoan do (disable nut/toggle cua dung dong do, khong khoa ca bang). Phai la
//   dem chu khong boolean don de hai hanh dong chong lan tren cung mot dong khong xoa busy cua nhau.
// - Moi hanh dong GHI goi `load()` lai sau khi thanh cong (yeu cau brief).
//
// AN TOAN BI MAT: store nay KHONG BAO GIO luu hay tra lai bi mat nguyen van. `rows` chi chua
// `CredentialView` (co `secretHint` da che). `add`/`update` nhan `secret` va chuyen THANG xuong
// IPC (ghi-mot-chieu) — khong luu vao state, khong log.
import { create } from "zustand";

import {
  credentialAdd,
  credentialDelete,
  credentialList,
  credentialSetDailyLimit,
  credentialSetNodeTypes,
  credentialUpdate,
  credentialVerify,
  loginCookieRelogin,
} from "../ipc/commands";
import { describeCredentialError } from "../format/credential";
import type { CredentialView, VerifyOutcome } from "../ipc/types";

/** Tang dem `busy[id]` len 1 (tao moi voi gia tri 1 neu chua co). */
function incBusy(busy: Record<string, number>, id: string): Record<string, number> {
  return { ...busy, [id]: (busy[id] ?? 0) + 1 };
}

/** Giam dem `busy[id]` di 1 — chi XOA HAN key khi dem ve 0. */
function decBusy(busy: Record<string, number>, id: string): Record<string, number> {
  const current = busy[id] ?? 0;
  if (current <= 1) {
    if (!(id in busy)) {
      return busy;
    }
    const next = { ...busy };
    delete next[id];
    return next;
  }
  return { ...busy, [id]: current - 1 };
}

/** Tham so `add` — `secret` ghi-mot-chieu (khong bao gio luu vao state). */
export interface CredentialAddInput {
  provider: string;
  kind: string;
  label: string;
  secret: string;
  meta?: Record<string, unknown>;
  proxy?: string;
}

/** Tham so `update` — moi truong `undefined` = khong doi; `secret` ghi-mot-chieu. */
export interface CredentialUpdateInput {
  secret?: string;
  label?: string;
  proxy?: string;
  enabled?: boolean;
}

interface CredentialsState {
  rows: CredentialView[];
  loading: boolean;
  error: string | null;
  busy: Record<string, number>;

  /** Tai lai toan bo danh sach — goi sau MOI hanh dong ghi thanh cong. */
  load: () => Promise<void>;
  /** Them tai khoan moi — tra ve `CredentialView` vua tao (chi co `secretHint`). */
  add: (input: CredentialAddInput) => Promise<CredentialView>;
  update: (id: string, patch: CredentialUpdateInput) => Promise<void>;
  verify: (id: string) => Promise<VerifyOutcome>;
  /** "Dang nhap lai": tiem cookie moi vao credential cookie da ton tai (giu profile/node_types). */
  relogin: (id: string, cookieText: string) => Promise<void>;
  /** Dat tag loai node (`video`/`image`/`script`) — node AI chi lay tai khoan/key dung tag. */
  setNodeTypes: (id: string, nodeTypes: string[]) => Promise<void>;
  setDailyLimit: (id: string, limit: number | null) => Promise<void>;
  /** `false` = khong co gi de xoa (idempotent). */
  remove: (id: string) => Promise<boolean>;
}

export const useCredentialsStore = create<CredentialsState>((set, get) => ({
  rows: [],
  loading: false,
  error: null,
  busy: {},

  load: async () => {
    set({ loading: true, error: null });
    try {
      const rows = await credentialList();
      set({ rows, loading: false });
    } catch (e) {
      set({ loading: false, error: describeCredentialError(e) });
    }
  },

  add: async (input) => {
    // `add` chua co `id` de dem busy — component quan ly busy cuc bo cho form (giong CookieImport).
    const view = await credentialAdd(
      input.provider,
      input.kind,
      input.label,
      input.secret,
      input.meta,
      input.proxy,
    );
    await get().load();
    return view;
  },

  update: async (id, patch) => {
    set((s) => ({ busy: incBusy(s.busy, id) }));
    try {
      await credentialUpdate(id, patch.secret, patch.label, patch.proxy, patch.enabled);
      await get().load();
    } finally {
      set((s) => ({ busy: decBusy(s.busy, id) }));
    }
  },

  verify: async (id) => {
    set((s) => ({ busy: incBusy(s.busy, id) }));
    try {
      const outcome = await credentialVerify(id);
      await get().load();
      return outcome;
    } finally {
      set((s) => ({ busy: decBusy(s.busy, id) }));
    }
  },

  relogin: async (id, cookieText) => {
    set((s) => ({ busy: incBusy(s.busy, id) }));
    try {
      await loginCookieRelogin(id, cookieText);
      await get().load();
    } finally {
      set((s) => ({ busy: decBusy(s.busy, id) }));
    }
  },

  setNodeTypes: async (id, nodeTypes) => {
    set((s) => ({ busy: incBusy(s.busy, id) }));
    try {
      await credentialSetNodeTypes(id, nodeTypes);
      await get().load();
    } finally {
      set((s) => ({ busy: decBusy(s.busy, id) }));
    }
  },

  setDailyLimit: async (id, limit) => {
    set((s) => ({ busy: incBusy(s.busy, id) }));
    try {
      await credentialSetDailyLimit(id, limit);
      await get().load();
    } finally {
      set((s) => ({ busy: decBusy(s.busy, id) }));
    }
  },

  remove: async (id) => {
    set((s) => ({ busy: incBusy(s.busy, id) }));
    try {
      const deleted = await credentialDelete(id);
      await get().load();
      return deleted;
    } finally {
      set((s) => ({ busy: decBusy(s.busy, id) }));
    }
  },
}));
