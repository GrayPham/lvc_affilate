// Ham THUAN cho "Kiểu kết nối" (proxy) — dung o `EditAccountDialog` (mot tai khoan) va
// `AssignProxyDialog` (gan hang loat). Mirror ngu nghia `effective_proxy` phia Rust
// (crates/lvc-core/src/proxy.rs):
//   proxy rong ""   -> dung PROXY CHUNG (global_proxy trong Cai dat)
//   proxy "direct"  -> KHONG dung proxy nao (IP may)
//   proxy khac      -> proxy co dinh cua rieng tai khoan
//
// QUYET DINH ve che do "Xoay vòng" (ban thiet ke AssignProxy.dc.html, "Lấy từ danh sách proxy"):
// repo CHUA co backend danh sach proxy de xoay vong THAT. De khong dung mot nut CHET, che do thu ba
// duoc map ve PROXY CHUNG toan cuc (proxy rong -> inherit global_proxy) voi nhan DUNG su that
// ("Proxy chung" / "Dùng chung trong Cài đặt") kem ghi chu "xoay vòng theo danh sách — sắp có".

export type ConnMode = "machine" | "fixed" | "shared";

export interface ConnModeInfo {
  mode: ConnMode;
  label: string;
  sub: string;
}

/** Ba che do hien tren luoi (dung thu tu ban thiet ke: IP máy · Cố định · Chung/Xoay vòng). */
export const CONN_MODES: readonly ConnModeInfo[] = [
  { mode: "machine", label: "IP máy", sub: "Không dùng proxy" },
  { mode: "fixed", label: "Proxy cố định", sub: "Mỗi tài khoản một IP" },
  { mode: "shared", label: "Proxy chung", sub: "Dùng chung trong Cài đặt" },
];

/** Suy che do tu chuoi proxy da luu (co the da che `••••` voi credential — van suy duoc mode). */
export function proxyToMode(proxy: string): ConnMode {
  const raw = proxy.trim();
  if (raw === "") {
    return "shared";
  }
  if (raw.toLowerCase() === "direct") {
    return "machine";
  }
  return "fixed";
}

/** Chuoi proxy se LUU theo che do; che do `fixed` dung `fixedValue` (da trim o noi goi). */
export function modeToProxy(mode: ConnMode, fixedValue: string): string {
  switch (mode) {
    case "machine":
      return "direct";
    case "shared":
      return "";
    case "fixed":
      return fixedValue.trim();
  }
}
