// Cong ban quyen: man khoa 2 buoc (dang nhap tai khoan -> kich hoat license) boc toan app. Chua
// kich hoat hop le thi KHONG render app chinh. Auto-login offline tu session da cache (backend
// `license_status`). Bi mat (mat khau/ma license) chi gui xuong backend, khong luu o store/log.
import { useEffect, useState } from "react";

import { licenseActivate, licenseLogin, licenseStatus } from "../ipc/commands";
import { BTN_PRIMARY_SM } from "../ui/buttonStyles";

type Phase = "loading" | "login" | "activate" | "activated";

function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return "Có lỗi xảy ra, vui lòng thử lại.";
}

export default function LicenseGate({ children }: { children: React.ReactNode }) {
  const [phase, setPhase] = useState<Phase>("loading");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [licenseKey, setLicenseKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    licenseStatus()
      .then((s) => setPhase(s.activated ? "activated" : "login"))
      .catch(() => setPhase("login"));
  }, []);

  async function doLogin(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      await licenseLogin(username.trim(), password);
      setPhase("activate");
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function doActivate(): Promise<void> {
    setBusy(true);
    setError(null);
    try {
      const s = await licenseActivate(licenseKey.trim());
      setPhase(s.activated ? "activated" : "activate");
      if (!s.activated) {
        setError("Kích hoạt chưa thành công, kiểm tra lại mã.");
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  if (phase === "activated") {
    return <>{children}</>;
  }

  const inputCls =
    "w-full rounded-md border border-line bg-bg2 px-3 py-2 text-sm text-fg placeholder:text-muted focus:border-accent focus:outline-none";

  return (
    <div className="grid h-screen w-screen place-items-center bg-bg p-4 text-fg">
      <div className="flex w-full max-w-sm flex-col gap-4 rounded-2xl border border-line bg-panel p-6 shadow-panel">
        <div className="flex flex-col items-center gap-2">
          <img src="/icon.png" alt="LVC Shop" className="h-12 w-12 rounded-lg object-cover" />
          <div className="text-center">
            <div className="text-lg font-bold">LVC Shop</div>
            <div className="text-xs text-muted">
              {phase === "activate" ? "Nhập mã license để kích hoạt" : "Đăng nhập để tiếp tục"}
            </div>
          </div>
        </div>

        {phase === "loading" ? (
          <p className="py-4 text-center text-sm text-muted">Đang kiểm tra bản quyền…</p>
        ) : phase === "login" ? (
          <form
            className="flex flex-col gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              void doLogin();
            }}
          >
            <input
              className={inputCls}
              placeholder="Tên đăng nhập"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              disabled={busy}
              autoFocus
            />
            <input
              className={inputCls}
              type="password"
              placeholder="Mật khẩu"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={busy}
            />
            <button
              type="submit"
              className={BTN_PRIMARY_SM}
              disabled={busy || username.trim() === "" || password === ""}
            >
              {busy ? "Đang đăng nhập…" : "Đăng nhập"}
            </button>
          </form>
        ) : (
          <form
            className="flex flex-col gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              void doActivate();
            }}
          >
            <input
              className={`${inputCls} font-mono`}
              placeholder="XXXX-XXXX-XXXX-XXXX"
              value={licenseKey}
              onChange={(e) => setLicenseKey(e.target.value)}
              disabled={busy}
              autoFocus
            />
            <button
              type="submit"
              className={BTN_PRIMARY_SM}
              disabled={busy || licenseKey.trim() === ""}
            >
              {busy ? "Đang kích hoạt…" : "Kích hoạt"}
            </button>
            <button
              type="button"
              className="text-xs text-muted hover:text-fg"
              onClick={() => {
                setPhase("login");
                setError(null);
              }}
              disabled={busy}
            >
              ← Quay lại đăng nhập
            </button>
          </form>
        )}

        {error ? <p className="text-center text-xs text-rose">{error}</p> : null}
      </div>
    </div>
  );
}
