// Ham THUAN (khong I/O) cho man Tai khoan (Task 14) — hai nhom:
// 1. Validate PHIA CLIENT truoc khi goi IPC, mirror chinh xac logic Rust da co (khong dinh nghia
//    lai quy tac o day, chi PORT lai) — server (`lvc-engine::accounts`) VAN la nguon su that cuoi
//    cung, loi `INVALID_NAME`/`INVALID_PROXY` server tra ve luon duoc hien (xem
//    `accountErrorMessage`), day chi la kiem nhanh de UI bao loi ngay khong doi round-trip.
// 2. Dinh dang hien thi cho `AccountRow` (nhan trang thai dang nhap, han cooldown/rate/quota con
//    lai, chuoi luot dung gon).
import { isIpcError } from "../ipc/commands";
import type { AccountStatus, IpcError } from "../ipc/types";

// ---------------------------------------------------------------------------------------------
// Ten tai khoan — crates/lvc-engine/src/accounts.rs::validate_name, NAME_RE = ^[A-Za-z0-9_-]{1,32}$
// (port Python server.py:100,1587,1682). Khong port lai kiem tra "1 Path::Component" cua Rust —
// do la phong thu duong dan he dieu hanh phia server, khong the/khong can lam lai trong trinh
// duyet; regex ky tu + do dai la du cho UX.
// ---------------------------------------------------------------------------------------------

const NAME_RE = /^[A-Za-z0-9_-]{1,32}$/;

export const NAME_INVALID_MSG =
  "Tên chỉ gồm chữ, số, _ hoặc -, tối đa 32 ký tự (không được để trống).";

/** `null` = hop le. */
export function validateName(name: string): string | null {
  return NAME_RE.test(name) ? null : NAME_INVALID_MSG;
}

// ---------------------------------------------------------------------------------------------
// Proxy — mirror crates/lvc-core/src/proxy.rs::parse_proxy (proxies.py). Ba dang cho phep:
//   "" / "direct"                  -> khong dat / khong dung proxy nao
//   "ip:port" | "ip:port:user:pass" (user/pass phai CUNG co hoac CUNG rong)
//   "scheme://[user:pass@]host:port" (scheme la http/https/socks5)
// KHONG percent-decode username/password o day (khong can cho MUC DICH validate — chi kiem
// "co ca hai hoac khong co ca hai", khong dung gia tri da giai ma cho gi khac).
// ---------------------------------------------------------------------------------------------

const SCHEMES = new Set(["http", "https", "socks5"]);

export const PROXY_INVALID_MSG =
  "proxy must be ip:port[:user:pass] or scheme://user:pass@host:port (scheme http/https/socks5), or 'direct'";

function isDigits(s: string): boolean {
  return s.length > 0 && [...s].every((c) => c >= "0" && c <= "9");
}

/** proxies.py `_HOST_RE = ^[A-Za-z0-9.\-_\[\]:]+$`. */
function hostOk(host: string): boolean {
  return host.length > 0 && [...host].every((c) => /[A-Za-z0-9.\-_[\]:]/.test(c));
}

/** proxies.py `_port_ok`: chi chu so, 1..=65535 (KHAC dang URL — xem `portOkUrlForm`). */
function portOkPlainForm(port: string): boolean {
  if (!isDigits(port)) {
    return false;
  }
  const n = Number(port);
  return n >= 1 && n <= 65535;
}

/** Cat "authority" ([user[:pass]@]host[:port]) khoi path/query/fragment con lai. */
function splitAuthority(rest: string): [authority: string, tail: string] {
  const idx = rest.search(/[/?#]/);
  return idx === -1 ? [rest, ""] : [rest.slice(0, idx), rest.slice(idx)];
}

interface Netloc {
  hasUser: boolean;
  hasPass: boolean;
  host: string;
  port: string | null;
}

/** Tach netloc thanh (co-username?, co-password?, host, port-tho). */
function splitNetloc(netloc: string): Netloc {
  const at = netloc.lastIndexOf("@");
  const userinfo = at === -1 ? null : netloc.slice(0, at);
  const hostinfo = at === -1 ? netloc : netloc.slice(at + 1);

  let hasUser = false;
  let hasPass = false;
  if (userinfo !== null) {
    const ci = userinfo.indexOf(":");
    const u = ci === -1 ? userinfo : userinfo.slice(0, ci);
    const p = ci === -1 ? null : userinfo.slice(ci + 1);
    hasUser = u !== "";
    hasPass = p !== null && p !== "";
  }

  let host: string;
  let port: string | null;
  if (hostinfo.startsWith("[")) {
    // IPv6: [::1]:1080 — host la phan TRONG ngoac, port sau "]:" neu co.
    const end = hostinfo.indexOf("]");
    if (end === -1) {
      host = hostinfo;
      port = null;
    } else {
      host = hostinfo.slice(1, end);
      const tail = hostinfo.slice(end + 1);
      port = tail.startsWith(":") && tail.length > 1 ? tail.slice(1) : null;
    }
  } else {
    const ci = hostinfo.indexOf(":");
    if (ci === -1) {
      host = hostinfo;
      port = null;
    } else {
      host = hostinfo.slice(0, ci);
      const p = hostinfo.slice(ci + 1);
      port = p === "" ? null : p;
    }
  }
  return { hasUser, hasPass, host: host.toLowerCase(), port };
}

/** Dang `scheme://[user:pass@]host:port`. */
function checkUrlForm(raw: string): string | null {
  const idx = raw.indexOf("://");
  if (idx === -1) {
    return PROXY_INVALID_MSG;
  }
  const scheme = raw.slice(0, idx).toLowerCase();
  if (!SCHEMES.has(scheme)) {
    return PROXY_INVALID_MSG;
  }
  const rest = raw.slice(idx + 3);
  const [authority, tail] = splitAuthority(rest);
  if (!(tail === "" || tail === "/")) {
    return PROXY_INVALID_MSG;
  }
  const { hasUser, hasPass, host, port } = splitNetloc(authority);
  if (host === "") {
    return PROXY_INVALID_MSG;
  }
  if (port === null || !isDigits(port)) {
    return PROXY_INVALID_MSG;
  }
  // Luu y: dang URL KHONG bat can-duoi 1 cho port (khop nguyen van parse_url_form phia Rust —
  // chi tu choi port > 65535, "0" hop le o day dau khac dang plain ben duoi).
  if (Number(port) > 65535) {
    return PROXY_INVALID_MSG;
  }
  if (!hostOk(host)) {
    return PROXY_INVALID_MSG;
  }
  if (hasUser !== hasPass) {
    return PROXY_INVALID_MSG;
  }
  return null;
}

/** Dang `ip:port` hoac `ip:port:user:pass`. */
function checkPlainForm(raw: string): string | null {
  const pieces = raw.split(":");
  let host: string;
  let port: string;
  if (pieces.length === 2) {
    // `!` an toan: nhanh nay chi vao khi `pieces.length === 2` vua kiem ngay tren.
    host = pieces[0]!;
    port = pieces[1]!;
  } else if (pieces.length === 4) {
    const user = pieces[2]!;
    const pass = pieces[3]!;
    if (user === "" || pass === "") {
      return PROXY_INVALID_MSG;
    }
    host = pieces[0]!;
    port = pieces[1]!;
  } else {
    return PROXY_INVALID_MSG;
  }
  host = host.trim();
  port = port.trim();
  if (host === "" || !hostOk(host) || !portOkPlainForm(port)) {
    return PROXY_INVALID_MSG;
  }
  return null;
}

/** `null` = hop le (gom "" va "direct"). */
export function validateProxy(text: string): string | null {
  const raw = text.trim();
  if (raw === "" || raw.toLowerCase() === "direct") {
    return null;
  }
  if (/\s/.test(raw)) {
    return PROXY_INVALID_MSG;
  }
  return raw.includes("://") ? checkUrlForm(raw) : checkPlainForm(raw);
}

// ---------------------------------------------------------------------------------------------
// Hien thi trang thai (AccountRow).
// ---------------------------------------------------------------------------------------------

export type LoginLabel = "OK" | "Hết phiên" | "Chưa rõ";

/**
 * `AccountStatus.loginOk` la `bool` THAT (khong nullable — port Rust bo tri-state cua Python
 * `login_ok: None|0|1`, xem task-10-context.md dong 4 + cot `login_ok` mac dinh 1 trong
 * `accounts_meta`). "Chua ro" duoc suy lai o UI tu `loginCheckedAt <= 0` (chua tung kiem tra —
 * cung quy uoc "0 = chua co gia tri" ma `cooldownUntil`/`rateLimitedUntil`/`quotaBlockedUntil`/
 * `creditCheckedAt` dang dung xuyen suot `AccountStatus`).
 */
export function accountStatusLabel(row: Pick<AccountStatus, "loginOk" | "loginCheckedAt">): LoginLabel {
  if (row.loginCheckedAt <= 0) {
    return "Chưa rõ";
  }
  return row.loginOk ? "OK" : "Hết phiên";
}

/** So phut CON LAI toi `until` (epoch giay) tinh tu `now`; `until <= 0` = khong ap dung -> 0. */
export function remainingMinutes(until: number, now: number): number {
  if (until <= 0) {
    return 0;
  }
  return Math.max(0, Math.ceil((until - now) / 60));
}

type UsageRow = Pick<
  AccountStatus,
  "usedToday" | "limit" | "usedPro" | "limitPro" | "usedProMax" | "limitProMax" | "usedAgent" | "limitAgent"
>;

/** Chuoi gon "3/10 · pro 0/2 · pro_max 0/2 · agent 0/30" (thu tu mode fast/pro/pro_max/agent). */
export function formatUsageCompact(row: UsageRow): string {
  return [
    `${row.usedToday}/${row.limit}`,
    `pro ${row.usedPro}/${row.limitPro}`,
    `pro_max ${row.usedProMax}/${row.limitProMax}`,
    `agent ${row.usedAgent}/${row.limitAgent}`,
  ].join(" · ");
}

type RemainingRow = Pick<
  AccountStatus,
  "cooling" | "cooldownUntil" | "rateLimited" | "rateLimitedUntil" | "quotaBlocked" | "quotaBlockedUntil"
>;

/** Chuoi gon cac han con hieu luc ("Nghỉ 5p · Rate 0p"); "—" khi khong co han nao dang ap dung. */
export function formatRemaining(row: RemainingRow, now: number): string {
  const parts: string[] = [];
  if (row.cooling) {
    parts.push(`Nghỉ ${remainingMinutes(row.cooldownUntil, now)}p`);
  }
  if (row.rateLimited) {
    parts.push(`Rate ${remainingMinutes(row.rateLimitedUntil, now)}p`);
  }
  if (row.quotaBlocked) {
    parts.push(`Quota ${remainingMinutes(row.quotaBlockedUntil, now)}p`);
  }
  return parts.length > 0 ? parts.join(" · ") : "—";
}

// ---------------------------------------------------------------------------------------------
// Ma loi IPC -> thong diep tieng Viet (xem task-10-report.md "Error codes").
// ---------------------------------------------------------------------------------------------

/**
 * Exported de test pin duoc (xem `tests/format_account.test.ts` — "pin ma loi"): danh sach 13 gia
 * tri phai khop CHINH XAC `AccountError::code()` (`crates/lvc-engine/src/accounts.rs`), khong hon
 * khong kem, de mot bien the loi MOI (hoac mot ma bi go sai) fail build thay vi am tham thieu
 * nhan tieng Viet (review task-14-review.md, Minor #1; NOT_A_PROFILE them boi fix-round 1
 * final-review I5).
 */
export const ACCOUNT_ERROR_VI: Record<string, string> = {
  ACCOUNT_EXISTS: "Tài khoản đã tồn tại.",
  ACCOUNT_NOT_FOUND: "Không tìm thấy tài khoản.",
  INVALID_NAME: NAME_INVALID_MSG,
  COOKIE_INVALID: "Nội dung cookie không hợp lệ (thiếu sessionid hợp lệ cho dola.com).",
  COOKIE_NOT_LOGGED_IN: "Cookie không đăng nhập được (phiên đã hết hạn hoặc sai).",
  INVALID_PROXY: PROXY_INVALID_MSG,
  PROFILE_BUSY: "Tài khoản đang được dùng bởi một tác vụ khác, thử lại sau.",
  ACCOUNT_BUSY: "Tài khoản đang có tác vụ đang chạy, không thể xoá lúc này.",
  // I5 (fix-round 1, final-review): thu muc khong giong profile Chromium — thuong do accountsDir
  // (Cài đặt) bi tro nham sang thu muc khac. Dong DB van con, xoa lai duoc sau khi sua accountsDir.
  NOT_A_PROFILE:
    "Thư mục tài khoản không giống hồ sơ Chromium — kiểm tra lại cài đặt \"Thư mục tài khoản\" rồi thử lại.",
  BROWSER: "Lỗi trình duyệt khi thao tác tài khoản.",
  STORE: "Lỗi lưu dữ liệu.",
  IO: "Lỗi đọc/ghi dữ liệu.",
  ENGINE: "Lỗi hệ thống.",
};

/** Thong diep tieng Viet cho `IpcError` cua man Tai khoan — roi ve `err.message` neu ma la. */
export function accountErrorMessage(err: IpcError): string {
  return ACCOUNT_ERROR_VI[err.code] ?? err.message;
}

/**
 * Thong diep hien thi cho MOT `catch (e: unknown)` bat ky trong man Tai khoan. `invoke()` tu choi
 * bang mot `IpcError` THUAN (`{code,message,details}`) — KHONG PHAI `Error` JS that,
 * `e instanceof Error` luon `false` va `String(e)` ra `"[object Object]"` neu dung nham (phat hien
 * o review Task 12b) — nen luon kiem `isIpcError` TRUOC, chi roi ve `Error`/`String` thong thuong
 * cho loi khong den tu IPC (vd loi JS noi bo).
 */
export function describeAccountError(e: unknown): string {
  if (isIpcError(e)) {
    return accountErrorMessage(e);
  }
  return e instanceof Error ? e.message : String(e);
}
