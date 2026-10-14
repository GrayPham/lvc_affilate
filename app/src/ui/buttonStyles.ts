// Style nut dung chung toan app — thay cac hang `BTN`/`BTN_NEUTRAL`/`BTN_PRIMARY`/`BTN_DANGER`
// truoc day dinh nghia LAP LAI o tung component (SceneRow/SceneTable/ScriptList/
// ScriptVideoConfig...) bang MOT noi duy nhat, accent amber/vang (hoa hop voi sidebar vang-den) +
// bo goc hao phong (`rounded-lg`) + hover/active/transition (dinh nghia global o `index.css`, chi
// can khai bao mau/bong o day).
// Nut CHINH (`PRIMARY`) co bong mau (shadow-amber) de "noi bat" ro rang so voi nut phu
// (`NEUTRAL`/`DANGER`, chi vien + bong trung tinh) — dung y "nut uu tien phai noi bat" thay vi
// moi nut cung mot trong luong thi giac.
//
// Nen TOI toan app (dong bo voi sidebar, khong con trang/dialog nao con nen sang) — `NEUTRAL`/
// `DANGER` dung surface toi (`bg-fill`/`border-line`) giong `RunBar`/`SidebarAccountFooter`
// thay vi `bg-white`/`border-slate-*` cu.
//
// Hai mat do (khop kich thuoc da dung san, chi doi mau sac/bo goc — KHONG doi layout goc):
// - `Xs` (11px, padding sit) — dong trong bang day dac (SceneRow).
// - `Sm` (12px) — nut toolbar/trang (SceneTable, ScriptList, ScriptVideoConfig, AccountsPage...).
const BASE = "disabled:cursor-not-allowed disabled:opacity-50 disabled:shadow-none";

export const BTN_XS = `${BASE} rounded-lg px-2.5 py-1 text-[11px] font-medium`;
export const BTN_NEUTRAL_XS = `${BTN_XS} border border-line bg-fill text-ink hover:border-line2 hover:bg-fill2`;
export const BTN_DANGER_XS = `${BTN_XS} border border-red-500/30 bg-red-500/5 text-rose hover:border-red-500/50 hover:bg-red-500/10`;
export const BTN_PRIMARY_XS = `${BTN_XS} bg-accent text-accent-ink shadow-sm shadow-amber-600/25 hover:bg-amber-500 hover:shadow-md hover:shadow-amber-500/35 active:bg-amber-700`;

export const BTN_SM = `${BASE} rounded-lg px-3.5 py-2 text-xs font-semibold`;
export const BTN_NEUTRAL_SM = `${BTN_SM} border border-line bg-fill font-medium text-ink hover:border-line2 hover:bg-fill2`;
export const BTN_DANGER_SM = `${BTN_SM} border border-red-500/30 bg-red-500/5 font-medium text-rose hover:border-red-500/50 hover:bg-red-500/10`;
export const BTN_PRIMARY_SM = `${BTN_SM} bg-accent text-accent-ink shadow-md shadow-amber-600/30 hover:bg-amber-500 hover:shadow-lg hover:shadow-amber-500/40 active:bg-amber-700`;
