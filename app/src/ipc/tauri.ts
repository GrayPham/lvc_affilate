// Lop mong boc `@tauri-apps/api` — MOI truy cap `invoke`/`listen` trong app phai di qua day
// (khong import thang `@tauri-apps/api/core` / `@tauri-apps/api/event` o noi khac), vi hai ly do:
// 1. `commands.ts`/`events.ts` co the mock module nay trong test/Storybook/trinh duyet thuong
//    (vd `vi.mock("../ipc/tauri")`) ma khong dung toi runtime Tauri that.
// 2. Chay ngoai cua so Tauri (vd `pnpm dev` mo trong trinh duyet de xem UI nhanh) se bao loi ro
//    rang thay vi mot loi noi bo kho hieu tu `@tauri-apps/api`.
import {
  convertFileSrc,
  invoke as tauriInvoke,
  type InvokeArgs,
  type InvokeOptions,
} from "@tauri-apps/api/core";
import {
  listen as tauriListen,
  type EventCallback,
  type UnlistenFn,
} from "@tauri-apps/api/event";

/** `true` khi trang dang chay trong mot cua so Tauri that (webview co inject cau IPC). */
export function hasTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined"
  );
}

function assertTauriRuntime(): void {
  if (!hasTauriRuntime()) {
    throw new Error(
      "Khong co runtime Tauri (window.__TAURI_INTERNALS__ khong ton tai) — trang nay dang " +
        "chay ngoai cua so Tauri (vd trinh duyet thuong hoac moi truong test chua mock " +
        "`src/ipc/tauri.ts`). Lenh IPC chi goi duoc tu ben trong ung dung Tauri.",
    );
  }
}

export async function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  assertTauriRuntime();
  return tauriInvoke<T>(cmd, args, options);
}

export async function listen<T>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  assertTauriRuntime();
  return tauriListen<T>(event, handler);
}

/**
 * Đổi đường dẫn file cục bộ → URL webview đọc được (giao thức `asset:` của Tauri, đã bật scope
 * trong `tauri.conf.json`). Dùng để preview ảnh/video đã chọn trong node. Ngoài Tauri (dev/browser)
 * → trả rỗng để `<img>`/`<video>` không cố tải đường dẫn tuyệt đối (tránh lỗi).
 */
export function assetUrl(path: string | null | undefined): string {
  if (!path) {
    return "";
  }
  if (!hasTauriRuntime()) {
    return "";
  }
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

export type { UnlistenFn };
