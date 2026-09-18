// Hop thoai "Nạp cookie" (Task 14 brief) — ten tai khoan + dan noi dung cookies.txt (dinh dang
// Netscape) + proxy tuy chon -> `account_import_cookies` (mo Chrome an de kiem tra dang nhap
// duoc). KHONG co duong chon file: doc file can FS plugin chua duoc cai (xem ghi chu duoi) — chi
// dan-noi-dung, nhan ro trong nhan de khong gay hieu lam thieu tinh nang.
//
// AN TOAN COOKIE: khong bao gio hien lai / log gia tri da dan — textarea bi xoa NGAY khi bam
// "Nạp" (truoc ca khi cho ket qua IPC tra ve), va khong co bat ky `console.*`/hien thi nao khac
// dung toi bien chua noi dung cookie.
import { useState, type FormEvent } from "react";

import { describeAccountError, validateName, validateProxy } from "../format/account";
import { useAccountsStore } from "../store/accounts";
import type { ImportCookiesReport } from "../ipc/types";

interface CookieImportProps {
  onClose: () => void;
}

export default function CookieImport({ onClose }: CookieImportProps) {
  const importCookies = useAccountsStore((s) => s.importCookies);

  const [name, setName] = useState("");
  const [cookieText, setCookieText] = useState("");
  const [proxy, setProxy] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ImportCookiesReport | null>(null);

  const nameError = name === "" ? null : validateName(name);
  const proxyError = proxy === "" ? null : validateProxy(proxy);
  const canSubmit =
    !busy &&
    result === null &&
    validateName(name) === null &&
    validateProxy(proxy) === null &&
    cookieText.trim() !== "";

  async function handleSubmit(e: FormEvent<HTMLFormElement>): Promise<void> {
    e.preventDefault();
    if (!canSubmit) {
      return;
    }
    const textToSend = cookieText;
    // Xoa NGAY — xem ghi chu AN TOAN COOKIE o dau file.
    setCookieText("");
    setBusy(true);
    setError(null);
    try {
      const report = await importCookies(name, textToSend, proxy);
      setResult(report);
    } catch (err) {
      setError(describeAccountError(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
      <div className="w-[28rem] max-w-full rounded-lg bg-white p-4 shadow-lg">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold">Nạp cookie</h2>
          <button type="button" className="text-slate-400 hover:text-slate-700" onClick={onClose}>
            Đóng
          </button>
        </div>

        {result ? (
          <div className="flex flex-col gap-3">
            <p className="text-sm text-emerald-700">
              Đã nạp tài khoản "{result.name}" — đăng nhập hợp lệ.
              {result.sessionidExpiresAt !== null
                ? ` Hạn cookie: ${new Date(result.sessionidExpiresAt * 1000).toLocaleString("vi-VN")}.`
                : ""}
            </p>
            <button
              type="button"
              className="self-end rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700"
              onClick={onClose}
            >
              Đóng
            </button>
          </div>
        ) : (
          <form className="flex flex-col gap-3" onSubmit={(e) => void handleSubmit(e)}>
            <label className="flex flex-col gap-1 text-xs">
              Tên tài khoản
              <input
                className="rounded border border-slate-300 px-2 py-1 text-sm"
                value={name}
                disabled={busy}
                onChange={(e) => setName(e.target.value)}
                placeholder="vd acc1"
              />
              {nameError ? <span className="text-red-600">{nameError}</span> : null}
            </label>

            <label className="flex flex-col gap-1 text-xs">
              Dán nội dung cookies.txt (định dạng Netscape)
              <textarea
                className="h-32 resize-none rounded border border-slate-300 px-2 py-1 font-mono text-xs"
                value={cookieText}
                disabled={busy}
                onChange={(e) => setCookieText(e.target.value)}
                placeholder={".dola.com\tTRUE\t/\tTRUE\t1800000000\tsessionid\t..."}
              />
              <span className="text-slate-400">
                Chưa hỗ trợ chọn file trực tiếp trong bản này — dán nội dung file vào đây.
              </span>
            </label>

            <label className="flex flex-col gap-1 text-xs">
              Proxy (tuỳ chọn)
              <input
                className="rounded border border-slate-300 px-2 py-1 font-mono text-sm"
                value={proxy}
                disabled={busy}
                onChange={(e) => setProxy(e.target.value)}
                placeholder="để trống = dùng proxy chung, hoặc ip:port, hoặc direct"
              />
              {proxyError ? <span className="text-red-600">{proxyError}</span> : null}
            </label>

            {busy ? <p className="text-xs text-sky-600">Đang mở Chrome ẩn để kiểm tra…</p> : null}
            {error ? <p className="text-xs text-red-600">{error}</p> : null}

            <div className="flex justify-end gap-2">
              <button
                type="button"
                disabled={busy}
                className="rounded border border-slate-300 px-3 py-1 text-xs hover:bg-slate-50 disabled:opacity-50"
                onClick={onClose}
              >
                Huỷ
              </button>
              <button
                type="submit"
                disabled={!canSubmit}
                className="rounded bg-slate-900 px-3 py-1 text-xs text-white hover:bg-slate-700 disabled:opacity-50"
              >
                Nạp
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}
