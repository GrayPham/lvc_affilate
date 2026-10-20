// Dialog DUY NHAT de them tai khoan (Dola hoac nen tang khac) — thay 3 khoi form luon-hien truoc
// day (review nguoi dung: "quan ly thanh 1 bang thong nhat, con muon them thi dialog bat len").
// Chon NEN TANG truoc, dialog tu suy CACH THEM hop le cho nen tang do:
// - Dola: dan cookie 1 tai khoan | nap nhieu file — CA HAI deu ghi vao `useAccountsStore` (kho
//   Dola rieng, co lich/han muc — xem AccountsPage) de khop DUNG bang da hop nhat.
// - Nen tang khac (Google/Gemini/Facebook/Apple/ChatGPT/MuseAI): dan cookie | nap nhieu file | mo
//   trinh duyet dang nhap (chi khi biet URL — `INTERACTIVE_LOGIN_PROVIDERS`) — ghi vao
//   `useCredentialsStore` (kho dung chung P4c).
//
// Luu y KIEN TRUC: "Mo trinh duyet dang nhap" (`login_open_interactive`, lvc-app) LUON ghi vao kho
// dung chung (`lvc-vault`) KE CA khi chon Dola — khac thu muc/bang voi `useAccountsStore` (kho Dola
// rieng, `accounts_dir/<ten>/`). Vi vay o day CHU DICH khong cho chon "mo trinh duyet" khi nen tang
// la Dola, de tranh tai khoan tao ra khong hien trong bang Dola da hop nhat (gay nham lan).
import { useState } from "react";

import Dialog from "./Dialog";
import { BTN_NEUTRAL_SM, BTN_PRIMARY_SM } from "../ui/buttonStyles";
import { useAccountsStore } from "../store/accounts";
import { useCredentialsStore } from "../store/credentials";
import { validateName, validateProxy, NAME_INVALID_MSG } from "../format/account";
import {
  COOKIE_PROVIDERS,
  INTERACTIVE_LOGIN_PROVIDERS,
  credentialProviderLabel,
  describeCredentialError,
} from "../format/credential";
import { loginCookieText, loginCookieTextBulk, loginImportCookieFiles, loginOpenInteractive, pickFiles } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { BrowserEngineId, CredentialProvider } from "../ipc/types";

type Platform = CredentialProvider; // "dola" la mot gia tri cua CredentialProvider, xu ly rieng ben duoi.
type Method = "paste" | "bulkPaste" | "bulk" | "browser";

/** Phuong thuc hop le theo nen tang — xem doc dau file ve ly do Dola khong co "browser". */
function methodsFor(platform: Platform): readonly Method[] {
  if (platform === "dola") {
    return ["paste", "bulk"];
  }
  return INTERACTIVE_LOGIN_PROVIDERS.includes(platform)
    ? ["browser", "paste", "bulkPaste", "bulk"]
    : ["paste", "bulkPaste", "bulk"];
}

const METHOD_LABEL: Record<Method, string> = {
  paste: "Dán cookie",
  bulkPaste: "Dán hàng loạt",
  bulk: "Nạp nhiều file",
  browser: "Mở trình duyệt đăng nhập",
};

export default function AddAccountDialog({ onClose }: { onClose: () => void }) {
  const [platform, setPlatform] = useState<Platform>("dola");
  const [method, setMethod] = useState<Method>("paste");
  const [label, setLabel] = useState("");
  const [secret, setSecret] = useState("");
  const [proxy, setProxy] = useState("");
  // Engine cho luong "Dan cookie" nen tang KHAC Dola: chromium (tu dong) hoac firefox_portable
  // (tiem cookie vao profile Firefox, CHI dung tay).
  const [engine, setEngine] = useState<BrowserEngineId>("chromium");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);
  const [bulkResult, setBulkResult] = useState<
    ReadonlyArray<{ label: string; ok: boolean; error: string | null }> | null
  >(null);

  const importCookies = useAccountsStore((s) => s.importCookies);
  const importCookieFiles = useAccountsStore((s) => s.importCookieFiles);

  const methods = methodsFor(platform);
  const proxyError = proxy === "" ? null : validateProxy(proxy);
  const nameError = platform === "dola" && label !== "" ? validateName(label) : null;

  function handlePlatformChange(next: Platform): void {
    setPlatform(next);
    const nextMethods = methodsFor(next);
    if (!nextMethods.includes(method)) {
      setMethod(nextMethods[0]!);
    }
    // Firefox chi ap cho nen tang credential; ve Dola thi quay lai Chromium.
    if (next === "dola") {
      setEngine("chromium");
    }
    setError(null);
    setOkMsg(null);
    setBulkResult(null);
  }

  async function submitPaste(): Promise<void> {
    setBusy(true);
    setError(null);
    setOkMsg(null);
    try {
      if (platform === "dola") {
        await importCookies(label.trim(), secret, proxy.trim());
      } else {
        // Tiem cookie vao profile that (Phan A) thay vi chi luu chuoi tho qua credentialAdd —
        // lenh Rust tu dung `credential_add` phia sau, khong can goi store rieng o day. `engine`
        // chon Chromium (CDP) hay Firefox Portable (ghi cookies.sqlite, dung tay).
        await loginCookieText(platform, label.trim(), secret, proxy.trim(), engine);
        await useCredentialsStore.getState().load();
      }
      setOkMsg(`Đã thêm "${label.trim()}".`);
      setLabel("");
      setSecret("");
      setProxy("");
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  async function submitBulk(): Promise<void> {
    setError(null);
    setOkMsg(null);
    setBulkResult(null);
    let paths: string[] = [];
    try {
      paths = await pickFiles(false);
    } catch (e) {
      setError(errorText(e));
      return;
    }
    if (paths.length === 0) {
      return;
    }
    setBusy(true);
    try {
      if (platform === "dola") {
        const items = await importCookieFiles(paths, proxy.trim());
        setBulkResult(items.map((b) => ({ label: b.name, ok: b.ok, error: b.error })));
      } else {
        const items = await loginImportCookieFiles(platform, paths, proxy.trim());
        setBulkResult(items);
        await useCredentialsStore.getState().load();
      }
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  async function submitBulkPaste(): Promise<void> {
    setError(null);
    setOkMsg(null);
    setBulkResult(null);
    if (secret.trim() === "") {
      setError("Dán nội dung (mỗi dòng: email|...|cookies, hoặc chỉ cookie).");
      return;
    }
    setBusy(true);
    try {
      const items = await loginCookieTextBulk(platform, secret, proxy.trim(), engine);
      setBulkResult(items);
      await useCredentialsStore.getState().load();
      const ok = items.filter((i) => i.ok).length;
      setOkMsg(`Đã thêm ${ok}/${items.length} tài khoản.`);
      if (ok > 0) {
        setSecret("");
      }
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  async function submitBrowser(): Promise<void> {
    setBusy(true);
    setError(null);
    setOkMsg(null);
    try {
      const res = await loginOpenInteractive(platform, label.trim(), proxy.trim());
      setOkMsg(`Đã mở & lưu "${res.label}". Hãy đăng nhập trong cửa sổ vừa mở.`);
      setLabel("");
    } catch (e) {
      setError(describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  const canSubmitPaste =
    !busy && label.trim() !== "" && secret.trim() !== "" && nameError === null && proxyError === null;
  const canSubmitBrowser = !busy && label.trim() !== "" && proxyError === null;
  const canSubmitBulkPaste = !busy && secret.trim() !== "" && proxyError === null;

  return (
    <Dialog title="Thêm tài khoản" onClose={onClose} width="36rem">
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap gap-3">
          <label className="flex flex-col gap-1 text-xs text-muted">
            Nền tảng
            <select
              className="rounded-md border border-line bg-fill px-2.5 py-1.5 text-sm text-fg"
              value={platform}
              disabled={busy}
              onChange={(e) => handlePlatformChange(e.target.value as Platform)}
            >
              {COOKIE_PROVIDERS.map((p) => (
                <option key={p} value={p} className="bg-panel text-fg">
                  {credentialProviderLabel(p)}
                </option>
              ))}
            </select>
          </label>

          <div className="flex flex-col gap-1 text-xs text-muted">
            Cách thêm
            <div className="flex overflow-hidden rounded-md border border-line">
              {methods.map((m) => (
                <button
                  key={m}
                  type="button"
                  disabled={busy}
                  className={
                    "px-2.5 py-1.5 text-xs disabled:cursor-not-allowed disabled:opacity-50 " +
                    (method === m
                      ? "bg-primary text-primary-fg"
                      : "bg-fill text-muted hover:bg-fill2")
                  }
                  onClick={() => {
                    setMethod(m);
                    setError(null);
                    setOkMsg(null);
                    setBulkResult(null);
                  }}
                >
                  {METHOD_LABEL[m]}
                </button>
              ))}
            </div>
          </div>
        </div>

        {method === "bulk" ? (
          <div className="flex flex-col gap-3 rounded-lg border border-line bg-fill p-3">
            <label className="flex flex-col gap-1 text-xs text-muted">
              Proxy chung (tuỳ chọn)
              <input
                className="rounded-md border border-line bg-fill px-2.5 py-1.5 font-mono text-sm text-fg placeholder:text-ink3"
                value={proxy}
                disabled={busy}
                onChange={(e) => setProxy(e.target.value)}
                placeholder="để trống, hoặc ip:port"
              />
              {proxyError ? <span className="text-rose">{proxyError}</span> : null}
            </label>
            <p className="text-[11px] text-ink3">
              Mỗi file = một tài khoản (tên = tên file), lưu dạng phiên (cookie). Đường hợp lệ — bạn
              tự xuất cookie đã đăng nhập.
            </p>
            <button
              type="button"
              disabled={busy || proxyError !== null}
              className={BTN_PRIMARY_SM + " self-start"}
              onClick={() => void submitBulk()}
            >
              {busy ? "Đang nạp…" : "Chọn nhiều file cookie…"}
            </button>
            {bulkResult ? (
              <div className="max-h-40 overflow-auto rounded border border-line bg-panel">
                {bulkResult.map((b, i) => (
                  <div
                    key={`${b.label}-${i}`}
                    className="flex items-start gap-2 border-b border-line px-2 py-1 text-xs last:border-b-0"
                  >
                    <span className={b.ok ? "text-ok" : "text-rose"}>
                      {b.ok ? "✓" : "✗"}
                    </span>
                    <span className="font-medium text-fg">{b.label}</span>
                    {b.error ? <span className="text-rose">— {b.error}</span> : null}
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        ) : (
          <div className="flex flex-col gap-3 rounded-lg border border-line bg-fill p-3">
            {method === "bulkPaste" ? null : (
              <label className="flex flex-col gap-1 text-xs text-muted">
                {platform === "dola" ? "Tên tài khoản" : "Nhãn"}
                <input
                  className="rounded-md border border-line bg-fill px-2.5 py-1.5 text-sm text-fg placeholder:text-ink3"
                  value={label}
                  disabled={busy}
                  onChange={(e) => setLabel(e.target.value)}
                  placeholder={platform === "dola" ? "vd acc1" : "vd Gmail acc1"}
                />
                {nameError ? <span className="text-rose">{NAME_INVALID_MSG}</span> : null}
              </label>
            )}

            {method === "paste" || method === "bulkPaste" ? (
              <>
                <label className="flex flex-col gap-1 text-xs text-muted">
                  {method === "bulkPaste"
                    ? "Dán hàng loạt — mỗi dòng một tài khoản"
                    : "Cookie / nội dung đăng nhập"}
                  <textarea
                    className="h-28 resize-none rounded-md border border-line bg-fill px-2.5 py-1.5 font-mono text-xs text-fg placeholder:text-ink3"
                    value={secret}
                    disabled={busy}
                    onChange={(e) => setSecret(e.target.value)}
                    placeholder={
                      method === "bulkPaste"
                        ? "email|...|cookies (mỗi dòng 1 tài khoản). Tự phát hiện cookie (name=value; ... hoặc Netscape)."
                        : "Dán cookie/nội dung đăng nhập vào đây (name=value; ... hoặc Netscape)"
                    }
                  />
                </label>
                {method === "bulkPaste" ? (
                  <p className="text-[11px] text-ink3">
                    Tên tài khoản tự lấy từ trường đầu (email) mỗi dòng. Tự tách cookie trong mỗi dòng.
                  </p>
                ) : null}

                {platform !== "dola" ? (
                  <div className="flex flex-col gap-1 text-xs text-muted">
                    Trình duyệt
                    <div className="flex overflow-hidden rounded-md border border-line self-start">
                      {(
                        [
                          ["chromium", "Chrome"],
                          ["firefox_portable", "Firefox Portable"],
                        ] as ReadonlyArray<readonly [BrowserEngineId, string]>
                      ).map(([id, text]) => (
                        <button
                          key={id}
                          type="button"
                          disabled={busy}
                          className={
                            "px-2.5 py-1.5 text-xs disabled:cursor-not-allowed disabled:opacity-50 " +
                            (engine === id
                              ? "bg-primary text-primary-fg"
                              : "bg-fill text-muted hover:bg-fill2")
                          }
                          onClick={() => setEngine(id)}
                        >
                          {text}
                        </button>
                      ))}
                    </div>
                    {engine === "firefox_portable" ? (
                      <span className="text-[11px] text-warn">
                        Firefox Portable: cookie được tiêm vào profile để bạn mở xem/dùng TAY. Tài
                        khoản này sẽ không verify/chạy workflow tự động (tự động hoá chỉ chạy trên
                        Chrome). Cần đặt biến môi trường LVC_FIREFOX trỏ tới firefox.exe nếu chưa
                        tìm thấy.
                      </span>
                    ) : null}
                  </div>
                ) : null}
              </>
            ) : (
              <p className="text-[11px] text-ink3">
                Cửa sổ trình duyệt sẽ mở tới trang đăng nhập. Bạn TỰ đăng nhập trong cửa sổ đó; phiên
                được lưu vào profile để tái dùng. Ứng dụng không tự điền thông tin đăng nhập.
              </p>
            )}

            <label className="flex flex-col gap-1 text-xs text-muted">
              Proxy (tuỳ chọn)
              <input
                className="rounded-md border border-line bg-fill px-2.5 py-1.5 font-mono text-sm text-fg placeholder:text-ink3"
                value={proxy}
                disabled={busy}
                onChange={(e) => setProxy(e.target.value)}
                placeholder="để trống, hoặc ip:port"
              />
              {proxyError ? <span className="text-rose">{proxyError}</span> : null}
            </label>

            <div className="flex justify-end">
              <button
                type="button"
                disabled={
                  method === "paste"
                    ? !canSubmitPaste
                    : method === "bulkPaste"
                      ? !canSubmitBulkPaste
                      : !canSubmitBrowser
                }
                className={BTN_PRIMARY_SM}
                onClick={() =>
                  void (method === "paste"
                    ? submitPaste()
                    : method === "bulkPaste"
                      ? submitBulkPaste()
                      : submitBrowser())
                }
              >
                {busy
                  ? "Đang xử lý…"
                  : method === "paste"
                    ? "Thêm"
                    : method === "bulkPaste"
                      ? "Thêm hàng loạt"
                      : "Mở trình duyệt đăng nhập"}
              </button>
            </div>
            {method === "bulkPaste" && bulkResult ? (
              <div className="max-h-40 overflow-auto rounded border border-line bg-panel">
                {bulkResult.map((b, i) => (
                  <div
                    key={`${b.label}-${i}`}
                    className="flex items-start gap-2 border-b border-line px-2 py-1 text-xs last:border-b-0"
                  >
                    <span className={b.ok ? "text-ok" : "text-rose"}>{b.ok ? "✓" : "✗"}</span>
                    <span className="font-medium text-fg">{b.label}</span>
                    {b.error ? <span className="text-rose">— {b.error}</span> : null}
                  </div>
                ))}
              </div>
            ) : null}
          </div>
        )}

        {error ? <p className="text-xs text-rose">{error}</p> : null}
        {okMsg ? <p className="text-xs text-ok">{okMsg}</p> : null}

        <div className="flex justify-end border-t border-line pt-3">
          <button type="button" className={BTN_NEUTRAL_SM} onClick={onClose}>
            Đóng
          </button>
        </div>
      </div>
    </Dialog>
  );
}
