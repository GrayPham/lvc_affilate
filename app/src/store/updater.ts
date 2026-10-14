// Store global (khong phai factory theo tab, khac `store/graph.tsx`) cho auto-update (P8, xem
// docs/superpowers/specs/2026-10-13-auto-update-design.md). Chi goi `@tauri-apps/plugin-updater`
// o DAY — khong noi nao khac trong app duoc goi `check()`/`downloadAndInstall()` truc tiep, giu
// MOT nguon trang thai duy nhat cho ca banner (App.tsx) lan nut thu cong (SettingsPage.tsx).
//
// ADR-R026 (spec): CHI check ngam, KHONG tu tai/cai — phai doi nguoi dung bam "Cai dat". App
// Windows-only: `downloadAndInstall()` thanh cong se TU THOAT app roi chay installer NSIS (installer
// tu khoi dong lai app sau khi cai xong) — khong goi them `relaunch()`/`tauri-plugin-process` nao.
import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type UpdaterPhase =
  | "idle"
  | "checking"
  | "up_to_date"
  | "available"
  | "downloading"
  | "error";

interface UpdaterState {
  phase: UpdaterPhase;
  /** Ban moi (null neu chua check hoac dang la ban moi nhat). Giu nguyen instance de goi
   * `downloadAndInstall()` sau — KHONG serialize ra state khac. */
  update: Update | null;
  /** Noi dung ghi chu phat hanh (tu `latest.json.notes`, co the rong). */
  notes: string | null;
  downloadedBytes: number;
  totalBytes: number | null;
  error: string | null;
  /** Nguoi dung da bam "An" tren banner cho ban nay — khong hien lai cho toi lan check tiep theo
   * tim thay ban MOI HON (so sanh theo `update.version`). */
  dismissedVersion: string | null;

  /** `silent = true` (lan tu kiem tra luc khoi dong): THAT BAI thi im lang, khong bao loi.
   *
   * Khong voi toi duoc kho phat hanh la chuyen thuong (chua cau hinh repo, mat mang, dang offline)
   * — no KHONG phai viec nguoi dung vua yeu cau, nen dap mot banner do len man hinh moi lan mo app
   * chi lam phien. Bam "Kiểm tra cập nhật" trong Cai dat thi loi VAN hien, vi luc do ho dang doi
   * cau tra loi. */
  checkForUpdate: (silent?: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const useUpdaterStore = create<UpdaterState>((set, get) => ({
  phase: "idle",
  update: null,
  notes: null,
  downloadedBytes: 0,
  totalBytes: null,
  error: null,
  dismissedVersion: null,

  checkForUpdate: async (silent = false) => {
    set({ phase: "checking", error: null });
    try {
      const update = await check();
      if (!update) {
        set({ phase: "up_to_date", update: null, notes: null });
        return;
      }
      set({
        phase: "available",
        update,
        notes: update.body ?? null,
        downloadedBytes: 0,
        totalBytes: null,
      });
    } catch (e) {
      if (silent) {
        // Xem doc `checkForUpdate`: lan tu kiem tra khong duoc bao loi.
        set({ phase: "idle", error: null });
        return;
      }
      set({ phase: "error", error: e instanceof Error ? e.message : String(e) });
    }
  },

  install: async () => {
    const { update } = get();
    if (!update) {
      return;
    }
    set({ phase: "downloading", downloadedBytes: 0, totalBytes: null, error: null });
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          set({ totalBytes: event.data.contentLength ?? null });
        } else if (event.event === "Progress") {
          set((s) => ({ downloadedBytes: s.downloadedBytes + event.data.chunkLength }));
        }
        // "Finished": tren Windows app thoat ngay sau do (installer tu chay) — khong con UI nao
        // doc duoc state nay nua, khong can xu ly rieng.
      });
    } catch (e) {
      set({ phase: "error", error: e instanceof Error ? e.message : String(e) });
    }
  },

  dismiss: () => {
    const { update } = get();
    set({ dismissedVersion: update?.version ?? null });
  },
}));
