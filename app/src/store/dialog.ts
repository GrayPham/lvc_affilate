// He thong dialog trong-app (P-lumina): thay `window.confirm/prompt/alert` mac dinh cua WebView2
// (xau, hien "localhost says"). Dung mot hang doi MOT dialog dang mo; cac helper tra Promise de
// goi nhu window.* cu (await). `<DialogHost/>` (mount o App) ve giao dien toi + resolve promise.
import { create } from "zustand";

export interface ConfirmReq {
  kind: "confirm";
  title?: string;
  message: string;
  okText?: string;
  cancelText?: string;
  danger?: boolean;
  resolve: (ok: boolean) => void;
}
export interface PromptReq {
  kind: "prompt";
  title?: string;
  message?: string;
  defaultValue?: string;
  placeholder?: string;
  multiline?: boolean;
  okText?: string;
  cancelText?: string;
  resolve: (value: string | null) => void;
}
export interface AlertReq {
  kind: "alert";
  title?: string;
  message: string;
  okText?: string;
  danger?: boolean;
  resolve: () => void;
}
export type DialogReq = ConfirmReq | PromptReq | AlertReq;

interface DialogState {
  current: DialogReq | null;
  open: (req: DialogReq) => void;
  close: () => void;
}

export const useDialogStore = create<DialogState>((set) => ({
  current: null,
  open: (req) => set({ current: req }),
  close: () => set({ current: null }),
}));

/** Hoi Yes/No — thay `window.confirm`. */
export function confirmDialog(opts: Omit<ConfirmReq, "kind" | "resolve">): Promise<boolean> {
  return new Promise((resolve) => {
    useDialogStore.getState().open({ kind: "confirm", ...opts, resolve });
  });
}

/** Nhap chuoi — thay `window.prompt`. `null` = huy. */
export function promptDialog(opts: Omit<PromptReq, "kind" | "resolve">): Promise<string | null> {
  return new Promise((resolve) => {
    useDialogStore.getState().open({ kind: "prompt", ...opts, resolve });
  });
}

/** Thong bao — thay `window.alert`. */
export function alertDialog(opts: Omit<AlertReq, "kind" | "resolve">): Promise<void> {
  return new Promise((resolve) => {
    useDialogStore.getState().open({ kind: "alert", ...opts, resolve });
  });
}
