// Logic THUAN cho hop thoai "Đang kiểm tra N tài khoản" (CheckProgressDialog) — tach khoi React de
// test duoc: uoc luong thoi gian con lai + anh xa ket qua kiem tra sang nhan/mau hang.
import type { CredentialStatus } from "../ipc/types";

/** Giai doan cua MOT hang trong lan kiem tra. */
export type CheckPhase = "waiting" | "checking" | "done" | "failed";

export interface RowView {
  /** Ky hieu hien o o tron ben trai. */
  icon: string;
  /** Nhan trang thai ben phai. */
  label: string;
  /** Lop mau chu (token theo theme). */
  colorClass: string;
}

/**
 * Hang -> ky hieu/nhan/mau. `done` lay theo `status` that tra ve (OK = Đạt, con lai = nhan trang
 * thai do). `failed` = loi mang/he thong khi goi kiem, khac voi "kiem xong va ra trang thai xau".
 */
export function rowView(phase: CheckPhase, status?: CredentialStatus): RowView {
  switch (phase) {
    case "waiting":
      return { icon: "·", label: "Chờ", colorClass: "text-ink3" };
    case "checking":
      return { icon: "…", label: "Đang kiểm tra", colorClass: "text-accent-text" };
    case "failed":
      return { icon: "✕", label: "Lỗi kiểm tra", colorClass: "text-rose" };
    default:
      // done
      if (status === "ok") {
        return { icon: "✓", label: "Đạt", colorClass: "text-ok" };
      }
      return { icon: "!", label: STATUS_LABEL[status ?? "unknown"], colorClass: "text-warn" };
  }
}

/** Nhan tieng Viet cho `CredentialStatus` khi KHONG dat (hang `done` + status != ok). */
const STATUS_LABEL: Record<CredentialStatus, string> = {
  ok: "Đạt",
  expired: "Hết hạn",
  banned: "Bị khoá",
  quota: "Hết hạn mức",
  unknown: "Chưa rõ",
};

/**
 * Uoc luong thoi gian con lai, dang chuoi "khoảng N giây nữa" / "khoảng N phút nữa".
 *
 * Dua tren thoi gian trung binh cua cac hang DA XONG — chua xong hang nao thi chua doan duoc.
 * `null` = khong hien (chua co du lieu, hoac da xong het).
 */
export function etaText(doneCount: number, total: number, elapsedMs: number): string | null {
  const remaining = total - doneCount;
  if (doneCount <= 0 || remaining <= 0) {
    return null;
  }
  const perItem = elapsedMs / doneCount;
  const leftMs = perItem * remaining;
  const leftSec = Math.round(leftMs / 1000);
  if (leftSec < 60) {
    return `khoảng ${Math.max(1, leftSec)} giây nữa`;
  }
  return `khoảng ${Math.round(leftSec / 60)} phút nữa`;
}

/** `"2/3 xong"`. */
export function progressText(doneCount: number, total: number): string {
  return `${doneCount}/${total} xong`;
}

/** Phan tram da xong, 0..100 — cho thanh do. */
export function progressPercent(doneCount: number, total: number): number {
  if (total <= 0) {
    return 0;
  }
  return Math.round((doneCount / total) * 100);
}
