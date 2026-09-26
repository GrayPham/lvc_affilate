// Ham THUAN (khong I/O) cho man "Tài khoản AI" (P4c) — nhan tieng Viet cho provider/kind/status +
// ma loi CRED_* -> thong diep tieng Viet. Theo dung quy uoc `format/account.ts`:
// - Cac record duoc EXPORT de test ghim (pin) day du cac gia tri enum/ma loi cua backend, mot gia
//   tri MOI (hoac go sai) se fail build thay vi am tham thieu nhan.
// - `describeCredentialError` luon kiem `isIpcError` TRUOC (invoke() reject bang IpcError THUAN,
//   khong phai Error JS that — xem ghi chu o `format/account.ts::describeAccountError`).
import { isIpcError } from "../ipc/commands";
import type {
  CredentialKind,
  CredentialProvider,
  CredentialStatus,
  IpcError,
} from "../ipc/types";

// ---------------------------------------------------------------------------------------------
// Nhan provider — khop dung 5 provider cua `CredentialProviderSchema` (src/ipc/types.ts).
// ---------------------------------------------------------------------------------------------

export const CREDENTIAL_PROVIDER_VI: Record<CredentialProvider, string> = {
  dola: "Dola",
  google: "Google",
  chatgpt_web: "ChatGPT (web)",
  openai: "OpenAI",
  gemini: "Gemini",
};

export function credentialProviderLabel(provider: CredentialProvider): string {
  return CREDENTIAL_PROVIDER_VI[provider];
}

// ---------------------------------------------------------------------------------------------
// Nhan loai bi mat (kind).
// ---------------------------------------------------------------------------------------------

export const CREDENTIAL_KIND_VI: Record<CredentialKind, string> = {
  session: "Phiên (cookie)",
  api_key: "API key",
  login: "Đăng nhập",
};

export function credentialKindLabel(kind: CredentialKind): string {
  return CREDENTIAL_KIND_VI[kind];
}

// ---------------------------------------------------------------------------------------------
// Chế độ API (lưu trong `meta.mode`, khái niệm phía UI): official = key chính thức, custom =
// endpoint bên thứ 3 (base_url riêng), sdk = dùng SDK chính chủ, cookie = đăng nhập/phiên.
// ---------------------------------------------------------------------------------------------

export type CredentialMode = "official" | "custom" | "sdk" | "cookie";

export const CREDENTIAL_MODE_VI: Record<CredentialMode, string> = {
  official: "Chính thức (API key)",
  custom: "Endpoint tuỳ chỉnh",
  sdk: "SDK",
  cookie: "Đăng nhập/Cookie",
};

export function credentialModeLabel(mode: CredentialMode): string {
  return CREDENTIAL_MODE_VI[mode];
}

/** Suy chế độ từ `meta.mode`; nếu thiếu thì đoán từ `kind` (api_key→official, còn lại→cookie). */
export function rowMode(meta: Record<string, unknown>, kind: CredentialKind): CredentialMode {
  const m = meta.mode;
  if (m === "official" || m === "custom" || m === "sdk" || m === "cookie") {
    return m;
  }
  return kind === "api_key" ? "official" : "cookie";
}

/** `base_url` (endpoint bên thứ 3) nếu có trong meta. */
export function rowBaseUrl(meta: Record<string, unknown>): string {
  return typeof meta.base_url === "string" ? meta.base_url : "";
}

// ---------------------------------------------------------------------------------------------
// Nhan trang thai — khop dung 5 status cua `CredentialStatusSchema`.
// ---------------------------------------------------------------------------------------------

export const CREDENTIAL_STATUS_VI: Record<CredentialStatus, string> = {
  ok: "OK",
  expired: "Hết hạn",
  banned: "Bị khoá",
  quota: "Hết hạn mức",
  unknown: "Chưa rõ",
};

export function credentialStatusLabel(status: CredentialStatus): string {
  return CREDENTIAL_STATUS_VI[status];
}

/** Lop mau badge cho tung trang thai (mau phan biet ro giua cac trang thai). */
export const CREDENTIAL_STATUS_BADGE_CLASS: Record<CredentialStatus, string> = {
  ok: "bg-emerald-100 text-emerald-700",
  expired: "bg-amber-100 text-amber-700",
  banned: "bg-red-100 text-red-700",
  quota: "bg-orange-100 text-orange-700",
  unknown: "bg-slate-100 text-slate-600",
};

// ---------------------------------------------------------------------------------------------
// Ma loi IPC -> thong diep tieng Viet (P4c contract). Exported de test pin duoc — 7 ma phai khop
// CHINH XAC cac ma backend tra ve cho mien credentials, khong hon khong kem.
// ---------------------------------------------------------------------------------------------

export const CREDENTIAL_ERROR_VI: Record<string, string> = {
  CRED_BAD_PROVIDER: "Nhà cung cấp không hợp lệ.",
  CRED_BAD_KIND: "Loại tài khoản không hợp lệ cho nhà cung cấp này.",
  CRED_BAD_LABEL: "Nhãn không hợp lệ (không được để trống).",
  CRED_EMPTY_SECRET: "Chưa nhập bí mật (API key hoặc cookie/đăng nhập).",
  CRED_NOT_FOUND: "Không tìm thấy tài khoản.",
  CRED_CRYPTO: "Lỗi mã hoá/giải mã bí mật.",
  STORE: "Lỗi lưu dữ liệu.",
};

/** Thong diep tieng Viet cho `IpcError` cua man Tai khoan AI — roi ve `err.message` neu ma la. */
export function credentialErrorMessage(err: IpcError): string {
  return CREDENTIAL_ERROR_VI[err.code] ?? err.message;
}

/** Thong diep hien thi cho MOT `catch (e: unknown)` bat ky — xem `describeAccountError`. */
export function describeCredentialError(e: unknown): string {
  if (isIpcError(e)) {
    return credentialErrorMessage(e);
  }
  return e instanceof Error ? e.message : String(e);
}
