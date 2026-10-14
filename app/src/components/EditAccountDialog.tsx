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
import type { AccountStatus, CredentialView } from "../ipc/types";

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

  const proxyChanged = proxy !== initialProxy;
  const proxyError = proxy === "" ? null : validateProxy(proxy);

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
        <div className="flex items-center justify-between text-xs text-ink2">
          <span>Nền tảng</span>
          <span className="font-medium text-ink">{platformLabel}</span>
        </div>

        <label className="flex flex-col gap-1 text-xs text-ink2">
          Proxy
          <div className="flex gap-2">
            <input
              className="flex-1 rounded-md border border-line bg-fill px-2.5 py-1.5 font-mono text-sm text-ink placeholder:text-ink3"
              value={proxy}
              disabled={busy}
              onChange={(e) => setProxy(e.target.value)}
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
