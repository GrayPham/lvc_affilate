// Store zustand cho kho template (P4d) — danh sach `TemplateMeta` cua MOT pham vi (Public HOAC
// Private, tuy lan `load` gan nhat) + cac helper mong goi thang xuong IPC. Ap dung mot mau len
// canvas KHONG di qua day — TemplateGallery lay `TemplateDoc` (helper `getDoc`) roi goi
// `useGraphStore.loadGraph` (dung duong nhu mo mot workflow da luu, xem `RunBar.handlePicked`).
import { create } from "zustand";

import { templateDelete, templateGet, templateSavePrivate } from "../ipc/commands";
import { templateList } from "../ipc/commands";
import { describeTemplateError } from "../format/template";
import type { Graph, TemplateDoc, TemplateMeta, TemplateScope } from "../ipc/types";

/** Tham so `save` — `id` null = tao mau moi; nguoc lai luu de mau private do. */
export interface TemplateSaveInput {
  id: string | null;
  name: string;
  graph: Graph;
  description?: string;
  thumbnail?: string;
}

interface TemplatesState {
  items: TemplateMeta[];
  loading: boolean;
  error: string | null;

  /** Tai danh sach theo pham vi (khong dat = public+private) — dat lai `items` va map loi sang
   * tieng Viet. `scope="remote"` goi mang (xem `commands.ts::templateList`). */
  load: (scope?: TemplateScope) => Promise<void>;
  /** Lay do thi day du cua mot mau (dung cho "Dùng mẫu"). */
  getDoc: (id: string) => Promise<TemplateDoc>;
  /** Luu mau private (tao moi hoac luu de) — tra ve meta vua luu. */
  save: (input: TemplateSaveInput) => Promise<TemplateMeta>;
  /** Xoa mau private — `false` = khong co gi de xoa (idempotent); bo khoi `items` khi thanh cong. */
  remove: (id: string) => Promise<boolean>;
}

export const useTemplatesStore = create<TemplatesState>((set) => ({
  items: [],
  loading: false,
  error: null,

  load: async (scope) => {
    set({ loading: true, error: null });
    try {
      const items = await templateList(scope);
      set({ items, loading: false });
    } catch (e) {
      set({ loading: false, error: describeTemplateError(e) });
    }
  },

  getDoc: (id) => templateGet(id),

  save: (input) =>
    templateSavePrivate(input.id, input.name, input.graph, input.description, input.thumbnail),

  remove: async (id) => {
    const deleted = await templateDelete(id);
    if (deleted) {
      set((s) => ({ items: s.items.filter((t) => t.id !== id) }));
    }
    return deleted;
  },
}));
