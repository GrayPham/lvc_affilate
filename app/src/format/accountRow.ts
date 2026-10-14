// Suy TRANG THAI GOP cua mot tai khoan tu cac co rieng le cua `AccountStatus` (Dola) — THUAN,
// test doc lap. Tach khoi `format/account.ts` (file do la validate + dinh dang tung truong) vi day
// la mot phep SUY LUAN moi cho man Tai khoan dung lai (bang giau du lieu, loc theo trang thai).
//
// CHI suy tu truong CO THAT trong `AccountStatus` — khong bia trang thai nao khong co du lieu nen
// (vd "đang chạy"/"chết" trong ban phac thao: backend chua cap co nao cho hai trang thai do, nen
// KHONG dua vao; them sau khi co du lieu that).
import type { AccountStatus } from "../ipc/types";
import { formatRemaining } from "./account";

/** Khoa trang thai gop — dung lam gia tri loc (chip) va khoa tra nhan/mau. */
export type AccountState =
  | "active"
  | "relogin"
  | "resting"
  | "limited"
  | "no_credit"
  | "unknown";

export const ACCOUNT_STATE_VI: Record<AccountState, string> = {
  active: "Hoạt động",
  relogin: "Cần đăng nhập lại",
  resting: "Đang nghỉ",
  limited: "Bị giới hạn",
  no_credit: "Hết credit",
  unknown: "Chưa rõ",
};

/** Mau cham + chu cho tung trang thai (nen toi). */
export const ACCOUNT_STATE_CLASS: Record<AccountState, { dot: string; text: string }> = {
  active: { dot: "bg-emerald-400", text: "text-ok" },
  relogin: { dot: "bg-amber-400", text: "text-accent-text" },
  resting: { dot: "bg-sky-400", text: "text-sky-400" },
  limited: { dot: "bg-fuchsia-400", text: "text-fuchsia-400" },
  no_credit: { dot: "bg-orange-400", text: "text-orange-400" },
  unknown: { dot: "bg-neutral-500", text: "text-ink2" },
};

/** Thu tu hien chip loc (trai -> phai). */
export const ACCOUNT_STATE_ORDER: readonly AccountState[] = [
  "active",
  "relogin",
  "resting",
  "limited",
  "no_credit",
  "unknown",
];

type StateRow = Pick<
  AccountStatus,
  | "loginOk"
  | "loginCheckedAt"
  | "cooling"
  | "rateLimited"
  | "quotaBlocked"
>;

/**
 * Trang thai gop. Thu tu uu tien CO CHU DICH (cai can nguoi dung xu ly truoc dung truoc):
 * mat phien > het credit > bi gioi han > dang nghi > chua kiem tra > hoat dong.
 */
export function accountState(row: StateRow): AccountState {
  if (row.loginCheckedAt > 0 && !row.loginOk) {
    return "relogin";
  }
  if (row.quotaBlocked) {
    return "no_credit";
  }
  if (row.rateLimited) {
    return "limited";
  }
  if (row.cooling) {
    return "resting";
  }
  if (row.loginCheckedAt <= 0) {
    return "unknown";
  }
  return "active";
}

type DetailRow = StateRow &
  Pick<
    AccountStatus,
    | "limitReason"
    | "quotaReason"
    | "cooldownUntil"
    | "rateLimitedUntil"
    | "quotaBlockedUntil"
  >;

/**
 * Dong giai thich NGAN duoi nhan trang thai (vd "Nghỉ 5p", "Phiên hết hạn — đăng nhập lại để chạy
 * tiếp"); chuoi rong = khong co gi them de noi.
 */
export function accountStateDetail(row: DetailRow, now: number): string {
  const state = accountState(row);
  if (state === "relogin") {
    return "Phiên hết hạn — đăng nhập lại để chạy tiếp";
  }
  if (state === "no_credit") {
    return row.quotaReason.trim() !== "" ? row.quotaReason : formatRemaining(row, now);
  }
  if (state === "limited") {
    return row.limitReason.trim() !== "" ? row.limitReason : formatRemaining(row, now);
  }
  if (state === "resting") {
    return formatRemaining(row, now);
  }
  if (state === "unknown") {
    return "Chưa kiểm tra lần nào";
  }
  return "";
}

/** Nhan loai proxy gon cho cot "Proxy / Trình duyệt" — `proxySet=false` = dung IP may. */
export function proxyKindLabel(proxy: string, proxySet: boolean): string {
  if (!proxySet || proxy.trim() === "" || proxy.trim().toLowerCase() === "direct") {
    return "IP máy";
  }
  return "Cố định";
}
