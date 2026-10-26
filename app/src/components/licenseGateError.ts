// Đổi lỗi IPC của cổng bản quyền thành KHOÁ i18n. Hàm THUẦN, tách riêng để test không cần React.
// Mã lỗi do `crates/lvc-app/src/commands/license.rs` sinh; mã lạ → thông báo chung (không bao giờ
// ném nguyên `message` kỹ thuật ra màn đăng nhập).
import type { DictKey } from "../i18n";
import { isIpcError } from "../ipc/commands";

const BIET: readonly string[] = [
  "NETWORK",
  "BAD_CREDENTIALS",
  "VERIFY_FAILED",
  "NOT_LOGGED_IN",
  "SESSION_SAVE",
];

export function licenseErrorKey(e: unknown): DictKey {
  if (isIpcError(e) && BIET.includes(e.code)) {
    return `gate.err.${e.code}` as DictKey;
  }
  return "gate.err.unknown";
}
