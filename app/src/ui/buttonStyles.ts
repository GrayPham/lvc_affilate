// Style nut dung chung toan app — chuan LVC Fox (dong bo apps/manager + templates/lvc-ui-kit).
// Nut CHINH: gradient vang kim `lvc-btn-primary` (chu toi), noi bat ro so voi nut phu.
// Nut PHU (`NEUTRAL`): vien + nen panel, hover sang hon. Nut NGUY HIEM (`DANGER`): vien/chu do.
// Chi dung TOKEN (bg-panel/border-line/text-fg/text-danger...), KHONG dung palette tho (amber-*/red-*).
// Bo goc tron (`rounded-full`) + chuyen dong/active dinh nghia global o `index.css`.
//
// Hai mat do (giu kich thuoc cu, chi doi mau/bo goc):
// - `Xs` (11px) — dong trong bang day dac.
// - `Sm` (12px) — nut toolbar/trang (AccountsPage...).
const BASE =
  "inline-flex items-center justify-center gap-1.5 whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none";

export const BTN_XS = `${BASE} rounded-full px-2.5 py-1 text-[11px] font-medium`;
export const BTN_NEUTRAL_XS = `${BTN_XS} border border-line bg-panel text-fg hover:bg-hover`;
export const BTN_DANGER_XS = `${BTN_XS} border border-danger/35 bg-danger/5 text-danger hover:border-danger/55 hover:bg-danger/10`;
export const BTN_PRIMARY_XS = `${BTN_XS} lvc-btn-primary font-semibold`;

export const BTN_SM = `${BASE} rounded-full px-3.5 py-2 text-xs font-semibold`;
export const BTN_NEUTRAL_SM = `${BTN_SM} border border-line bg-panel font-medium text-fg hover:bg-hover`;
export const BTN_DANGER_SM = `${BTN_SM} border border-danger/35 bg-danger/5 font-medium text-danger hover:border-danger/55 hover:bg-danger/10`;
export const BTN_PRIMARY_SM = `${BTN_SM} lvc-btn-primary`;

/** Nut bieu tuong TRON tren thanh tren (doi sang/toi, cai dat, chuong) — dung chung voi man khoa. */
export const BTN_ICON_ROUND =
  "inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-line bg-panel text-muted shadow-card transition-colors hover:bg-hover hover:text-fg";
