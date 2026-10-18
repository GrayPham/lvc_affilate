// Dialog "Sửa proxy" cho MỘT dòng trong bảng tài khoản đã hợp nhất (AccountsPage). CHỈ còn sửa
// Proxy — Kiểm tra/Mở trình duyệt/Bật-Tắt/Xoá đã TÁCH RA thành nút riêng trực tiếp trên mỗi hàng +
// menu chuột phải ở `AccountsPage.tsx` (review người dùng: "tách bớt chức năng của nút sửa ra, chứ
// sao gom hết vào được, có ai làm vậy đâu" — trước đây dialog này ôm cả 5 việc).
import { useState } from "react";

import Dialog from "./Dialog";
import { BTN_NEUTRAL_SM, BTN_PRIMARY_SM } from "../ui/buttonStyles";
import { useAccountsStore } from "../store/accounts";
import { useCredentialsStore } from "../store/credentials";
import { validateProxy } from "../format/account";
import { credentialProviderLabel, describeCredentialError } from "../format/credential";
import { describeAccountError } from "../format/account";
import { proxyTest } from "../ipc/commands";
import { errorText } from "../ipc/error_text";
import type { AccountStatus, CredentialView, ProxyProbe } from "../ipc/types";

export type EditTarget =
  | { source: "dola"; row: AccountStatus }
  | { source: "credential"; row: CredentialView };

export default function EditAccountDialog({
  target,
  onClose,
}: {
  target: EditTarget;
  onClose: () => void;
}) {
  const setProxyDola = useAccountsStore((s) => s.setProxy);
  const updateCredential = useCredentialsStore((s) => s.update);

  const initialProxy = target.source === "dola" ? target.row.proxy : target.row.proxyMasked;
  const [proxy, setProxy] = useState(initialProxy);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [okMsg, setOkMsg] = useState<string | null>(null);

  const [testing, setTesting] = useState(false);
  const [probe, setProbe] = useState<ProxyProbe | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);

  const proxyChanged = proxy !== initialProxy;
  const proxyError = proxy === "" ? null : validateProxy(proxy);
  // Proxy da luu hien dang che mat khau (`••••`) — khong gui chuoi che di kiem (se sai mat khau).
  // Phai nhap lai proxy moi test duoc; proxy rong (= IP may) van test duoc de xem IP hien tai.
  const proxyMasked = proxy.includes("•");
  const canTest = !testing && !busy && proxyError === null && !proxyMasked;

  async function testProxy(): Promise<void> {
    setTesting(true);
    setProbe(null);
    setProbeError(null);
    try {
      setProbe(await proxyTest(proxy));
    } catch (e) {
      setProbeError(errorText(e));
    } finally {
      setTesting(false);
    }
  }

  const name = target.source === "dola" ? target.row.name : target.row.label;
  const platformLabel =
    target.source === "dola" ? "Dola" : credentialProviderLabel(target.row.provider);

  async function saveProxy(): Promise<void> {
    if (proxyError !== null) {
      return;
    }
    setBusy(true);
    setError(null);
    setOkMsg(null);
    try {
      if (target.source === "dola") {
        await setProxyDola(target.row.name, proxy);
      } else {
        await updateCredential(target.row.id, { proxy });
      }
      setOkMsg("Đã lưu proxy.");
    } catch (e) {
      setError(target.source === "dola" ? describeAccountError(e) : describeCredentialError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title={`Sửa proxy — ${name}`} onClose={onClose} width="26rem">
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between text-xs text-muted">
          <span>Nền tảng</span>
          <span className="font-medium text-fg">{platformLabel}</span>
        </div>

        <label className="flex flex-col gap-1 text-xs text-muted">
          Proxy
          <div className="flex gap-2">
            <input
              className="flex-1 rounded-md border border-line bg-fill px-2.5 py-1.5 font-mono text-sm text-fg placeholder:text-ink3"
              value={proxy}
              disabled={busy}
              onChange={(e) => {
                setProxy(e.target.value);
                // Ket qua kiem cu khong con dung voi chuoi vua sua — xoa di.
                setProbe(null);
                setProbeError(null);
              }}
              placeholder="để trống, hoặc ip:port, hoặc direct"
            />
            <button
              type="button"
              disabled={busy || !proxyChanged || proxyError !== null}
              className={BTN_NEUTRAL_SM}
              onClick={() => void saveProxy()}
            >
              Lưu
            </button>
          </div>
          {proxyError ? <span className="text-rose">{proxyError}</span> : null}
        </label>

        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={!canTest}
              className={BTN_NEUTRAL_SM}
              onClick={() => void testProxy()}
              title={
                proxyMasked
                  ? "Nhập lại proxy để kiểm tra — mật khẩu đã lưu đang được ẩn"
                  : "Nối thử qua proxy để xem IP ra, vị trí, độ trễ"
              }
            >
              {testing ? "Đang kiểm tra…" : "Kiểm tra proxy"}
            </button>
            {proxyMasked ? (
              <span className="text-[11px] text-ink3">Nhập lại proxy để kiểm tra (mật khẩu đã ẩn)</span>
            ) : proxy === "" ? (
              <span className="text-[11px] text-ink3">Để trống = kiểm tra IP máy (không proxy)</span>
            ) : null}
          </div>

          {probe ? (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-md border border-ok/30 bg-ok/10 px-3 py-2 text-xs">
              <span className="font-semibold text-ok">✓ Kết nối được</span>
              <span className="text-muted">
                IP ra: <span className="font-mono text-fg">{probe.outboundIp || "—"}</span>
              </span>
              <span className="text-muted">
                Vị trí: <span className="text-fg">{probe.location || "—"}</span>
              </span>
              <span className="text-muted">
                Độ trễ: <span className="text-fg">{probe.latencyMs} ms</span>
              </span>
            </div>
          ) : null}
          {probeError ? (
            <div className="rounded-md border border-rose/30 bg-rose/10 px-3 py-2 text-xs text-rose">
              ✕ {probeError}
            </div>
          ) : null}
        </div>

        {error ? <p className="text-xs text-rose">{error}</p> : null}
        {okMsg ? <p className="text-xs text-ok">{okMsg}</p> : null}

        <div className="flex justify-end border-t border-line pt-3">
          <button type="button" className={BTN_PRIMARY_SM} onClick={onClose}>
            Đóng
          </button>
        </div>
      </div>
    </Dialog>
  );
}
